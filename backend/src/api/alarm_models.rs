use crate::api::admin_models::PaginatedResponse;
use crate::api::error::ApiError;
use crate::db::models::{AlarmRecord, AlarmRule};
use crate::rule_engine::evaluator::is_supported_operator;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    10
}

/// Query parameters for listing alarm rules
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AlarmRuleQuery {
    /// Filter by product ID
    pub product_id: Option<String>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
    /// Page number, default 1
    #[serde(default = "default_page")]
    pub page: i64,
    /// Page size, default 10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

/// Request body for creating an alarm rule
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateAlarmRuleRequest {
    /// Product ID (mapped to product.model_no)
    pub product_id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: Option<String>,
    /// Trigger type: property / event / device_online / device_offline
    pub trigger_type: String,
    /// Trigger configuration (e.g. property_name, event_identifier)
    #[serde(default = "default_trigger_config")]
    pub trigger_config: JsonValue,
    /// Condition expression
    pub condition: JsonValue,
    /// Action list (must contain at least one alarm action)
    pub actions: JsonValue,
    /// Dedup interval in minutes, 0 means no dedup
    #[serde(default)]
    pub throttle_minutes: i32,
    /// Duration condition in minutes, 0 = instant trigger, only for property trigger type
    #[serde(default)]
    pub duration_minutes: i32,
    /// Clear condition, same format as condition, only for property trigger type
    #[serde(default)]
    pub clear_condition: Option<JsonValue>,
}

fn default_trigger_config() -> JsonValue {
    JsonValue::Object(serde_json::Map::new())
}

/// Request body for updating an alarm rule
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAlarmRuleRequest {
    /// Rule name
    pub name: Option<String>,
    /// Rule description
    pub description: Option<String>,
    /// Trigger configuration
    pub trigger_config: Option<JsonValue>,
    /// Condition expression
    pub condition: Option<JsonValue>,
    /// Action list
    pub actions: Option<JsonValue>,
    /// Dedup interval in minutes
    pub throttle_minutes: Option<i32>,
    /// Duration condition in minutes, 0 = instant trigger, only for property trigger type
    pub duration_minutes: Option<i32>,
    /// Clear condition, same format as condition. Some(None) = clear to null, None = do not update.
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub clear_condition: Option<Option<JsonValue>>,
}

/// Deserialize an optional optional field: absent -> None, null -> Some(None), value -> Some(Some(value)).
fn deserialize_double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    // Use a visitor that distinguishes absent (unit) from null from present value.
    struct DoubleOptionVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> serde::de::Visitor<'de> for DoubleOptionVisitor<T>
    where
        T: serde::Deserialize<'de>,
    {
        type Value = Option<Option<T>>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "a nullable value or nothing")
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Option<Option<T>>, E> {
            // Field was absent (unit) -> outer None
            Ok(None)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Option<Option<T>>, E> {
            // Explicit null -> Some(None) means "clear to null"
            Ok(Some(None))
        }

        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            de: D,
        ) -> Result<Option<Option<T>>, D::Error> {
            // Non-null value -> Some(Some(value))
            T::deserialize(de).map(|v| Some(Some(v)))
        }
    }

    de.deserialize_option(DoubleOptionVisitor(std::marker::PhantomData))
}

/// Request body for enabling/disabling an alarm rule
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAlarmRuleStatusRequest {
    /// Whether the rule is enabled
    pub enabled: bool,
}

/// Single alarm rule response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct AlarmRuleResponse {
    pub data: AlarmRule,
}

/// Paginated alarm rule list response
pub type AlarmRuleListResponse = PaginatedResponse<AlarmRule>;

// --- Alarm Record DTOs ---

