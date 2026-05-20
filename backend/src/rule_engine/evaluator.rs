use serde_json::Value as JsonValue;

/// Trigger types that the rule engine can evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerType {
    Property,
    Event,
    DeviceOnline,
    DeviceOffline,
}

impl TriggerType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "property" => Some(TriggerType::Property),
            "event" => Some(TriggerType::Event),
            "device_online" => Some(TriggerType::DeviceOnline),
            "device_offline" => Some(TriggerType::DeviceOffline),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerType::Property => "property",
            TriggerType::Event => "event",
            TriggerType::DeviceOnline => "device_online",
            TriggerType::DeviceOffline => "device_offline",
        }
    }
}

pub struct RuleEvaluator;

impl RuleEvaluator {
    /// Evaluate a condition against an actual value.
    ///
    /// The `condition` JSON must contain an `"operator"` field.
    /// Supported operators: `>`, `>=`, `<`, `<=`, `==`, `!=`, `between`, `contains`, `always`.
    ///
    /// Returns `false` on type mismatch or malformed condition instead of panicking.
    pub fn evaluate(condition: &JsonValue, actual_value: &JsonValue) -> bool {
        let operator = match condition.get("operator").and_then(|v| v.as_str()) {
            Some(op) => op,
            None => return false,
        };

        match operator {
            ">" => Self::compare_ordered(condition, actual_value, |a, b| a > b, |a, b| a > b),
            ">=" => Self::compare_ordered(condition, actual_value, |a, b| a >= b, |a, b| a >= b),
            "<" => Self::compare_ordered(condition, actual_value, |a, b| a < b, |a, b| a < b),
            "<=" => Self::compare_ordered(condition, actual_value, |a, b| a <= b, |a, b| a <= b),
            "==" => {
                let expected = match condition.get("value") {
                    Some(v) => v,
                    None => return false,
                };
                if let Some(result) =
                    compare_f64(actual_value, expected, |a, b| (a - b).abs() < f64::EPSILON)
                {
                    return result;
                }
                if let Some(result) = compare_str(actual_value, expected, |a, b| a == b) {
                    return result;
                }
                actual_value == expected
            }
            "!=" => {
                let expected = match condition.get("value") {
                    Some(v) => v,
                    None => return false,
                };
                if let Some(result) =
                    compare_f64(actual_value, expected, |a, b| (a - b).abs() >= f64::EPSILON)
                {
                    return result;
                }
                if let Some(result) = compare_str(actual_value, expected, |a, b| a != b) {
                    return result;
                }
                actual_value != expected
            }
            "between" => {
                let min = match condition.get("min").and_then(json_to_f64) {
                    Some(v) => v,
                    None => return false,
                };
                let max = match condition.get("max").and_then(json_to_f64) {
                    Some(v) => v,
                    None => return false,
                };
                let actual_f64 = match json_to_f64(actual_value) {
                    Some(v) => v,
                    None => return false,
                };
                actual_f64 >= min && actual_f64 <= max
            }
            "contains" => {
                let pattern = match condition.get("value").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return false,
                };
                actual_value
                    .as_str()
                    .map(|s| s.contains(pattern))
                    .unwrap_or(false)
            }
            "always" => true,
            _ => false,
        }
    }

    /// Compare two values using both numeric (f64) and lexicographic (string) fallback.
    fn compare_ordered<FnF, FnS>(
        condition: &JsonValue,
        actual: &JsonValue,
        num_op: FnF,
        str_op: FnS,
    ) -> bool
    where
        FnF: Fn(f64, f64) -> bool,
        FnS: Fn(&str, &str) -> bool,
    {
        let threshold = match condition.get("value") {
            Some(v) => v,
            None => return false,
        };
        compare_f64(actual, threshold, num_op)
            .or_else(|| compare_str(actual, threshold, str_op))
            .unwrap_or(false)
    }
}

