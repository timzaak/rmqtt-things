use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use time::OffsetDateTime;
use utoipa::ToSchema;

// 数据库模型
#[derive(Debug, FromRow, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct PropertyLatest {
    pub product_id: String,
    pub device_id: String,
    pub properties: JsonValue,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_time: OffsetDateTime,
}

#[derive(Debug, FromRow, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct PropertyHistory {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub properties: JsonValue,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reported_time: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_time: OffsetDateTime,
}

#[derive(Debug, FromRow, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct EventHistory {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub events: JsonValue,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reported_time: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_time: OffsetDateTime,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize, ToSchema,
)]
#[repr(i16)]
#[sqlx(type_name = "int2")]
pub enum CommandStatus {
    Pending = 0,
    Sent = 1,
    Success = 2,
    Failed = 3,
    Deleted = 4,
}

#[derive(Debug, FromRow, Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct PropertyCommand {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub command: JsonValue,
    pub status: CommandStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_time: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_time: OffsetDateTime,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize, ToSchema,
)]
#[repr(i16)]
#[sqlx(type_name = "int2")]
pub enum DeviceConnectionStatus {
    Offline = 0,
    Online = 1,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize, ToSchema,
)]
#[repr(i16)]
#[sqlx(type_name = "int2")]
pub enum RegistrationSource {
    Auto = 0,
    Manual = 1,
}

#[cfg(test)]
#[derive(Debug, FromRow, Serialize, ToSchema)]
pub struct Device {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub registration_source: RegistrationSource,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, FromRow, Serialize, ToSchema)]
pub struct DeviceStatusWithSource {
    pub product_id: String,
    pub device_id: String,
    /// None when device is registered but has never connected.
    pub status: Option<DeviceConnectionStatus>,
    pub ip_address: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_online_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_offline_at: Option<OffsetDateTime>,
    pub registration_source: RegistrationSource,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, FromRow, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct DeviceStatusHistory {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub status: DeviceConnectionStatus,
    pub ip_address: Option<String>,
    pub reason: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub connected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub disconnected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
pub struct CertIssue {
    pub id: i64,
    pub product_id: String,
    pub device_id: String,
    pub pub_cert: String,
    #[serde(with = "time::serde::rfc3339")]
    pub start_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub end_at: OffsetDateTime,
    pub status: CertStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize, ToSchema,
)]
#[repr(i16)]
#[sqlx(type_name = "int2")]
pub enum CertStatus {
    Normal = 0,
    InValid = 1,
    Revoked = 2,
}

impl TryFrom<i16> for CertStatus {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CertStatus::Normal),
            1 => Ok(CertStatus::InValid),
            2 => Ok(CertStatus::Revoked),
            _ => Err(()),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize, ToSchema,
)]
#[repr(i16)]
#[sqlx(type_name = "int2")]
pub enum EventValidTemplateStatus {
    Draft = 0,
    Active = 1,
    Inactive = 2,
}

#[derive(Debug, FromRow, Serialize, ToSchema)]
pub struct EventValidTemplate {
    pub id: i64,
    pub product_id: String,
    pub event: String,
    pub description: Option<String>,
    pub schema: JsonValue,
    pub status: EventValidTemplateStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "smallint")]
#[repr(i16)]
pub enum ProductStatus {
    Online = 0,
    Offline = 1,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub model_no: String,
    pub description: Option<String>,
    pub status: ProductStatus,
    pub auto_provisioning: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProductRequest {
    pub name: String,
    pub model_no: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProductRequest {
    pub name: String,
    pub description: String,
    pub auto_provisioning: bool,
}

#[derive(Debug, FromRow, Serialize, Deserialize, ToSchema, Clone)]
pub struct OtaVersion {
    pub id: i32,
    pub product_id: String,
    pub key: String,
    pub version: i32,
    pub max_version: Option<i32>,
    pub min_version: i32,
    pub file_key: String,
    pub log: Option<JsonValue>,
    pub device_ids: Option<Vec<String>>,
    #[serde(with = "time::serde::rfc3339")]
    pub released_at: OffsetDateTime,
    pub status: i16,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/*
#[derive(Debug, FromRow, Serialize, ToSchema)]
pub struct DeviceVersion {
    pub id: i32,
    pub product_id: String,
    pub device_id: String,
    pub key: String,
    pub version: i32,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_updated_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

 */

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
pub struct AlarmRule {
    pub id: i64,
    pub product_id: String,
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: JsonValue,
    pub condition: JsonValue,
    pub actions: JsonValue,
    pub enabled: bool,
    pub throttle_minutes: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
pub struct AlarmRecord {
    pub id: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub product_id: String,
    pub device_id: String,
    pub level: i16,
    pub message: Option<String>,
    pub trigger_value: Option<JsonValue>,
    pub acknowledged: bool,
    pub webhook_status: Option<i16>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use sqlx::FromRow;
    use time::OffsetDateTime;
    use utoipa::ToSchema;

    #[derive(Debug, FromRow, Serialize, ToSchema)]
    struct TimeSerde {
        #[serde(with = "time::serde::rfc3339")]
        pub created_at: OffsetDateTime,
    }

    #[test]
    fn test_time_serialization_and_deserialization() {
        let data = TimeSerde {
            created_at: OffsetDateTime::from_unix_timestamp(0).unwrap(),
        };
        let result = serde_json::to_string(&data).unwrap();
        println!("{result}");
    }
}
