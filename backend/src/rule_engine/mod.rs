pub mod actions;
pub mod cache;
pub mod evaluator;

pub use actions::{ActionExecutor, TriggerContext};
pub use cache::RuleCache;
pub use evaluator::{RuleEvaluator, TriggerType};

use crate::db::alarm::AlarmRepo;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

/// Top-level entry point for rule evaluation and action triggering.
///
/// Called from webhook handlers via `tokio::spawn`. It:
/// 1. Fetches rules from cache (or loads from DB on miss).
/// 2. Filters by trigger_type.
/// 3. For property rules, extracts the actual value for the configured property_name.
/// 4. For event rules, matches the event_identifier against top-level keys in trigger_value.
/// 5. Checks dedup.
/// 6. Evaluates the condition.
/// 7. On match, marks triggered and executes actions.
pub async fn evaluate_and_trigger(
    ctx: TriggerContext,
    alarm_repo: AlarmRepo,
    rule_cache: RuleCache,
) {
    let product_id = ctx.product_id.clone();

    // Get rules from cache, loading from DB on miss
    let rules = match rule_cache.get_rules(&product_id) {
        Some(rules) => rules,
        None => match alarm_repo.query_enabled_rules_by_product(&product_id).await {
            Ok(rules) => {
                rule_cache.set_rules(&product_id, rules.clone());
                rules
            }
            Err(e) => {
                warn!("Failed to query rules for product {}: {}", product_id, e);
                return;
            }
        },
    };

    for rule in &rules {
        // Filter by trigger type
        let rule_trigger_type = match TriggerType::from_str(&rule.trigger_type) {
            Some(tt) => tt,
            None => {
                debug!(
                    "Unknown trigger_type '{}' on rule {}",
                    rule.trigger_type, rule.id
                );
                continue;
            }
        };

        if rule_trigger_type != ctx.trigger_type {
            continue;
        }

        // Dedup check
        if rule_cache.check_dedup(rule.id, &ctx.device_id, rule.throttle_minutes as i64) {
            debug!(
                "Skipping rule {} for device {} due to dedup window",
                rule.id, ctx.device_id
            );
            continue;
        }

        // Extract the actual value to evaluate against
        let actual_value = match rule_trigger_type {
            TriggerType::Property => {
                let property_name = match rule
                    .trigger_config
                    .get("property_name")
                    .and_then(|v| v.as_str())
                {
                    Some(name) => name,
                    None => {
                        debug!(
                            "Property rule {} missing property_name in trigger_config",
                            rule.id
                        );
                        continue;
                    }
                };
                match extract_property_value(&ctx.trigger_value, property_name) {
                    Some(v) => v,
                    None => continue,
                }
            }
            TriggerType::Event => {
                let event_identifier = match rule
                    .trigger_config
                    .get("event_identifier")
                    .and_then(|v| v.as_str())
                {
                    Some(id) => id,
                    None => {
                        debug!(
                            "Event rule {} missing event_identifier in trigger_config",
                            rule.id
                        );
                        continue;
                    }
                };
                match ctx.trigger_value.get(event_identifier) {
                    Some(v) => v.clone(),
                    None => continue,
                }
            }
            TriggerType::DeviceOnline | TriggerType::DeviceOffline => {
                // No value extraction needed; condition "always" is typical
                JsonValue::Null
            }
        };

        // Evaluate condition
        if !RuleEvaluator::evaluate(&rule.condition, &actual_value) {
            continue;
        }

        // Matched -- mark triggered and execute actions
        debug!(
            "Rule {} matched for device {} on product {}",
            rule.id, ctx.device_id, ctx.product_id
        );

        rule_cache.mark_triggered(rule.id, &ctx.device_id);

        if let Err(e) = ActionExecutor::execute_actions(
            &rule.actions.as_array().cloned().unwrap_or_default(),
            &ctx,
            &alarm_repo,
            rule.id,
            &rule.name,
        )
        .await
        {
            warn!("Failed to execute actions for rule {}: {}", rule.id, e);
        }
    }
}

/// Extract a property value from the trigger payload.
///
/// The trigger_value for property events is the raw properties object.
/// The property may be nested as `{"property_name": {"value": ...}}` or flat as `{"property_name": value}`.
fn extract_property_value(trigger_value: &JsonValue, property_name: &str) -> Option<JsonValue> {
    let prop = trigger_value.get(property_name)?;
    // Check for nested {"value": ...} structure (from property_latest format)
    if let Some(inner_value) = prop.get("value") {
        Some(inner_value.clone())
    } else {
        Some(prop.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_property_value_flat() {
        let trigger = json!({"temperature": 42});
        let result = extract_property_value(&trigger, "temperature").unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn test_extract_property_value_nested() {
        let trigger = json!({"temperature": {"value": 42, "time": "2024-01-01T00:00:00Z"}});
        let result = extract_property_value(&trigger, "temperature").unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn test_extract_property_value_missing() {
        let trigger = json!({"humidity": 80});
        assert!(extract_property_value(&trigger, "temperature").is_none());
    }
}
