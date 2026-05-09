//! Scenario tests for MQTT device flow, mirroring demo/e2e/mqtt-device-flow-demo.e2e.ts.
//!
//! Covers:
//! - Property upload via webhook → query via admin API
//! - Event upload via webhook → query via admin API
//! - Property command lifecycle: create → send → reply → status = Success
//! - Full integrated device flow

use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use crate::api::admin_models::CreatePropertyCommandRequest;
use crate::api::web_models::RMqttPublishMessage;
use axum::http::{Method, StatusCode};
use base64::Engine;
use serde_json::json;
use test_context::test_context;

fn encode_payload(value: &serde_json::Value) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

fn property_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/post")
}

fn event_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/test/post")
}

fn property_set_reply_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/reply")
}

fn mqtt_publish_message(
    client_id: &str,
    topic: &str,
    payload: &serde_json::Value,
) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

/// Verifies: device posts properties via webhook → admin API returns them.
///
/// Mirrors demo step: `device.postProperties({ temperature, humidity, power })`
/// → `GET /api/admin/property` returns the posted values.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_post_and_query(ctx: &mut TestContext) {
    let product_id = "scenario_product_prop";
    let device_id = "scenario_device_prop";
    let topic = property_post_topic(product_id, device_id);

    let temperature: f64 = 25.5;
    let humidity: f64 = 60.0;

    let payload = json!({
        "id": "prop-test-001",
        "ack": 0,
        "params": {
            "temperature": temperature,
            "humidity": humidity,
            "power": true
        }
    });

    let msg = mqtt_publish_message(device_id, &topic, &payload);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let props = &resp["data"][0]["properties"];
    assert_eq!(props["temperature"]["value"], temperature);
    assert_eq!(props["humidity"]["value"], humidity);
    assert_eq!(props["power"]["value"], true);
}

/// Verifies: device posts event via webhook → admin API returns it.
///
/// Mirrors demo step: `device.postEvent({ event, marker })`
/// → `GET /api/admin/event` contains the event with the marker.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_event_post_and_query(ctx: &mut TestContext) {
    let product_id = "scenario_product_event";
    let device_id = "scenario_device_event";
    let topic = event_post_topic(product_id, device_id);

    let marker = format!(
        "marker-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let payload = json!({
        "id": "event-test-001",
        "ack": 0,
        "params": {
            "event": "mqtt_e2e_boot",
            "marker": marker
        }
    });

    let msg = mqtt_publish_message(device_id, &topic, &payload);
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let events = resp["data"].as_array().expect("Expected data array");
    let found = events
        .iter()
        .any(|row| row["events"]["marker"].as_str() == Some(marker.as_str()));
    assert!(found, "Event with marker '{marker}' not found in response");
}

/// Verifies: admin creates property command → webhook simulates delivery →
/// device replies via webhook → command status becomes Success.
///
/// Mirrors demo steps:
/// 1. `POST /api/admin/property/command` → 201
/// 2. Manually set status to Sent (simulates MQTT delivery)
/// 3. `POST /api/thing/property/set_reply` with code 200
/// 4. `GET /api/admin/property/command` → status = "Success"
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_command_lifecycle(ctx: &mut TestContext) {
    use crate::db::models::CommandStatus;

    let product_id = "scenario_product_cmd";
    let device_id = "scenario_device_cmd";

    // 1. Create property command
    let command_value = json!({ "power": false, "brightness": 42 });
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: command_value.clone(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Query command to get its id, verify status is Pending
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commands = resp["data"].as_array().expect("Expected data array");
    assert_eq!(commands.len(), 1);
    let command_id = commands[0]["id"].as_i64().expect("Command must have id");
    assert_eq!(commands[0]["status"], "Pending");

    // 3. Simulate MQTT delivery: update status from Pending → Sent
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![command_id],
            product_id,
            device_id,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    // 4. Device replies via webhook with code 200
    let reply_topic = property_set_reply_topic(product_id, device_id);
    let reply_payload = json!({
        "id": "reply-001",
        "data": [command_id],
        "code": 200
    });
    let reply_msg = mqtt_publish_message(device_id, &reply_topic, &reply_payload);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 5. Verify command status is now Success
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commands = resp["data"].as_array().expect("Expected data array");
    let updated = commands
        .iter()
        .find(|c| c["id"].as_i64() == Some(command_id))
        .expect("Command should exist");
    assert_eq!(updated["status"], "Success");
}

/// Full integrated device flow: property post → event post → command create →
/// command delivery → command reply → all verified.
///
/// Mirrors the entire `mqtt-device-flow-demo.e2e.ts` test without MQTT client.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_full_device_flow(ctx: &mut TestContext) {
    use crate::db::models::CommandStatus;

    let product_id = "scenario_product_full";
    let device_id = "scenario_device_full";

    // --- Step 1: Post properties ---
    let temperature: f64 = 20.0 + (rand_float() * 10.0);
    let prop_topic = property_post_topic(product_id, device_id);
    let prop_payload = json!({
        "id": "full-prop-001",
        "ack": 0,
        "params": {
            "temperature": temperature,
            "humidity": 51,
            "power": true
        }
    });
    let msg = mqtt_publish_message(device_id, &prop_topic, &prop_payload);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Property post failed");

    // Verify properties
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let temp_val = &resp["data"][0]["properties"]["temperature"]["value"];
    let temp: f64 = temp_val
        .as_f64()
        .unwrap_or_else(|| panic!("Expected number, got {temp_val}"));
    assert!(
        (temp - temperature).abs() < f64::EPSILON,
        "Expected temperature {temperature}, got {temp}"
    );

    // --- Step 2: Post event ---
    let marker = format!(
        "full-flow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let event_topic = event_post_topic(product_id, device_id);
    let event_payload = json!({
        "id": "full-event-001",
        "ack": 0,
        "params": {
            "event": "mqtt_e2e_boot",
            "marker": marker
        }
    });
    let msg = mqtt_publish_message(device_id, &event_topic, &event_payload);
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Event post failed");

    // Verify event
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let found = resp["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["events"]["marker"].as_str() == Some(marker.as_str()));
    assert!(found, "Event with marker not found");

    // --- Step 3: Create property command ---
    let command_value = json!({ "power": false, "brightness": 42 });
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: command_value.clone(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Command creation failed");

    // Get command id
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let command_id = resp["data"][0]["id"]
        .as_i64()
        .expect("Command must have id");

    // Simulate delivery: Pending → Sent
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![command_id],
            product_id,
            device_id,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    // --- Step 4: Device replies ---
    let reply_topic = property_set_reply_topic(product_id, device_id);
    let reply_payload = json!({
        "id": "full-reply-001",
        "data": [command_id],
        "code": 200
    });
    let msg = mqtt_publish_message(device_id, &reply_topic, &reply_payload);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/set_reply",
        &msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Reply post failed");

    // --- Step 5: Verify command status is Success ---
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cmd = resp["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_i64() == Some(command_id))
        .expect("Command must exist");
    assert_eq!(cmd["status"], "Success", "Command status should be Success");
}

fn rand_float() -> f64 {
    use std::time::SystemTime;
    let ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (ns % 1000) as f64 / 1000.0
}
