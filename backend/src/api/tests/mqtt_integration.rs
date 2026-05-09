//! MQTT integration tests using real RMQTT broker + MQTT client.
//!
//! These tests exercise the full round-trip:
//! MQTT client → RMQTT broker → webhook → axum HTTP server → database
//!
//! Requires RMQTT running (scripts/test-start.py). Run with:
//!   uv run scripts/backend-test.py -- -E 'test(mqtt_)'

use super::mqtt_test_context::MqttTestContext;
use crate::api::utils::send_property_command_to_device;
use serde_json::json;
use serial_test::serial;
use std::time::Duration;
use test_context::test_context;

/// Device posts properties via real MQTT → admin API returns them.
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
#[ignore = "RMQTT webhook URL is static; needs dynamic port registration to run in parallel"]
async fn mqtt_property_post_and_query(ctx: &mut MqttTestContext) {
    let product_id = "mqtt_test_product_prop";
    let device_id = &format!(
        "mqtt-dev-prop-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let mut device = ctx.connect_device(product_id, device_id).await;

    let temperature: f64 = 25.5;
    device
        .post_properties(json!({
            "temperature": temperature,
            "humidity": 60.0,
            "power": true
        }))
        .await;

    // Poll admin API until data appears
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (status, body) = ctx.admin_get(
            &format!("/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
        ).await;
        assert_eq!(status, 200);

        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        if let Some(props) = resp["data"][0]["properties"]["temperature"]["value"].as_f64()
            && (props - temperature).abs() < f64::EPSILON
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout waiting for property data"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    device.disconnect().await;
}

/// Device posts event via real MQTT → admin API returns it.
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
#[ignore = "RMQTT webhook URL is static; needs dynamic port registration to run in parallel"]
async fn mqtt_event_post_and_query(ctx: &mut MqttTestContext) {
    let product_id = "mqtt_test_product_event";
    let device_id = &format!(
        "mqtt-dev-event-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let mut device = ctx.connect_device(product_id, device_id).await;

    let marker = format!(
        "mqtt-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    device
        .post_event(json!({
            "event": "mqtt_integration_boot",
            "marker": marker
        }))
        .await;

    // Poll admin API until event appears
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (status, body) = ctx
            .admin_get(&format!(
                "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
            ))
            .await;
        assert_eq!(status, 200);

        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        let found = resp["data"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .any(|row| row["events"]["marker"].as_str() == Some(marker.as_str()))
            })
            .unwrap_or(false);

        if found {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout waiting for event data"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    device.disconnect().await;
}

/// Full property command lifecycle via MQTT:
/// create command → device receives via MQTT → reply → status = Success
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
#[ignore = "RMQTT webhook URL is static; needs dynamic port registration to run in parallel"]
async fn mqtt_property_command_lifecycle(ctx: &mut MqttTestContext) {
    let product_id = "mqtt_test_product_cmd";
    let device_id = &format!(
        "mqtt-dev-cmd-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    // Connect device — auto-subscription activates property/set topic
    let mut device = ctx.connect_device(product_id, device_id).await;

    // Wait for auto-subscription webhook to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create property command via admin API
    let command_value = json!({ "power": false, "brightness": 42 });
    let (status, _) = ctx
        .admin_post_json(
            "/api/admin/property/command",
            &json!({
                "product_id": product_id,
                "device_id": device_id,
                "command": command_value,
            }),
        )
        .await;
    assert_eq!(status, 201, "Command creation failed");

    // Manually trigger command sending (admin handler skips send when
    // is_subscribed_to_properties returns false due to auto-sub wildcard topic)
    send_property_command_to_device(
        &ctx._admin_state.db,
        &ctx._admin_state.rmqtt_client,
        product_id,
        device_id,
    )
    .await
    .unwrap();

    // Wait for command to arrive via MQTT (backend publishes to RMQTT HTTP API → RMQTT → device)
    let command = device.wait_for_command(Duration::from_secs(10)).await;
    assert!(!command.ids.is_empty(), "Command should have IDs");
    assert_eq!(command.data["power"], json!(false));
    assert_eq!(command.data["brightness"], json!(42));

    // Reply with success
    device.reply_command(&command, 200).await;

    // Poll until command status is Success
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (status, body) = ctx.admin_get(
            &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
        ).await;
        assert_eq!(status, 200);

        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        if let Some(cmd) = resp["data"].as_array().and_then(|cmds| {
            cmds.iter()
                .find(|c| command.ids.contains(&c["id"].as_i64().unwrap_or(-1)))
        }) && cmd["status"] == "Success"
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout waiting for command Success"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    device.disconnect().await;
}

/// Full integrated device flow: property post → event post → command create → reply → verify.
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
#[ignore = "RMQTT webhook URL is static; needs dynamic port registration to run in parallel"]
async fn mqtt_full_device_flow(ctx: &mut MqttTestContext) {
    let product_id = "mqtt_test_product_full";
    let device_id = &format!(
        "mqtt-dev-full-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    let mut device = ctx.connect_device(product_id, device_id).await;

    // --- Step 1: Post properties ---
    let temperature: f64 = 20.0 + (rand_float() * 10.0);
    device
        .post_properties(json!({
            "temperature": temperature,
            "humidity": 51,
            "power": true
        }))
        .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (_, body) = ctx.admin_get(
            &format!("/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
        ).await;
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        if let Some(val) = resp["data"][0]["properties"]["temperature"]["value"].as_f64()
            && (val - temperature).abs() < f64::EPSILON
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout: property not found"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // --- Step 2: Post event ---
    let marker = format!(
        "full-flow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    device
        .post_event(json!({
            "event": "mqtt_e2e_boot",
            "marker": marker
        }))
        .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (_, body) = ctx
            .admin_get(&format!(
                "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
            ))
            .await;
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        let found = resp["data"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .any(|r| r["events"]["marker"].as_str() == Some(marker.as_str()))
            })
            .unwrap_or(false);
        if found {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout: event not found"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // --- Step 3: Create and reply to command ---
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (status, _) = ctx
        .admin_post_json(
            "/api/admin/property/command",
            &json!({
                "product_id": product_id,
                "device_id": device_id,
                "command": { "power": false, "brightness": 42 }
            }),
        )
        .await;
    assert_eq!(status, 201, "Command creation failed");

    send_property_command_to_device(
        &ctx._admin_state.db,
        &ctx._admin_state.rmqtt_client,
        product_id,
        device_id,
    )
    .await
    .unwrap();

    let command = device.wait_for_command(Duration::from_secs(10)).await;
    device.reply_command(&command, 200).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (_, body) = ctx.admin_get(
            &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
        ).await;
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        if let Some(cmd) = resp["data"].as_array().and_then(|cmds| {
            cmds.iter()
                .find(|c| command.ids.contains(&c["id"].as_i64().unwrap_or(-1)))
        }) {
            assert_eq!(cmd["status"], "Success", "Command should be Success");
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timeout: command not found"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    device.disconnect().await;
}

fn rand_float() -> f64 {
    use std::time::SystemTime;
    let ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (ns % 1000) as f64 / 1000.0
}
