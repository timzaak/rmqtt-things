use crate::db::models::AlarmRule;
use dashmap::DashMap;
use time::OffsetDateTime;

/// Result of a duration check for a rule that requires sustained condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationCheckResult {
    /// Condition has been met for the full duration window.
    Met,
    /// Condition was met before but duration has not elapsed yet.
    NotYetMet,
    /// First time condition is met; tracking started now.
    JustStarted,
}

/// Rule cache with DashMap-backed storage for rules (keyed by product_id)
/// and dedup tracking (keyed by "rule_id:device_id").
#[derive(Clone)]
pub struct RuleCache {
    rules: DashMap<String, Vec<AlarmRule>>,
    dedup: DashMap<String, OffsetDateTime>,
    /// Duration tracking: key is "{rule_id}:{device_id}", value is the first time
    /// the condition was met (sustained from that point).
    duration_tracking: DashMap<String, OffsetDateTime>,
}

impl RuleCache {
    pub fn new() -> Self {
        Self {
            rules: DashMap::new(),
            dedup: DashMap::new(),
            duration_tracking: DashMap::new(),
        }
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
    pub fn invalidate_product(&self, product_id: &str) {
        let rule_ids: std::collections::HashSet<String> = self
            .rules
            .get(product_id)
            .map(|entry| entry.value().iter().map(|r| r.id.to_string()).collect())
            .unwrap_or_default();
        self.rules.remove(product_id);
        if !rule_ids.is_empty() {
            self.duration_tracking
                .retain(|k, _| k.split(':').next().is_none_or(|id| !rule_ids.contains(id)));
            self.dedup
                .retain(|k, _| k.split(':').next().is_none_or(|id| !rule_ids.contains(id)));
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
    pub fn check_dedup(&self, rule_id: i64, device_id: &str, throttle_minutes: i64) -> bool {
        if throttle_minutes <= 0 {
            return false;
        }
        let key = format!("{}:{}", rule_id, device_id);
        match self.dedup.get(&key) {
            Some(last_triggered) => {
                let now = OffsetDateTime::now_utc();
                let elapsed = now - *last_triggered;
                elapsed.whole_minutes() < throttle_minutes
            }
            None => false,
        }
    }

    /// Mark a (rule_id, device_id) as triggered at the current time.
    pub fn mark_triggered(&self, rule_id: i64, device_id: &str) {
        let key = format!("{}:{}", rule_id, device_id);
        self.dedup.insert(key, OffsetDateTime::now_utc());
    }

    /// Check whether the sustained condition duration has been met.
    ///
    /// - If no tracking entry exists, inserts `now` and returns `JustStarted`.
    /// - If entry exists and `now - stored_time >= duration_minutes`, returns `Met`.
    /// - If entry exists but duration not yet reached, returns `NotYetMet`.
    pub fn check_duration(
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

    /// Reset duration tracking when the condition is no longer met.
    pub fn reset_duration(&self, rule_id: i64, device_id: &str) {
        let key = format!("{}:{}", rule_id, device_id);
        self.duration_tracking.remove(&key);
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

    #[test]
    fn test_set_and_get_rules() {
        let cache = RuleCache::new();
        let rule = make_rule(1, "prod_a");
        cache.set_rules("prod_a", vec![rule.clone()]);

        let result = cache.get_rules("prod_a").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[test]
    fn test_get_rules_miss() {
        let cache = RuleCache::new();
        assert!(cache.get_rules("nonexistent").is_none());
    }

    #[test]
    fn test_invalidate_product() {
        let cache = RuleCache::new();
        cache.set_rules("prod_a", vec![make_rule(1, "prod_a")]);
        cache.invalidate_product("prod_a");
        assert!(cache.get_rules("prod_a").is_none());
    }

    #[test]
    fn test_invalidate_all() {
        let cache = RuleCache::new();
        cache.set_rules("prod_a", vec![make_rule(1, "prod_a")]);
        cache.set_rules("prod_b", vec![make_rule(2, "prod_b")]);
        cache.invalidate_all();
        assert!(cache.get_rules("prod_a").is_none());
        assert!(cache.get_rules("prod_b").is_none());
    }

    #[test]
    fn test_dedup_skip_within_window() {
        let cache = RuleCache::new();
        cache.mark_triggered(1, "device_1");

        // Manually set a recent time to simulate being within the window
        let key = "1:device_1".to_string();
        cache.dedup.insert(key, OffsetDateTime::now_utc());

        // Should be skipped because we just marked it
        assert!(cache.check_dedup(1, "device_1", 10));
    }

    #[test]
    fn test_dedup_pass_after_window() {
        let cache = RuleCache::new();

        // Set the last triggered time to 20 minutes ago
        let key = "1:device_1".to_string();
        let past = OffsetDateTime::now_utc() - Duration::minutes(20);
        cache.dedup.insert(key, past);

        // Should NOT be skipped because 20 > 10 minutes
        assert!(!cache.check_dedup(1, "device_1", 10));
    }

    #[test]
    fn test_dedup_zero_throttle_always_passes() {
        let cache = RuleCache::new();
        cache.mark_triggered(1, "device_1");

        // throttle_minutes=0 means no dedup
        assert!(!cache.check_dedup(1, "device_1", 0));
    }

    #[test]
    fn test_dedup_no_previous_trigger() {
        let cache = RuleCache::new();
        assert!(!cache.check_dedup(1, "device_1", 10));
    }

    // --- Duration tracking tests ---

    #[test]
    fn test_duration_first_condition_met_returns_just_started() {
        let cache = RuleCache::new();
        let now = OffsetDateTime::now_utc();
        let result = cache.check_duration(1, "device_1", 5, now);
        assert_eq!(result, DurationCheckResult::JustStarted);

        // Entry was stored
        let key = "1:device_1".to_string();
        assert!(cache.duration_tracking.get(&key).is_some());
    }

    #[test]
    fn test_duration_not_yet_met() {
        let cache = RuleCache::new();
        let now = OffsetDateTime::now_utc();
        // Start tracking
        cache.check_duration(1, "device_1", 5, now);

        // Check again 3 minutes later (not yet 5)
        let later = now + Duration::minutes(3);
        let result = cache.check_duration(1, "device_1", 5, later);
        assert_eq!(result, DurationCheckResult::NotYetMet);
    }

    #[test]
    fn test_duration_met_after_window() {
        let cache = RuleCache::new();
        let now = OffsetDateTime::now_utc();
        // Start tracking
        cache.check_duration(1, "device_1", 5, now);

        // Check again 6 minutes later (exceeds 5)
        let later = now + Duration::minutes(6);
        let result = cache.check_duration(1, "device_1", 5, later);
        assert_eq!(result, DurationCheckResult::Met);
    }

    #[test]
    fn test_duration_reset_clears_tracking() {
        let cache = RuleCache::new();
        let now = OffsetDateTime::now_utc();
        cache.check_duration(1, "device_1", 5, now);

        // Reset
        cache.reset_duration(1, "device_1");
        let key = "1:device_1".to_string();
        assert!(cache.duration_tracking.get(&key).is_none());

        // After reset, should start fresh (JustStarted)
        let result = cache.check_duration(1, "device_1", 5, now);
        assert_eq!(result, DurationCheckResult::JustStarted);
    }

    #[test]
    fn test_duration_full_cycle() {
        // Full cycle: first met -> not yet met -> condition breaks -> reset
        // -> condition met again -> duration passes -> Met
        let cache = RuleCache::new();
        let t0 = OffsetDateTime::now_utc();

        // 1. First condition met
        let result = cache.check_duration(1, "dev", 3, t0);
        assert_eq!(result, DurationCheckResult::JustStarted);

        // 2. Still not met (1 min later)
        let result = cache.check_duration(1, "dev", 3, t0 + Duration::minutes(1));
        assert_eq!(result, DurationCheckResult::NotYetMet);

        // 3. Condition breaks -> reset
        cache.reset_duration(1, "dev");

        // 4. Condition met again at t0+5
        let result = cache.check_duration(1, "dev", 3, t0 + Duration::minutes(5));
        assert_eq!(result, DurationCheckResult::JustStarted);

        // 5. Duration passes (4 minutes after re-start = t0+9)
        let result = cache.check_duration(1, "dev", 3, t0 + Duration::minutes(9));
        assert_eq!(result, DurationCheckResult::Met);
    }
}
