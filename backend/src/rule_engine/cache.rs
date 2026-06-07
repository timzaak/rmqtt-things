use crate::db::models::AlarmRule;
use anyhow::Context;
use async_trait::async_trait;
use dashmap::DashMap;
use redis::AsyncCommands;
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::warn;

/// Result of a duration check for a rule that requires sustained condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationCheckResult {
    /// Condition has been met for the full duration window.
    Met,
    /// Condition was met before but duration has not elapsed yet.
    NotYetMet,
    /// First time condition is met; tracking started now.
    JustStarted,
    /// State store is unavailable; treat as "not started tracking" (will not trigger alarm).
    NotStarted,
}

/// Trait for rule state storage (dedup and duration tracking).
/// Implemented by InMemoryRuleStateStore (DashMap) and RedisRuleStateStore.
#[async_trait]
pub trait RuleStateStore: Send + Sync {
    /// Check whether this (rule_id, device_id) should be skipped due to throttle dedup.
    /// Returns `true` if the trigger should be skipped (still in dedup window).
    async fn check_dedup(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
        now: OffsetDateTime,
    ) -> bool;

    /// Mark a (rule_id, device_id) as triggered. `throttle_minutes` is used by Redis
    /// to set TTL; InMemory ignores it (DashMap has no TTL).
    async fn mark_triggered(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
    ) -> anyhow::Result<()>;

    /// Check whether the sustained condition duration has been met.
    async fn check_duration(
        &self,
        rule_id: i64,
        device_id: &str,
        duration_minutes: i32,
        now: OffsetDateTime,
    ) -> DurationCheckResult;

    /// Reset duration tracking when the condition is no longer met.
    async fn reset_duration(&self, rule_id: i64, device_id: &str) -> anyhow::Result<()>;

    /// Invalidate dedup and duration state for the given rule IDs.
    async fn invalidate_by_rule_ids(&self, rule_ids: &[i64]) -> anyhow::Result<()>;
}

/// In-memory implementation of RuleStateStore using DashMap.
/// Logic is ported from the original RuleCache dedup/duration methods.
pub struct InMemoryRuleStateStore {
    dedup: DashMap<String, OffsetDateTime>,
    duration_tracking: DashMap<String, OffsetDateTime>,
}

impl InMemoryRuleStateStore {
    pub fn new() -> Self {
        Self {
            dedup: DashMap::new(),
            duration_tracking: DashMap::new(),
        }
    }
}

#[async_trait]
impl RuleStateStore for InMemoryRuleStateStore {
    async fn check_dedup(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
        now: OffsetDateTime,
    ) -> bool {
        if throttle_minutes <= 0 {
            return false;
        }
        let key = format!("{}:{}", rule_id, device_id);
        match self.dedup.get(&key) {
            Some(last_triggered) => {
                let elapsed = now - *last_triggered;
                elapsed.whole_minutes() < throttle_minutes
            }
            None => false,
        }
    }

    async fn mark_triggered(
        &self,
        rule_id: i64,
        device_id: &str,
        _throttle_minutes: i64,
    ) -> anyhow::Result<()> {
        let key = format!("{}:{}", rule_id, device_id);
        self.dedup.insert(key, OffsetDateTime::now_utc());
        Ok(())
    }

    async fn check_duration(
        &self,
        rule_id: i64,
        device_id: &str,
        duration_minutes: i32,
        now: OffsetDateTime,
    ) -> DurationCheckResult {
        if duration_minutes <= 0 {
            return DurationCheckResult::Met;
        }
        let key = format!("{}:{}", rule_id, device_id);
        match self.duration_tracking.get(&key) {
            Some(stored) => {
                let elapsed = now - *stored;
                if elapsed.whole_minutes() >= duration_minutes as i64 {
                    DurationCheckResult::Met
                } else {
                    DurationCheckResult::NotYetMet
                }
            }
            None => {
                self.duration_tracking.insert(key, now);
                DurationCheckResult::JustStarted
            }
        }
    }

    async fn reset_duration(&self, rule_id: i64, device_id: &str) -> anyhow::Result<()> {
        let key = format!("{}:{}", rule_id, device_id);
        self.duration_tracking.remove(&key);
        Ok(())
    }

