use crate::db::models::CommandStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_repr::{Deserialize_repr, Serialize_repr};
use time::OffsetDateTime;
use utoipa::ToSchema;

// MQTT 消息结构
#[derive(Debug, Default, Deserialize, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct RMqttPublishMessage {
    pub dup: bool,
    #[serde(rename = "from_clientid")]
    pub client_id: String,
    #[serde(rename = "from_ipaddress")]
    pub ip_address: String,
    #[serde(rename = "from_type")]
    pub from_type: String,
    #[serde(rename = "from_username")]
    #[serde(default)]
    pub username: String,
    #[serde(rename = "packet_id")]
    pub packet_id: i32,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
    pub time: String,
    pub topic: String,
    pub ts: i64,
}

// MQTT 订阅消息结构 (类似 Java 的 RMqttSubscribeMessage)
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct RMqttSubscribeMessage {
    #[serde(rename = "clean_start")]
    pub clean_start: Option<bool>,
    #[serde(rename = "clientid")]
    pub client_id: String,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(rename = "connected_at")]
    pub connected_at: Option<i64>,
    #[serde(rename = "ipaddress")]
    pub ip_address: Option<String>,
    #[serde(rename = "keepalive")]
    pub keep_alive: Option<i32>,
    pub node: Option<i32>,
    #[serde(rename = "proto_ver")]
    pub proto_ver: Option<i32>,
    #[serde(rename = "session_present")]
    pub session_present: Option<bool>,
    pub time: Option<String>,
    pub username: Option<String>,
}

impl RMqttPublishMessage {
    pub fn decode_payload(&self) -> anyhow::Result<Vec<u8>> {
        use base64::{Engine as _, engine::general_purpose};
        Ok(general_purpose::STANDARD.decode(&self.payload)?)
    }

    pub fn decode_payload_as_json(&self) -> anyhow::Result<MqttPayload> {
        let bytes = self.decode_payload()?;
        let json_str = String::from_utf8(bytes)?;
        Ok(serde_json::from_str(&json_str)?)
    }
}

// MQTT Payload 结构
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MqttPayload {
    pub id: String,
    pub ack: AckStatus, // 0 或 1
    pub params: Option<JsonValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, ToSchema)]
#[repr(u8)]
pub enum AckStatus {
    No = 0,
    // Needs Response
    Yes = 1,
}

// 响应结构
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MqttResponse {
    pub id: String,
    pub code: i32,
    pub data: Option<JsonValue>,
}

// API 请求结构
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct PropertySetRequest {
    pub product_id: String,
    pub device_id: String,
    pub properties: JsonValue,
    #[serde(with = "time::serde::rfc3339::option")]
    pub timestamp: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EventPostRequest {
    pub product_id: String,
    pub device_id: String,
    pub events: JsonValue,
    #[serde(with = "time::serde::rfc3339::option")]
    pub timestamp: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PropertyPostRequest {
    pub product_id: String,
    pub device_id: String,
    pub properties: JsonValue,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PropertyPostReplyRequest {
    pub product_id: String,
    pub device_id: String,
    pub command_id: i64,
    pub status: CommandStatus,
    pub result: Option<JsonValue>,
}

// API for device status

// {"node":1,"ipaddress":"172.17.0.1:54592","clientid":"X1241801234567","username":"undefined","keepalive":60,"proto_ver":5,"clean_start":true,
// "connected_at":1756261355844,
// "session_present":false,"time":"2025-08-27 02:22:35.844","action":"client_connected"}
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct DeviceConnectRequest {
    pub node: i64,
    pub ipaddress: String,
    #[serde(rename = "clientid", default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub product_id: String,
    #[serde(default)]
    pub device_id: String,
    pub username: Option<String>,
    pub keepalive: u16,
    pub proto_ver: u8,
    #[serde(alias = "clean_session", default)]
    pub clean_start: bool,
    pub connected_at: i64, // Unix timestamp
    pub session_present: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct DeviceDisconnectRequest {
    pub node: i64,
    pub ipaddress: Option<String>,
    #[serde(rename = "clientid", default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub product_id: String,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(rename = "disconnected_at")]
    pub disconnected_at: i64, // Unix timestamp
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FileUploadRequest {
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub directory: String,
    #[serde(rename = "useOriginName")]
    pub use_origin_name: bool,
    #[serde(rename = "fileType")]
    pub file_type: String,
}

use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FileUploadResponse {
    pub url: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OtaReport {
    pub key: String,
    pub version: i32,
}

#[cfg(test)]
mod test {
    use crate::api::web_models::MqttPayload;

    #[test]
    fn test_decode_mqtt_payload() {
        let json_str = r#"{
            "id":"test_id1234",
            "params":{"testEvent":"test_value"},
            "ack":1
        }"#;
        let data: MqttPayload = serde_json::from_str(json_str).unwrap();
        println!("{data:?}");
    }
}
