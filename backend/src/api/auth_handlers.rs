use crate::api::ApiState;
use axum::Json;
use axum::extract::State;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha1::Sha1;
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{info, warn};
use utoipa::ToSchema;

type HmacSha1 = Hmac<Sha1>;

#[derive(Deserialize, ToSchema, Debug)]
#[allow(dead_code)]
pub struct AuthPayload {
    // #[salvo(schema(max_length = 64))]
    pub client_id: String,
    // #[salvo(schema(max_length = 32))]
    #[serde(default)]
    pub username: Option<String>,
    // #[salvo(schema(max_length = 256))]
    #[serde(default)]
    pub password: String,
    pub protocol: serde_json::Value,
    pub ipaddress: String,
}

//const Subscribe:&str = "1";
// const Publish:&str = "2";

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
pub enum Access {
    #[serde(rename = "1")]
    Subscribe,
    #[serde[rename = "2"]]
    Publish,
}

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
pub enum MqttProtocol {
    #[serde(rename = "3")]
    Mqttv3,
    #[serde(rename = "4")]
    MqttV311,
    #[serde(rename = "5")]
    MqttV5,
}

#[derive(Deserialize, ToSchema, Debug)]
#[allow(dead_code)]
pub struct AclPayload {
    pub access: Access,
    #[serde(default)]
    pub username: Option<String>,
    pub client_id: String,
    pub ip: String,
    pub topic: String,
    pub protocol: serde_json::Value,
}

#[utoipa::path(
    post,
    path = "/api/access/acl",
    tag = "access",
    request_body = AclPayload,
    responses((status = 200, description = "allow or deny", body = String))
)]
pub async fn acl(Json(payload): Json<AclPayload>) -> &'static str {
    // Strip leading '/' if present (backend publishes OTA to /{pid}/{did}/ota/upgrade)
    let topic = payload.topic.strip_prefix('/').unwrap_or(&payload.topic);
    let mut parts = topic.split('/');
    let (p0, p1, p2, p3) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return "deny",
    };

    if p1 != payload.client_id {
        return "deny";
    }

    if let Some(username) = &payload.username
        && p0 != username
    {
        return "deny";
    }

    // Allow thing topics: {product}/{device}/thing/{event|service}/...
    if p2 == "thing" && (p3 == "event" || p3 == "service") {
        return "allow";
    }

    // Allow OTA topics: {product}/{device}/ota/upgrade or {product}/{device}/ota/version
    if p2 == "ota" && (p3 == "upgrade" || p3 == "version") {
        return "allow";
    }

    "deny"
}

#[utoipa::path(
    post,
    path = "/api/access/auth",
    tag = "access",
    request_body = AuthPayload,
    responses((status = 200, description = "allow or deny", body = String))
)]
pub async fn auth(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<AuthPayload>,
) -> &'static str {
    // Note: The check to see if a device is subscribed to its properties topic
    // is handled in the `create_property_command` function in `admin_handlers.rs`.
    // This is because we only need to check for the subscription when a command is being sent.
    // return "allow";
    let state = &state.app;
    let suffix = &state.config.mqtt.access.auth.suffix;

    // Validate input lengths
    if payload.client_id.len() > 64 || payload.password.len() > 256 || payload.password.is_empty() {
        return "deny";
    }

    // Deconstruct password
    let parts: Vec<&str> = payload.password.split('.').collect();
    if parts.len() != 3 {
        return "deny";
    }

    let nonce = parts[0];
    if nonce.len() != 6 {
        return "deny";
    }
    let timestamp_str = parts[1];
    let hash = parts[2];

    // Validate timestamp
    let timestamp: i64 = match timestamp_str.parse() {
        Ok(t) => t,
        Err(_) => return "deny",
    };

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let time_diff = (now - timestamp).abs();

    if time_diff > 300 {
        // 5 minutes
        warn!(
            clientid = %payload.client_id,
            time_diff = time_diff,
            "Timestamp out of range"
        );
        return "deny";
    }

    // Reconstruct and verify password
    let to_sign = format!(
        "{}.{}.{}.{}",
        payload.client_id, nonce, timestamp_str, suffix
    );

    let mac = HmacSha1::new_from_slice(suffix.as_bytes());
    let mac = match mac {
        Ok(mut mac) => {
            mac.update(to_sign.as_bytes());
            mac
        }
        Err(_) => return "deny",
    };
    let result = mac.finalize();
    let expected_hash = hex::encode(result.into_bytes());

    if expected_hash != hash {
        return "deny";
    }
    info!(client_id = %payload.client_id, "Authentication successful");
    "allow"
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex;
    use rand::distr::Alphanumeric;
    use rand::{Rng, rng};
    use serde_json::json;

    #[test]
    fn test_generate_password() {
        let client_id = "test_client";
        let suffix = "test_suffix";
        let (password, timestamp) = generate_test_password(client_id, suffix);

        let parts: Vec<&str> = password.split('.').collect();
        assert_eq!(parts.len(), 3);

        let nonce = parts[0];
        let timestamp_str = parts[1];
        let hash = parts[2];

        assert_eq!(nonce.len(), 6);
        assert_eq!(timestamp_str, timestamp.to_string());

        let to_sign = format!("{client_id}.{nonce}.{timestamp}.{suffix}");
        let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let result = mac.finalize();
        let expected_hash = hex::encode(result.into_bytes());

        assert_eq!(hash, expected_hash);
    }

    #[tokio::test]
    async fn test_acl_allows_device_thing_topic_for_own_product() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/demo-device/thing/event/property/post".to_string(),
            protocol: json!(4),
        };

        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_denies_cross_device_topic() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/other-device/thing/event/property/post".to_string(),
            protocol: json!(4),
        };

        assert_eq!(acl(Json(payload)).await, "deny");
    }

    #[tokio::test]
    async fn test_acl_allows_ota_upgrade_topic_with_leading_slash() {
        let payload = AclPayload {
            access: Access::Subscribe,
            username: Some("demo_product".to_string()),
            client_id: "demo-e2e-ota-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "/demo_product/demo-e2e-ota-device/ota/upgrade".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_allows_ota_version_topic() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/demo-device/ota/version".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_denies_ota_for_wrong_device() {
        let payload = AclPayload {
            access: Access::Subscribe,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "/demo_product/other-device/ota/upgrade".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "deny");
    }

    fn generate_test_password(client_id: &str, suffix: &str) -> (String, i64) {
        let nonce: String = rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();

        let to_sign = format!("{client_id}.{nonce}.{timestamp}.{suffix}");

        let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let result = mac.finalize();
        let hash = hex::encode(result.into_bytes());

        (format!("{nonce}.{timestamp}.{hash}"), timestamp)
    }
}
