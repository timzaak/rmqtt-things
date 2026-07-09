//! Shadow device support: pure delta computation.
//!
//! `compute_delta` compares a desired document (bare values) against a reported
//! snapshot and returns the per-property set of desired values that have not
//! yet converged. It is a pure in-memory function with no I/O, so the
//! Set-Desired / Get-Delta handlers (see `admin_handlers`) and the unit tests
//! below can exercise it directly.
//!
//! Conventions (design shadow-device-support.md §5.1):
//! - `desired` stores **bare** property values (no `{value, time}` wrapping).
//! - `reported` follows the `property_latest.properties` shape, where each
//!   property is wrapped as `{"value": v, "time": ts}`. The delta reads
//!   `reported[prop]["value"]` to compare against the bare desired value.
//! - A key appears in the delta when `reported` is missing it, or when the
//!   reported value differs from the desired value. The delta value is the
//!   **bare** desired value.
//! - An empty result means the device has fully converged.

use serde_json::Value as JsonValue;

/// Compute the per-property delta between `desired` (bare values) and
/// `reported` (the `property_latest.properties` `{value, time}` shape).
///
/// Returns a JSON object map whose entries are the desired keys that have not
/// converged, each mapped to its bare desired value. Empty when converged.
///
/// The function never returns `null`-valued entries: a desired value of `null`
/// is not a real desired state (it is the RFC 7396 "delete" sentinel handled
/// upstream by `merge_desired`), so it is intentionally skipped. This keeps
/// Get-Delta / Set-Desired responses free of spurious delta entries.
pub fn compute_delta(
    desired: &serde_json::Map<String, JsonValue>,
    reported: &serde_json::Map<String, JsonValue>,
) -> serde_json::Map<String, JsonValue> {
    let mut delta = serde_json::Map::new();
    for (key, dval) in desired.iter() {
        // `null` is the RFC 7396 delete sentinel, not a real desired value;
        // never surface it as a delta to push.
        if dval.is_null() {
            continue;
        }
        let diverged = match reported.get(key) {
            None => true,
            Some(obj) => obj.get("value") != Some(dval),
        };
        if diverged {
            delta.insert(key.clone(), dval.clone());
        }
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map_of(value: JsonValue) -> serde_json::Map<String, JsonValue> {
        match value {
            JsonValue::Object(map) => map,
            _ => panic!("expected JSON object"),
        }
    }

    #[test]
    fn delta_reports_missing_reported_property() {
        // reported missing the key entirely -> appears in delta with desired value.
        let desired = map_of(json!({"brightness": 80}));
        let reported = map_of(json!({}));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({"brightness": 80}));
    }

    #[test]
    fn delta_reports_when_values_differ() {
        // reported has the key but value differs -> appears in delta.
        let desired = map_of(json!({"brightness": 80}));
        let reported = map_of(json!({"brightness": {"value": 50, "time": "2026-07-09T00:00:00Z"}}));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({"brightness": 80}));
    }

    #[test]
    fn delta_empty_when_converged() {
        // reported value equals desired -> converged, no delta.
        let desired = map_of(json!({"brightness": 80, "mode": "eco"}));
        let reported = map_of(json!({
            "brightness": {"value": 80, "time": "2026-07-09T00:00:00Z"},
            "mode": {"value": "eco", "time": "2026-07-09T00:00:00Z"}
        }));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({}));
    }

    #[test]
    fn delta_empty_for_empty_desired() {
        // No desired keys -> trivially converged (handler rejects empty patch
        // before reaching compute_delta; this guards the Get-Delta path).
        let desired = map_of(json!({}));
        let reported = map_of(json!({"brightness": {"value": 80, "time": "2026-07-09T00:00:00Z"}}));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({}));
    }

    #[test]
    fn delta_partial_convergence_mixed() {
        // Mix of converged + divergent + missing keys -> only divergent/missing.
        let desired = map_of(json!({"brightness": 80, "mode": "eco", "color": "red"}));
        let reported = map_of(json!({
            "brightness": {"value": 80, "time": "2026-07-09T00:00:00Z"},
            "mode": {"value": "manual", "time": "2026-07-09T00:00:00Z"}
        }));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(
            JsonValue::Object(delta),
            json!({"mode": "eco", "color": "red"})
        );
    }

    #[test]
    fn delta_skips_null_desired_value() {
        // A null desired value is the RFC 7396 delete sentinel, not a real
        // desired state, and is handled by merge_desired upstream. compute_delta
        // must never surface it as a delta to push.
        let desired = map_of(json!({"brightness": null}));
        let reported = map_of(json!({}));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({}));
    }

    #[test]
    fn delta_treats_non_object_reported_entry_as_missing() {
        // Defensive: a malformed reported entry (not an object) is treated as
        // divergent so the desired value is surfaced for re-push.
        let desired = map_of(json!({"brightness": 80}));
        let reported = map_of(json!({"brightness": 80}));
        let delta = compute_delta(&desired, &reported);
        assert_eq!(JsonValue::Object(delta), json!({"brightness": 80}));
    }
}
