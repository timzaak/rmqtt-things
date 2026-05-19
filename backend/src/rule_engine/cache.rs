use crate::db::models::AlarmRule;
use dashmap::DashMap;
use time::OffsetDateTime;

/// Rule cache with DashMap-backed storage for rules (keyed by product_id)
/// and dedup tracking (keyed by "rule_id:device_id").
#[derive(Clone)]
pub struct RuleCache {
    rules: DashMap<String, Vec<AlarmRule>>,
    dedup: DashMap<String, OffsetDateTime>,
}

impl RuleCache {
    pub fn new() -> Self {
        Self {
            rules: DashMap::new(),
            dedup: DashMap::new(),
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

    /// Invalidate cached rules for a specific product.
    pub fn invalidate_product(&self, product_id: &str) {
        self.rules.remove(product_id);
    }

    /// Invalidate all cached rules.
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
}