    async fn invalidate_by_rule_ids(&self, rule_ids: &[i64]) -> anyhow::Result<()> {
        if rule_ids.is_empty() {
            return Ok(());
        }
        let rule_id_set: std::collections::HashSet<String> =
            rule_ids.iter().map(|id| id.to_string()).collect();
        self.duration_tracking.retain(|k, _| {
            k.split(':')
                .next()
                .is_none_or(|id| !rule_id_set.contains(id))
        });
        self.dedup.retain(|k, _| {
            k.split(':')
                .next()
                .is_none_or(|id| !rule_id_set.contains(id))
        });
        Ok(())
    }
}

/// Redis-backed implementation of RuleStateStore with graceful degradation.
/// Follows the same connection pattern as RedisSchemaCache:
/// acquires a multiplexed async connection per method call (internally pooled by redis crate).
pub struct RedisRuleStateStore {
    client: redis::Client,
}

impl RedisRuleStateStore {
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url).context("Failed to open redis client")?;
        Ok(Self { client })
    }

    fn dedup_key(rule_id: i64, device_id: &str) -> String {
        format!("alarm:dedup:{}:{}", rule_id, device_id)
    }

    fn duration_key(rule_id: i64, device_id: &str) -> String {
        format!("alarm:duration:{}:{}", rule_id, device_id)
    }
}