/// Query parameters for listing alarm records
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AlarmQuery {
    /// Filter by product ID
    pub product_id: Option<String>,
    /// Filter by device ID
    pub device_id: Option<String>,
    /// Filter by level: info / warning / critical
    pub level: Option<String>,
    /// Filter by acknowledged status
    pub acknowledged: Option<bool>,
    /// Filter by status: active / acknowledged / cleared
    #[serde(default)]
    pub status: Option<String>,
    /// Page number, default 1
    #[serde(default = "default_page")]
    pub page: i64,
    /// Page size, default 10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

/// API-level alarm record with string level and webhook_status
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiAlarmRecord {
    pub id: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub product_id: String,
    pub device_id: String,
    /// Alarm level: "info" / "warning" / "critical"
    pub level: String,
    pub message: Option<String>,
    pub trigger_value: Option<JsonValue>,
    pub acknowledged: bool,
    /// Alarm status: "active" / "acknowledged" / "cleared"
    pub status: String,
    /// Webhook status: None = not configured, Some("success") / Some("failed")
    pub webhook_status: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
    /// RFC3339 timestamp when alarm was cleared
    pub cleared_at: Option<String>,
}

impl From<AlarmRecord> for ApiAlarmRecord {
    fn from(record: AlarmRecord) -> Self {
        let level = match record.level {
            0 => "info".to_string(),
            1 => "warning".to_string(),
            _ => "critical".to_string(),
        };
        let webhook_status = match record.webhook_status {
            None => None,
            Some(0) => Some("success".to_string()),
            Some(_) => Some("failed".to_string()),
        };
        let cleared_at = record.cleared_at.and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        });
        Self {
            id: record.id,
            rule_id: record.rule_id,
            rule_name: record.rule_name,
            product_id: record.product_id,
            device_id: record.device_id,
            level,
            message: record.message,
            trigger_value: record.trigger_value,
            acknowledged: record.status != "active",
            status: record.status,
            webhook_status,
            created_at: record.created_at,
            cleared_at,
        }
    }
}

/// Single alarm record response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct AlarmRecordResponse {
    pub data: ApiAlarmRecord,
}

/// Paginated alarm record list response
pub type AlarmRecordListResponse = PaginatedResponse<ApiAlarmRecord>;

// ===========================================================================
// Rule request validation (P1-3 audit fix)
// ===========================================================================
//
// PRD: docs/prd/core/alarm-rule-engine.md §5.2 requires that a correctly
// configured rule fires within 2 seconds of threshold crossing. Before this
// validation the API silently accepted rules with unknown operators, empty
// action lists, or property/event triggers missing their identifying field —
// such rules would never fire, hiding misconfiguration until runtime.
//
// These functions are pure (no DB) so they can be unit-tested directly.

/// Validate a condition JSON object. A condition is valid when:
/// - It is a JSON object containing an `operator` field that is a string
///   belonging to [`SUPPORTED_OPERATORS`] (see rule_engine::evaluator).
///
/// Empty objects or objects missing `operator` are rejected, because the rule
/// engine evaluates such conditions as always-false (silent misconfiguration).
pub fn validate_condition(condition: &JsonValue, field_name: &str) -> Result<(), ApiError> {
    let obj = condition
        .as_object()
        .ok_or_else(|| ApiError::bad_request(format!("{field_name} must be a JSON object")))?;
    let operator = obj
        .get("operator")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "{field_name} must contain an 'operator' string field"
            ))
        })?;
    if !is_supported_operator(operator) {
        return Err(ApiError::bad_request(format!(
            "{field_name} has unsupported operator '{operator}'. Must be one of: >, >=, <, <=, ==, !=, between, contains, always"
        )));
    }
    Ok(())
}

/// Validate the `actions` JSON value. The rule engine (rule_engine::actions)
/// recognizes actions with `type: "alarm"` or `type: "webhook"`. PRD
/// alarm-rule-engine requires that a rule produces at least one alarm when
/// triggered, so actions must be a non-empty array containing at least one
/// entry with `type: "alarm"`.
pub fn validate_actions(actions: &JsonValue) -> Result<(), ApiError> {
    let arr = actions
        .as_array()
        .ok_or_else(|| ApiError::bad_request("actions must be a JSON array"))?;
    if arr.is_empty() {
        return Err(ApiError::bad_request(
            "actions must contain at least one alarm action",
        ));
    }
    let has_alarm = arr
        .iter()
        .any(|a| a.get("type").and_then(|v| v.as_str()) == Some("alarm"));
    if !has_alarm {
        return Err(ApiError::bad_request(
            "actions must contain at least one alarm action",
        ));
    }
    Ok(())
}