/// Convert a JSON value to f64 if possible.
fn json_to_f64(v: &JsonValue) -> Option<f64> {
    match v {
        JsonValue::Number(n) => n.as_f64(),
        JsonValue::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn compare_f64<F>(a: &JsonValue, b: &JsonValue, op: F) -> Option<bool>
where
    F: Fn(f64, f64) -> bool,
{
    let a_f64 = json_to_f64(a)?;
    let b_f64 = json_to_f64(b)?;
    Some(op(a_f64, b_f64))
}

fn compare_str<F>(a: &JsonValue, b: &JsonValue, op: F) -> Option<bool>
where
    F: Fn(&str, &str) -> bool,
{
    let a_str = a.as_str()?;
    let b_str = b.as_str()?;
    Some(op(a_str, b_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Greater than (>) ---
    #[test]
    fn test_gt_numeric_match() {
        let cond = json!({"operator": ">", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(60)));
    }

    #[test]
    fn test_gt_numeric_no_match() {
        let cond = json!({"operator": ">", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(40)));
    }

    #[test]
    fn test_gt_numeric_equal() {
        let cond = json!({"operator": ">", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    // --- Greater or equal (>=) ---
    #[test]
    fn test_gte_numeric_match() {
        let cond = json!({"operator": ">=", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    #[test]
    fn test_gte_numeric_no_match() {
        let cond = json!({"operator": ">=", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(49)));
    }

    // --- Less than (<) ---
    #[test]
    fn test_lt_numeric_match() {
        let cond = json!({"operator": "<", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(40)));
    }

    #[test]
    fn test_lt_numeric_no_match() {
        let cond = json!({"operator": "<", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    // --- Less or equal (<=) ---
    #[test]
    fn test_lte_numeric_match() {
        let cond = json!({"operator": "<=", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    #[test]
    fn test_lte_numeric_no_match() {
        let cond = json!({"operator": "<=", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(51)));
    }

    // --- Equal (==) ---
    #[test]
    fn test_eq_numeric_match() {
        let cond = json!({"operator": "==", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    #[test]
    fn test_eq_numeric_no_match() {
        let cond = json!({"operator": "==", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(51)));
    }

    #[test]
    fn test_eq_string_match() {
        let cond = json!({"operator": "==", "value": "hello"});
        assert!(RuleEvaluator::evaluate(&cond, &json!("hello")));
    }

    #[test]
    fn test_eq_string_no_match() {
        let cond = json!({"operator": "==", "value": "hello"});
        assert!(!RuleEvaluator::evaluate(&cond, &json!("world")));
    }

    #[test]
    fn test_eq_bool_match() {
        let cond = json!({"operator": "==", "value": true});
        assert!(RuleEvaluator::evaluate(&cond, &json!(true)));
    }

    // --- Not equal (!=) ---
    #[test]
    fn test_ne_numeric_match() {
        let cond = json!({"operator": "!=", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(51)));
    }

    #[test]
    fn test_ne_numeric_no_match() {
        let cond = json!({"operator": "!=", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    // --- Between ---
    #[test]
    fn test_between_match() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(30)));
    }

    #[test]
    fn test_between_match_at_min() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(10)));
    }

    #[test]
    fn test_between_match_at_max() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!(50)));
    }

    #[test]
    fn test_between_no_match_below() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(9)));
    }

    #[test]
    fn test_between_no_match_above() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(51)));
    }

    #[test]
    fn test_between_type_mismatch() {
        let cond = json!({"operator": "between", "min": 10, "max": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!("not a number")));
    }

    // --- Contains ---
    #[test]
    fn test_contains_match() {
        let cond = json!({"operator": "contains", "value": "error"});
        assert!(RuleEvaluator::evaluate(&cond, &json!("an error occurred")));
    }

    #[test]
    fn test_contains_no_match() {
        let cond = json!({"operator": "contains", "value": "error"});
        assert!(!RuleEvaluator::evaluate(&cond, &json!("all good")));
    }

    #[test]
    fn test_contains_non_string_actual() {
        let cond = json!({"operator": "contains", "value": "error"});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(42)));
    }

    // --- Always ---
    #[test]
    fn test_always() {
        let cond = json!({"operator": "always"});
        assert!(RuleEvaluator::evaluate(&cond, &json!(null)));
        assert!(RuleEvaluator::evaluate(&cond, &json!(42)));
        assert!(RuleEvaluator::evaluate(&cond, &json!("anything")));
    }

    // --- Edge cases ---
    #[test]
    fn test_missing_operator() {
        let cond = json!({"value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(60)));
    }

    #[test]
    fn test_unknown_operator() {
        let cond = json!({"operator": "unknown", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(60)));
    }

    #[test]
    fn test_type_mismatch_number_vs_string() {
        let cond = json!({"operator": ">", "value": 50});
        assert!(!RuleEvaluator::evaluate(&cond, &json!("not a number")));
    }

    #[test]
    fn test_string_value_parsed_as_number() {
        // String "60" should be parseable as f64 and compare against numeric threshold
        let cond = json!({"operator": ">", "value": 50});
        assert!(RuleEvaluator::evaluate(&cond, &json!("60")));
    }

    #[test]
    fn test_numeric_string_comparison_fallback() {
        // When actual is a string and value is a string, use string comparison
        let cond = json!({"operator": ">", "value": "b"});
        assert!(RuleEvaluator::evaluate(&cond, &json!("c")));
        assert!(!RuleEvaluator::evaluate(&cond, &json!("a")));
    }

    #[test]
    fn test_float_comparison() {
        let cond = json!({"operator": ">", "value": 10.5});
        assert!(RuleEvaluator::evaluate(&cond, &json!(10.6)));
        assert!(!RuleEvaluator::evaluate(&cond, &json!(10.4)));
    }

    #[test]
    fn test_missing_value_for_comparison() {
        let cond = json!({"operator": ">"});
        assert!(!RuleEvaluator::evaluate(&cond, &json!(60)));
    }
}
