use crate::api::error::ApiError;
use crate::api::web_models::{AckStatus, MqttPayload};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::{PublishRequest, RmqttHttpClient};
use serde_json::Value as JsonValue;
use tracing::info;

// 辅助函数：向设备发送属性命令
pub async fn send_property_command_to_device(
    db: &DatabaseService,
    rmqtt_client: &RmqttHttpClient,
    product_id: &str,
    device_id: &str,
) -> anyhow::Result<()> {
    // 更新 pending 状态的命令为 sent，并获取命令信息
    let commands = db
        .update_pending_commands_to_sent(product_id, device_id)
        .await?;

    if commands.is_empty() {
        info!("No pending commands found for device: {}", device_id);
        return Ok(());
    }

    // 按 created_at 合并命令，保证最新值存在
    let mut merged_command = serde_json::Map::new();
    let mut command_ids = Vec::new();

    for (id, command, _created_time) in commands {
        command_ids.push(id);

        if let JsonValue::Object(cmd_obj) = command {
            for (key, value) in cmd_obj {
                merged_command.insert(key, value);
            }
        }
    }

    // 构造下发指令
    let config = &rmqtt_client.config;
    let publish_config = &config.property_command.publish;

    let topic = publish_config
        .topic
        .replace("${productId}", product_id)
        .replace("$clientid", device_id);

    let command_payload = MqttPayload {
        id: uuid::Uuid::new_v4().to_string(),
        ack: AckStatus::Yes,
        params: Some(serde_json::json!({
            "ids": command_ids,
            "data": JsonValue::Object(merged_command.clone())
        })),
    };

    let publish_request = PublishRequest {
        topic,
        clientid: publish_config.clientid.clone(),
        payload: serde_json::to_string(&command_payload)?,
        encoding: Some("plain".to_string()),
        qos: Some(publish_config.qos),
        retain: Some(publish_config.retain),
    };

    // 发布命令到 RMQTT
    if let Err(e) = rmqtt_client.publish_command(publish_request).await {
        // 如果发布失败，将命令状态更新为 Failed
        db.update_sent_commands_to_failed(&command_ids).await?;
        return Err(e);
    }

    info!(
        "Published property command to device: {}, command_ids: {:?}",
        device_id, command_ids
    );

    Ok(())
}

pub fn extract_product_id_from_topic(topic: &str) -> Option<String> {
    if !topic.starts_with('/') && !topic.contains('/') {
        return None;
    }
    topic
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Extract the event identifier from a thing/event topic of the form
/// `/{product}/{device}/thing/event/{event_identifier}/post`.
/// Returns None when the topic does not match the expected segment layout.
pub fn extract_event_identifier_from_topic(topic: &str) -> Option<String> {
    let mut parts = topic.trim_start_matches('/').split('/');
    // expected: {product}, {device}, "thing", "event", {event_identifier}, "post"
    match (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) {
        (Some(_), Some(_), Some(seg3), Some(seg4), Some(event_id))
            if seg3 == "thing" && seg4 == "event" && !event_id.is_empty() =>
        {
            Some(event_id.to_string())
        }
        _ => None,
    }
}

const MAX_IDENTIFIER_LENGTH: usize = 128;

pub fn validate_identifier(id: &str, field_name: &str) -> Result<(), ApiError> {
    if id.is_empty() {
        return Err(ApiError::bad_request(format!(
            "{field_name} must not be empty"
        )));
    }
    if id.len() > MAX_IDENTIFIER_LENGTH {
        return Err(ApiError::bad_request(format!(
            "{field_name} must not exceed {MAX_IDENTIFIER_LENGTH} characters"
        )));
    }
    if let Some(ch) = id
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_')
    {
        return Err(ApiError::bad_request(format!(
            "{field_name} contains invalid character '{ch}'. Only alphanumeric characters, hyphens, and underscores are allowed"
        )));
    }
    Ok(())
}

/// Validate that a version string follows the x.y.z format (3 dot-separated numeric parts).
pub fn validate_version_format(version: &str, field_name: &str) -> Result<(), ApiError> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::bad_request(format!(
            "Invalid {field_name} format '{version}': must be x.y.z (e.g. 1.2.3)"
        )));
    }
    for (i, part) in parts.iter().enumerate() {
        if part.parse::<u32>().is_err() {
            return Err(ApiError::bad_request(format!(
                "Invalid {field_name} format '{version}': must be x.y.z (e.g. 1.2.3)"
            )));
        }
        // Match the DB layer limits: major/minor <= 99, patch <= 999
        let max = if i < 2 { 99 } else { 999 };
        let value: u32 = part.parse().unwrap();
        if value > max {
            return Err(ApiError::bad_request(format!(
                "Invalid {field_name} format '{version}': each part exceeds allowed range"
            )));
        }
    }
    Ok(())
}