/// Validate trigger-config requirements for a given trigger type.
/// - property: `trigger_config.property_name` must be a non-empty string.
/// - event: `trigger_config.event_identifier` must be a non-empty string.
/// - device_online / device_offline: no additional required fields.
pub fn validate_trigger_config(
    trigger_type: &str,
    trigger_config: &JsonValue,
) -> Result<(), ApiError> {
    let obj = trigger_config
        .as_object()
        .ok_or_else(|| ApiError::bad_request("trigger_config must be a JSON object"))?;
    match trigger_type {
        "property" => {
            let name = obj
                .get("property_name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "trigger_config.property_name is required for property trigger type",
                    )
                })?;
            validate_identifier_safe(name, "property_name")?;
        }
        "event" => {
            let id = obj
                .get("event_identifier")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "trigger_config.event_identifier is required for event trigger type",
                    )
                })?;
            validate_identifier_safe(id, "event_identifier")?;
        }
        "device_online" | "device_offline" => {}
        _ => {
            return Err(ApiError::bad_request(format!(
                "Invalid trigger_type '{trigger_type}'. Must be one of: property, event, device_online, device_offline"
            )));
        }
    }
    Ok(())
}

/// Local identifier check (non-empty, <=128 chars, [A-Za-z0-9_-]) for
/// property_name / event_identifier. Mirrors api::utils::validate_identifier
/// but kept here to avoid a cross-module dependency for two short checks.
fn validate_identifier_safe(id: &str, field_name: &str) -> Result<(), ApiError> {
    if id.len() > 128 {
        return Err(ApiError::bad_request(format!(
            "{field_name} must not exceed 128 characters"
        )));
    }
    if id
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
    {
        return Err(ApiError::bad_request(format!(
            "{field_name} contains invalid character. Only alphanumeric, '-' and '_' are allowed"
        )));
    }
    Ok(())
}

/// Validate the full create-rule request body. Pure function (no DB).
pub fn validate_create_rule_request(req: &CreateAlarmRuleRequest) -> Result<(), ApiError> {
    validate_trigger_config(&req.trigger_type, &req.trigger_config)?;
    validate_condition(&req.condition, "condition")?;
    if let Some(ref clear) = req.clear_condition {
        validate_condition(clear, "clear_condition")?;
    }
    validate_actions(&req.actions)?;
    Ok(())
}

/// Allowed values for `alarm.status` (PRD alarm-rule-check.md §4.2 — three-state
/// lifecycle). Kept in sync with the `alarm_status_check` DB constraint.
pub const ALARM_STATUSES: &[&str] = &["active", "acknowledged", "cleared"];