#[async_trait]
impl RuleStateStore for RedisRuleStateStore {
    async fn check_dedup(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
        now: OffsetDateTime,
    ) -> bool {
        if throttle_minutes <= 0 {
            return false;
        }
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis connection failed in check_dedup, allowing trigger");
                return true;
            }
        };
        let key = Self::dedup_key(rule_id, device_id);
        let value: Option<String> = match conn.get(&key).await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Redis GET failed in check_dedup, allowing trigger");
                return true;
            }
        };
        match value {
            Some(ts) => {
                let last_triggered = match OffsetDateTime::parse(
                    &ts,
                    &time::format_description::well_known::Iso8601::DEFAULT,
                ) {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(error = %e, "Failed to parse dedup timestamp, allowing trigger");
                        return true;
                    }
                };
                let elapsed = now - last_triggered;
                elapsed.whole_minutes() < throttle_minutes
            }
            None => false,
        }
    }

    async fn mark_triggered(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
    ) -> anyhow::Result<()> {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis connection failed in mark_triggered");
                return Err(anyhow::anyhow!(
                    "Redis connection failed in mark_triggered: {}",
                    e
                ));
            }
        };
        let key = Self::dedup_key(rule_id, device_id);
        let now = OffsetDateTime::now_utc();
        let ttl_secs = (throttle_minutes * 2 * 60).max(60);
        let ts = now.format(&time::format_description::well_known::Iso8601::DEFAULT);
        match ts {
            Ok(ts_str) => {
                if let Err(e) = redis::cmd("SET")
                    .arg(&key)
                    .arg(&ts_str)
                    .arg("EX")
                    .arg(ttl_secs)
                    .query_async::<()>(&mut conn)
                    .await
                {
                    warn!(error = %e, "Redis SET failed in mark_triggered");
                    return Err(anyhow::anyhow!("Redis SET failed in mark_triggered: {}", e));
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to format timestamp in mark_triggered");
                return Err(anyhow::anyhow!("Failed to format timestamp: {}", e));
            }
        }
        Ok(())
    }

    async fn check_duration(
        &self,
        rule_id: i64,
        device_id: &str,
        duration_minutes: i32,
        now: OffsetDateTime,
    ) -> DurationCheckResult {
        if duration_minutes <= 0 {
            return DurationCheckResult::Met;
        }
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis connection failed in check_duration, returning NotStarted");
                return DurationCheckResult::NotStarted;
            }
        };
        let key = Self::duration_key(rule_id, device_id);
        let value: Option<String> = match conn.get(&key).await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Redis GET failed in check_duration, returning NotStarted");
                return DurationCheckResult::NotStarted;
            }
        };
        match value {
            Some(ts) => {
                let stored = match OffsetDateTime::parse(
                    &ts,
                    &time::format_description::well_known::Iso8601::DEFAULT,
                ) {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(error = %e, "Failed to parse duration timestamp, returning NotStarted");
                        return DurationCheckResult::NotStarted;
                    }
                };
                let elapsed = now - stored;
                if elapsed.whole_minutes() >= duration_minutes as i64 {
                    DurationCheckResult::Met
                } else {
                    DurationCheckResult::NotYetMet
                }
            }
            None => {
                // First time: set key with TTL and return JustStarted
                let ts = now.format(&time::format_description::well_known::Iso8601::DEFAULT);
                let ts_str = match ts {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(error = %e, "Failed to format timestamp in check_duration");
                        return DurationCheckResult::NotStarted;
                    }
                };
                let ttl_secs = (duration_minutes as i64 * 3 * 60).max(60);
                if let Err(e) = redis::cmd("SET")
                    .arg(&key)
                    .arg(&ts_str)
                    .arg("EX")
                    .arg(ttl_secs)
                    .query_async::<()>(&mut conn)
                    .await
                {
                    warn!(error = %e, "Redis SET failed in check_duration");
                    return DurationCheckResult::NotStarted;
                }
                DurationCheckResult::JustStarted
            }
        }
    }

    async fn reset_duration(&self, rule_id: i64, device_id: &str) -> anyhow::Result<()> {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis connection failed in reset_duration");
                return Err(anyhow::anyhow!(
                    "Redis connection failed in reset_duration: {}",
                    e
                ));
            }
        };
        let key = Self::duration_key(rule_id, device_id);
        if let Err(e) = conn.del::<_, ()>(&key).await {
            warn!(error = %e, "Redis DEL failed in reset_duration");
            return Err(anyhow::anyhow!("Redis DEL failed in reset_duration: {}", e));
        }
        Ok(())
    }

    async fn invalidate_by_rule_ids(&self, rule_ids: &[i64]) -> anyhow::Result<()> {
        if rule_ids.is_empty() {
            return Ok(());
        }
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis connection failed in invalidate_by_rule_ids");
                return Err(anyhow::anyhow!(
                    "Redis connection failed in invalidate_by_rule_ids: {}",
                    e
                ));
            }
        };
        for &rule_id in rule_ids {
            for pattern in &[
                format!("alarm:dedup:{}:*", rule_id),
                format!("alarm:duration:{}:*", rule_id),
            ] {
                let mut cursor: u64 = 0;
                loop {
                    let (next_cursor, keys): (u64, Vec<String>) = match redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(pattern)
                        .arg("COUNT")
                        .arg(100u64)
                        .query_async(&mut conn)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            warn!(error = %e, rule_id, "Redis SCAN failed in invalidate_by_rule_ids");
                            break;
                        }
                    };
                    if !keys.is_empty()
                        && let Err(e) = conn.del::<_, ()>(&keys).await
                    {
                        warn!(error = %e, rule_id, "Redis DEL failed in invalidate_by_rule_ids");
                    }
                    cursor = next_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Rule cache with DashMap-backed storage for rules (keyed by product_id)
/// and state_store for dedup and duration tracking.
#[derive(Clone)]
pub struct RuleCache {
    rules: DashMap<String, Vec<AlarmRule>>,
    state_store: Arc<dyn RuleStateStore>,
}

impl RuleCache {
    pub fn new(state_store: Arc<dyn RuleStateStore>) -> Self {
        Self {
            rules: DashMap::new(),
            state_store,
        }
    }

    /// Convenience constructor using in-memory state store.
    #[cfg(test)]
    pub fn new_in_memory() -> Self {
        Self::new(Arc::new(InMemoryRuleStateStore::new()))
    }

    /// Get cached rules for a product.
    pub fn get_rules(&self, product_id: &str) -> Option<Vec<AlarmRule>> {
        self.rules
            .get(product_id)
            .map(|entry| entry.value().clone())
    }

    /// Set cached rules for a product.
    pub fn set_rules(&self, product_id: &str, rules: Vec<AlarmRule>) {
        self.rules.insert(product_id.to_string(), rules);
    }

    /// Invalidate cached rules for a specific product, also clearing
    /// any duration_tracking and dedup entries for rules of that product.
    pub async fn invalidate_product(&self, product_id: &str) {
        let rule_ids: Vec<i64> = self
            .rules
            .get(product_id)
            .map(|entry| entry.value().iter().map(|r| r.id).collect())
            .unwrap_or_default();
        self.rules.remove(product_id);
        if !rule_ids.is_empty() {
            let _ = self.state_store.invalidate_by_rule_ids(&rule_ids).await;
        }
    }

    /// Invalidate all cached rules.
    #[cfg(test)]
    pub fn invalidate_all(&self) {
        self.rules.clear();
    }

    /// Check whether this (rule_id, device_id) combination should be skipped
    /// because it was triggered within the throttle window.
    ///
    /// Returns `true` if the trigger should be skipped (still in dedup window).
    pub async fn check_dedup(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
        now: OffsetDateTime,
    ) -> bool {
        self.state_store
            .check_dedup(rule_id, device_id, throttle_minutes, now)
            .await
    }

    /// Mark a (rule_id, device_id) as triggered at the current time.
    pub async fn mark_triggered(
        &self,
        rule_id: i64,
        device_id: &str,
        throttle_minutes: i64,
    ) -> anyhow::Result<()> {
        self.state_store
            .mark_triggered(rule_id, device_id, throttle_minutes)
            .await
    }

    /// Check whether the sustained condition duration has been met.
    ///
    /// - If no tracking entry exists, inserts `now` and returns `JustStarted`.
    /// - If entry exists and `now - stored_time >= duration_minutes`, returns `Met`.
    /// - If entry exists but duration not yet reached, returns `NotYetMet`.
    pub async fn check_duration(
        &self,
        rule_id: i64,
        device_id: &str,
        duration_minutes: i32,
        now: OffsetDateTime,
    ) -> DurationCheckResult {
        self.state_store
            .check_duration(rule_id, device_id, duration_minutes, now)
            .await
    }

    /// Reset duration tracking when the condition is no longer met.
    pub async fn reset_duration(&self, rule_id: i64, device_id: &str) -> anyhow::Result<()> {
        self.state_store.reset_duration(rule_id, device_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::Duration;

    fn make_rule(id: i64, product_id: &str) -> AlarmRule {
        AlarmRule {
            id,
            product_id: product_id.to_string(),
            name: format!("rule_{}", id),
            description: None,
            trigger_type: "property".to_string(),
            trigger_config: json!({}),
            condition: json!({"operator": "always"}),
            actions: json!([]),
            enabled: true,
            throttle_minutes: 0,
            duration_minutes: 0,
            clear_condition: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[tokio::test]
    async fn test_set_and_get_rules() {
        let cache = RuleCache::new_in_memory();
        let rule = make_rule(1, "prod_a");
        cache.set_rules("prod_a", vec![rule.clone()]);

        let result = cache.get_rules("prod_a").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[tokio::test]
    async fn test_get_rules_miss() {
        let cache = RuleCache::new_in_memory();
        assert!(cache.get_rules("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_invalidate_product() {
        let cache = RuleCache::new_in_memory();
        cache.set_rules("prod_a", vec![make_rule(1, "prod_a")]);
        cache.invalidate_product("prod_a").await;
        assert!(cache.get_rules("prod_a").is_none());
    }

    #[tokio::test]
    async fn test_invalidate_all() {
        let cache = RuleCache::new_in_memory();
        cache.set_rules("prod_a", vec![make_rule(1, "prod_a")]);
        cache.set_rules("prod_b", vec![make_rule(2, "prod_b")]);
        cache.invalidate_all();
        assert!(cache.get_rules("prod_a").is_none());
        assert!(cache.get_rules("prod_b").is_none());
    }

    #[tokio::test]
    async fn test_dedup_skip_within_window() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        cache.mark_triggered(1, "device_1", 10).await.unwrap();

        // Should be skipped because we just marked it
        assert!(cache.check_dedup(1, "device_1", 10, now).await);
    }

    #[tokio::test]
    async fn test_dedup_pass_after_window() {
        let cache = RuleCache::new_in_memory();
        cache.mark_triggered(1, "device_1", 10).await.unwrap();
        // Check 20 minutes in the future (well past the 10-minute throttle window)
        let future_now = OffsetDateTime::now_utc() + Duration::minutes(20);

        // Should NOT be skipped because 20 > 10 minutes
        assert!(!cache.check_dedup(1, "device_1", 10, future_now).await);
    }

    #[tokio::test]
    async fn test_dedup_zero_throttle_always_passes() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        cache.mark_triggered(1, "device_1", 10).await.unwrap();

        // throttle_minutes=0 means no dedup
        assert!(!cache.check_dedup(1, "device_1", 0, now).await);
    }

    #[tokio::test]
    async fn test_dedup_no_previous_trigger() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        assert!(!cache.check_dedup(1, "device_1", 10, now).await);
    }

    // --- Duration tracking tests ---

    #[tokio::test]
    async fn test_duration_first_condition_met_returns_just_started() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        let result = cache.check_duration(1, "device_1", 5, now).await;
        assert_eq!(result, DurationCheckResult::JustStarted);
    }

    #[tokio::test]
    async fn test_duration_not_yet_met() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        // Start tracking
        cache.check_duration(1, "device_1", 5, now).await;

        // Check again 3 minutes later (not yet 5)
        let later = now + Duration::minutes(3);
        let result = cache.check_duration(1, "device_1", 5, later).await;
        assert_eq!(result, DurationCheckResult::NotYetMet);
    }

    #[tokio::test]
    async fn test_duration_met_after_window() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        // Start tracking
        cache.check_duration(1, "device_1", 5, now).await;

        // Check again 6 minutes later (exceeds 5)
        let later = now + Duration::minutes(6);
        let result = cache.check_duration(1, "device_1", 5, later).await;
        assert_eq!(result, DurationCheckResult::Met);
    }

    #[tokio::test]
    async fn test_duration_reset_clears_tracking() {
        let cache = RuleCache::new_in_memory();
        let now = OffsetDateTime::now_utc();
        cache.check_duration(1, "device_1", 5, now).await;

        // Reset
        cache.reset_duration(1, "device_1").await.unwrap();

        // After reset, should start fresh (JustStarted)
        let result = cache.check_duration(1, "device_1", 5, now).await;
        assert_eq!(result, DurationCheckResult::JustStarted);
    }

    #[tokio::test]
    async fn test_duration_full_cycle() {
        // Full cycle: first met -> not yet met -> condition breaks -> reset
        // -> condition met again -> duration passes -> Met
        let cache = RuleCache::new_in_memory();
        let t0 = OffsetDateTime::now_utc();

        // 1. First condition met
        let result = cache.check_duration(1, "dev", 3, t0).await;
        assert_eq!(result, DurationCheckResult::JustStarted);

        // 2. Still not met (1 min later)
        let result = cache
            .check_duration(1, "dev", 3, t0 + Duration::minutes(1))
            .await;
        assert_eq!(result, DurationCheckResult::NotYetMet);

        // 3. Condition breaks -> reset
        cache.reset_duration(1, "dev").await.unwrap();

        // 4. Condition met again at t0+5
        let result = cache
            .check_duration(1, "dev", 3, t0 + Duration::minutes(5))
            .await;
        assert_eq!(result, DurationCheckResult::JustStarted);

        // 5. Duration passes (4 minutes after re-start = t0+9)
        let result = cache
            .check_duration(1, "dev", 3, t0 + Duration::minutes(9))
            .await;
        assert_eq!(result, DurationCheckResult::Met);
    }

    // -------------------------------------------------------------------------
    // InMemoryRuleStateStore direct tests (through RuleStateStore trait)
    //
    // User Story: As a backend developer, I need to verify that
    // InMemoryRuleStateStore correctly implements dedup windowing, duration
    // tracking, and edge cases, so that alarm rule evaluation behaves
    // predictably without relying on external services.
    //
    // Covers:
    // - Dedup: no previous trigger, within window, after window, zero/negative throttle
    // - Duration: first check (JustStarted), not yet met, met after window,
    //   reset clears tracking, zero minutes returns Met, full cycle
    // - mark_triggered: succeeds and updates dedup state
    // - reset_duration: succeeds and clears duration state
    // -------------------------------------------------------------------------

    mod in_memory_state_store_tests {
        use super::*;
        use time::Duration;

        /// Helper: create an Arc<dyn RuleStateStore> backed by InMemoryRuleStateStore.
        fn make_store() -> Arc<dyn RuleStateStore> {
            Arc::new(InMemoryRuleStateStore::new())
        }

        // --- Dedup tests ---

        // Covers: check_dedup returns false when no prior trigger exists,
        // meaning the alarm is not suppressed.
        #[tokio::test]
        async fn test_in_memory_dedup_no_previous_trigger_returns_false() {
            let store = make_store();
            let now = OffsetDateTime::now_utc();
            // No mark_triggered call; should not be in dedup window.
            let is_dup = store.check_dedup(1, "device_1", 10, now).await;
            assert!(
                !is_dup,
                "Expected false: no previous trigger means not in dedup window"
            );
        }

        // Covers: check_dedup returns true when checked within the throttle
        // window after mark_triggered, suppressing duplicate alarms.
        #[tokio::test]
        async fn test_in_memory_dedup_within_window_returns_true() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();
            store.mark_triggered(1, "device_1", 10).await.unwrap();

            // Check at t0 + 5 minutes (within the 10-minute window).
            let is_dup = store
                .check_dedup(1, "device_1", 10, t0 + Duration::minutes(5))
                .await;
            assert!(is_dup, "Expected true: still within dedup window");
        }

        // Covers: check_dedup returns false when checked after the throttle
        // window expires, allowing a new alarm trigger.
        #[tokio::test]
        async fn test_in_memory_dedup_after_window_returns_false() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();
            store.mark_triggered(1, "device_1", 10).await.unwrap();

            // Check at t0 + 15 minutes (past the 10-minute window).
            let is_dup = store
                .check_dedup(1, "device_1", 10, t0 + Duration::minutes(15))
                .await;
            assert!(!is_dup, "Expected false: dedup window has expired");
        }

        // Covers: check_dedup with throttle_minutes=0 always returns false,
        // meaning no dedup suppression regardless of prior triggers.
        #[tokio::test]
        async fn test_in_memory_dedup_zero_throttle_always_passes() {
            let store = make_store();
            let now = OffsetDateTime::now_utc();
            store.mark_triggered(1, "device_1", 10).await.unwrap();

            // throttle_minutes=0 disables dedup entirely.
            let is_dup = store.check_dedup(1, "device_1", 0, now).await;
            assert!(!is_dup, "Expected false: zero throttle means no dedup");
        }

        // Covers: check_dedup with negative throttle_minutes always returns
        // false, treated the same as zero (no dedup suppression).
        #[tokio::test]
        async fn test_in_memory_dedup_negative_throttle_always_passes() {
            let store = make_store();
            let now = OffsetDateTime::now_utc();
            store.mark_triggered(1, "device_1", 10).await.unwrap();

            // Negative throttle should behave like zero: no dedup.
            let is_dup = store.check_dedup(1, "device_1", -5, now).await;
            assert!(!is_dup, "Expected false: negative throttle means no dedup");
        }

        // --- Duration tests ---

        // Covers: First check_duration call for a (rule, device) pair returns
        // JustStarted, indicating that duration tracking has begun.
        #[tokio::test]
        async fn test_in_memory_duration_first_check_returns_just_started() {
            let store = make_store();
            let now = OffsetDateTime::now_utc();
            let result = store.check_duration(1, "device_1", 5, now).await;
            assert_eq!(result, DurationCheckResult::JustStarted);
        }

        // Covers: check_duration returns NotYetMet when the elapsed time is
        // less than the required duration window.
        #[tokio::test]
        async fn test_in_memory_duration_not_yet_met() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();
            // Start tracking at t0.
            store.check_duration(1, "device_1", 5, t0).await;

            // Check 3 minutes later (less than the 5-minute duration).
            let result = store
                .check_duration(1, "device_1", 5, t0 + Duration::minutes(3))
                .await;
            assert_eq!(result, DurationCheckResult::NotYetMet);
        }

        // Covers: check_duration returns Met when the elapsed time equals or
        // exceeds the required duration window.
        #[tokio::test]
        async fn test_in_memory_duration_met_after_window() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();
            // Start tracking at t0.
            store.check_duration(1, "device_1", 5, t0).await;

            // Check 6 minutes later (exceeds the 5-minute duration).
            let result = store
                .check_duration(1, "device_1", 5, t0 + Duration::minutes(6))
                .await;
            assert_eq!(result, DurationCheckResult::Met);
        }

        // Covers: reset_duration clears tracking so a subsequent check_duration
        // returns JustStarted again, allowing the duration timer to restart.
        #[tokio::test]
        async fn test_in_memory_duration_reset_clears_tracking() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();
            store.check_duration(1, "device_1", 5, t0).await;

            // Reset the tracking state.
            store.reset_duration(1, "device_1").await.unwrap();

            // After reset, should behave as if tracking never started.
            let result = store.check_duration(1, "device_1", 5, t0).await;
            assert_eq!(result, DurationCheckResult::JustStarted);
        }

        // Covers: check_duration with duration_minutes=0 immediately returns
        // Met, because zero duration means the condition is met instantly.
        #[tokio::test]
        async fn test_in_memory_duration_zero_minutes_returns_met() {
            let store = make_store();
            let now = OffsetDateTime::now_utc();
            let result = store.check_duration(1, "device_1", 0, now).await;
            assert_eq!(result, DurationCheckResult::Met);
        }

        // Covers: Full duration lifecycle -- JustStarted -> NotYetMet -> reset
        // -> JustStarted (re-entry) -> Met, verifying that tracking state is
        // correctly managed across the entire alarm evaluation cycle.
        #[tokio::test]
        async fn test_in_memory_duration_full_cycle() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();

            // 1. First condition met -> JustStarted
            let result = store.check_duration(1, "dev", 3, t0).await;
            assert_eq!(result, DurationCheckResult::JustStarted);

            // 2. Not yet met (1 minute in, need 3)
            let result = store
                .check_duration(1, "dev", 3, t0 + Duration::minutes(1))
                .await;
            assert_eq!(result, DurationCheckResult::NotYetMet);

            // 3. Condition breaks -> reset
            store.reset_duration(1, "dev").await.unwrap();

            // 4. Condition re-met at t0+5 -> JustStarted (fresh tracking)
            let result = store
                .check_duration(1, "dev", 3, t0 + Duration::minutes(5))
                .await;
            assert_eq!(result, DurationCheckResult::JustStarted);

            // 5. Duration passes (4 minutes after re-start = t0+9)
            let result = store
                .check_duration(1, "dev", 3, t0 + Duration::minutes(9))
                .await;
            assert_eq!(result, DurationCheckResult::Met);
        }

        // --- mark_triggered tests ---

        // Covers: mark_triggered succeeds and causes subsequent check_dedup
        // to return true (within the throttle window).
        #[tokio::test]
        async fn test_in_memory_mark_triggered_succeeds() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();

            // Before mark_triggered, dedup should not suppress.
            let is_dup = store.check_dedup(1, "device_1", 10, t0).await;
            assert!(!is_dup);

            // Mark as triggered.
            store.mark_triggered(1, "device_1", 10).await.unwrap();

            // After mark_triggered, dedup should suppress within the window.
            let is_dup = store.check_dedup(1, "device_1", 10, t0).await;
            assert!(
                is_dup,
                "Expected true: mark_triggered should cause dedup suppression"
            );
        }

        // --- reset_duration tests ---

        // Covers: reset_duration returns Ok and clears duration tracking,
        // verified by a subsequent check_duration returning JustStarted.
        #[tokio::test]
        async fn test_in_memory_reset_duration_succeeds() {
            let store = make_store();
            let t0 = OffsetDateTime::now_utc();

            // Start duration tracking.
            store.check_duration(1, "device_1", 5, t0).await;

            // Reset should succeed.
            let result = store.reset_duration(1, "device_1").await;
            assert!(result.is_ok(), "reset_duration should succeed");

            // Verify state is cleared: check_duration returns JustStarted.
            let check = store.check_duration(1, "device_1", 5, t0).await;
            assert_eq!(check, DurationCheckResult::JustStarted);
        }
    }
}