pub fn extract_and_validate_product_id(topic: &str) -> Result<String, ApiError> {
    let product_id = extract_product_id_from_topic(topic)
        .ok_or_else(|| ApiError::bad_request("Product ID not found in topic"))?;
    validate_identifier(&product_id, "product_id")?;
    Ok(product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_product_id_from_topic() {
        let topic = "/product1/device1/thing/property/post";
        assert_eq!(
            extract_product_id_from_topic(topic),
            Some("product1".to_string())
        );

        let topic = "product1/device1/thing/property/post";
        assert_eq!(
            extract_product_id_from_topic(topic),
            Some("product1".to_string())
        );

        let topic = "/product1";
        assert_eq!(
            extract_product_id_from_topic(topic),
            Some("product1".to_string())
        );

        let topic = "product1";
        assert_eq!(extract_product_id_from_topic(topic), None);

        let topic = "";
        assert_eq!(extract_product_id_from_topic(topic), None);
    }

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("product1", "product_id").is_ok());
        assert!(validate_identifier("my-product-123", "product_id").is_ok());
        assert!(validate_identifier("device_001", "device_id").is_ok());
        assert!(validate_identifier("ABC", "product_id").is_ok());
    }

    #[test]
    fn test_validate_identifier_rejects_slash() {
        assert!(validate_identifier("prod/evil", "product_id").is_err());
    }

    #[test]
    fn test_validate_identifier_rejects_backslash() {
        assert!(validate_identifier("prod\\evil", "product_id").is_err());
    }

    #[test]
    fn test_validate_identifier_rejects_dot() {
        assert!(validate_identifier("device.v1", "device_id").is_err());
    }

    #[test]
    fn test_validate_identifier_rejects_space() {
        assert!(validate_identifier("my product", "product_id").is_err());
    }

    #[test]
    fn test_validate_identifier_rejects_empty() {
        assert!(validate_identifier("", "product_id").is_err());
    }

    #[test]
    fn test_validate_identifier_rejects_too_long() {
        let long_id = "a".repeat(129);
        assert!(validate_identifier(&long_id, "product_id").is_err());
    }

    #[test]
    fn test_validate_identifier_accepts_max_length() {
        let max_id = "a".repeat(128);
        assert!(validate_identifier(&max_id, "product_id").is_ok());
    }

    #[test]
    fn test_validate_identifier_rejects_mqtt_wildcards() {
        assert!(validate_identifier("prod#evil", "product_id").is_err());
        assert!(validate_identifier("prod+evil", "product_id").is_err());
    }

    #[test]
    fn test_extract_and_validate_product_id_valid() {
        let result = extract_and_validate_product_id("/good-product/dev1/thing/property/post");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "good-product");
    }

    #[test]
    fn test_extract_and_validate_product_id_rejects_invalid() {
        let result = extract_and_validate_product_id("/bad.product/dev1/thing/property/post");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_event_identifier_from_topic() {
        // Standard thing/event topic: /{product}/{device}/thing/event/{id}/post
        assert_eq!(
            extract_event_identifier_from_topic("/demo_product/dev1/thing/event/alert/post"),
            Some("alert".to_string())
        );
        assert_eq!(
            extract_event_identifier_from_topic("/p/d/thing/event/property/post"),
            Some("property".to_string())
        );
        // Tolerates topics without leading slash
        assert_eq!(
            extract_event_identifier_from_topic("p/d/thing/event/alert/post"),
            Some("alert".to_string())
        );
    }

    #[test]
    fn test_extract_event_identifier_from_topic_rejects_non_event() {
        // Wrong segment 4 (not "event")
        assert_eq!(
            extract_event_identifier_from_topic("/p/d/thing/property/post"),
            None
        );
        // Wrong segment 3 (not "thing")
        assert_eq!(
            extract_event_identifier_from_topic("/p/d/ota/event/alert/post"),
            None
        );
        // Too few segments
        assert_eq!(
            extract_event_identifier_from_topic("/p/d/thing/event"),
            None
        );
        assert_eq!(extract_event_identifier_from_topic(""), None);
    }

    #[test]
    fn test_validate_version_format_accepts_valid() {
        assert!(validate_version_format("1.2.3", "version").is_ok());
        assert!(validate_version_format("0.0.0", "version").is_ok());
        assert!(validate_version_format("99.99.999", "version").is_ok());
    }

    #[test]
    fn test_validate_version_format_rejects_too_few_parts() {
        assert!(validate_version_format("1.2", "version").is_err());
        assert!(validate_version_format("1", "version").is_err());
    }

    #[test]
    fn test_validate_version_format_rejects_too_many_parts() {
        assert!(validate_version_format("1.2.3.4", "version").is_err());
    }

    #[test]
    fn test_validate_version_format_rejects_non_numeric() {
        assert!(validate_version_format("1.a.3", "version").is_err());
        assert!(validate_version_format("v1.2.3", "version").is_err());
    }

    #[test]
    fn test_validate_version_format_rejects_out_of_range() {
        assert!(validate_version_format("100.0.0", "version").is_err());
        assert!(validate_version_format("0.100.0", "version").is_err());
        assert!(validate_version_format("0.0.1000", "version").is_err());
    }
}