/// Returns Ok(()) when `status` is one of [`ALARM_STATUSES`] or None;
/// returns 400 otherwise. Used by the list_alarms query parser.
pub fn validate_alarm_status(status: Option<&str>) -> Result<(), ApiError> {
    if let Some(s) = status
        && !ALARM_STATUSES.contains(&s)
    {
        return Err(ApiError::bad_request(format!(
            "Invalid status '{s}'. Must be one of: active, acknowledged, cleared"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod rule_validation_tests {
    use super::*;
    use serde_json::json;

    fn base_property_req() -> CreateAlarmRuleRequest {
        CreateAlarmRuleRequest {
            product_id: "p1".into(),
            name: "n".into(),
            description: None,
            trigger_type: "property".into(),
            trigger_config: json!({ "property_name": "temperature" }),
            condition: json!({ "operator": ">", "value": 50 }),
            actions: json!([{ "type": "alarm", "level": "warning" }]),
            throttle_minutes: 0,
            duration_minutes: 0,
            clear_condition: None,
        }
    }

    // --- validate_condition -------------------------------------------------

    #[test]
    fn condition_rejects_unknown_operator() {
        let err = validate_condition(&json!({"operator": "_APPROX", "value": 1}), "condition")
            .unwrap_err();
        // 400 Bad Request
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn condition_rejects_missing_operator() {
        let err = validate_condition(&json!({"value": 1}), "condition").unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn condition_rejects_non_object() {
        let err = validate_condition(&json!([1, 2, 3]), "condition").unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn condition_accepts_always_operator() {
        assert!(validate_condition(&json!({"operator": "always"}), "condition").is_ok());
    }

    #[test]
    fn condition_accepts_between_operator() {
        assert!(
            validate_condition(
                &json!({"operator": "between", "min": 0, "max": 10}),
                "condition"
            )
            .is_ok()
        );
    }

    // --- validate_actions ---------------------------------------------------

    #[test]
    fn actions_rejects_empty_array() {
        let err = validate_actions(&json!([])).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn actions_rejects_no_alarm_action() {
        // Only a webhook action — rule would never raise an alarm.
        let err = validate_actions(&json!([{ "type": "webhook", "url": "http://x" }])).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn actions_rejects_non_array() {
        let err = validate_actions(&json!({"type": "alarm"})).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn actions_accepts_with_alarm_action() {
        assert!(validate_actions(&json!([{ "type": "alarm", "level": "info" }])).is_ok());
    }

    // --- validate_trigger_config -------------------------------------------

    #[test]
    fn trigger_property_requires_property_name() {
        let err = validate_trigger_config("property", &json!({})).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn trigger_event_requires_event_identifier() {
        let err = validate_trigger_config("event", &json!({})).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn trigger_property_rejects_empty_property_name() {
        let err = validate_trigger_config("property", &json!({"property_name": ""})).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn trigger_device_online_needs_no_fields() {
        assert!(validate_trigger_config("device_online", &json!({})).is_ok());
    }

    #[test]
    fn trigger_invalid_type_rejected() {
        let err = validate_trigger_config("pressure", &json!({})).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    // --- validate_create_rule_request (integration of above) ---------------

    #[test]
    fn create_request_valid_payload_passes() {
        assert!(validate_create_rule_request(&base_property_req()).is_ok());
    }

    #[test]
    fn create_request_invalid_operator_rejected() {
        let mut req = base_property_req();
        req.condition = json!({"operator": "BOGUS", "value": 1});
        assert_eq!(
            err_status(validate_create_rule_request(&req).unwrap_err()),
            400
        );
    }

    #[test]
    fn create_request_empty_actions_rejected() {
        let mut req = base_property_req();
        req.actions = json!([]);
        assert_eq!(
            err_status(validate_create_rule_request(&req).unwrap_err()),
            400
        );
    }

    #[test]
    fn create_request_property_missing_property_name_rejected() {
        let mut req = base_property_req();
        req.trigger_config = json!({});
        assert_eq!(
            err_status(validate_create_rule_request(&req).unwrap_err()),
            400
        );
    }

    #[test]
    fn create_request_event_missing_identifier_rejected() {
        let mut req = base_property_req();
        req.trigger_type = "event".into();
        req.trigger_config = json!({});
        assert_eq!(
            err_status(validate_create_rule_request(&req).unwrap_err()),
            400
        );
    }

    #[test]
    fn create_request_clear_condition_invalid_operator_rejected() {
        let mut req = base_property_req();
        req.clear_condition = Some(json!({"operator": "BOGUS"}));
        assert_eq!(
            err_status(validate_create_rule_request(&req).unwrap_err()),
            400
        );
    }

    // Helper: extract the status code from an ApiError by value via
    // IntoResponse. ApiError's status field is private, so the only way to
    // observe it publicly is to render the error into a Response.
    fn err_status(err: ApiError) -> u16 {
        use axum::response::IntoResponse;
        err.into_response().status().as_u16()
    }

    // --- validate_alarm_status (P1-11) -------------------------------------

    #[test]
    fn alarm_status_none_is_ok() {
        assert!(validate_alarm_status(None).is_ok());
    }

    #[test]
    fn alarm_status_active_is_ok() {
        assert!(validate_alarm_status(Some("active")).is_ok());
        assert!(validate_alarm_status(Some("acknowledged")).is_ok());
        assert!(validate_alarm_status(Some("cleared")).is_ok());
    }

    #[test]
    fn alarm_status_unknown_rejected() {
        let err = validate_alarm_status(Some("foo")).unwrap_err();
        assert_eq!(err_status(err), 400);
    }

    #[test]
    fn alarm_status_case_sensitive() {
        // Status values are written verbatim to the DB column (CHECK
        // constraint is case-sensitive); "Active" must be rejected.
        let err = validate_alarm_status(Some("Active")).unwrap_err();
        assert_eq!(err_status(err), 400);
    }
}
