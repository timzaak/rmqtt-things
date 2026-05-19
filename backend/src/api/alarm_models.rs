use crate::api::admin_models::PaginatedResponse;
use crate::db::models::{AlarmRecord, AlarmRule};
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
    /// Webhook status: None = not configured, Some("success") / Some("failed")
    pub webhook_status: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
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
        Self {
            id: record.id,
            rule_id: record.rule_id,
            rule_name: record.rule_name,
            product_id: record.product_id,
            device_id: record.device_id,
            level,
            message: record.message,
            trigger_value: record.trigger_value,
            acknowledged: record.acknowledged,
            webhook_status,
            created_at: record.created_at,
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
