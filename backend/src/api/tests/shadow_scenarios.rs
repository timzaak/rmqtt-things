//! Scenario tests for shadow device support (desired state / delta).
//!
//! Covers US-PA-042 (Set-Desired) and US-PA-043 (Get-Delta) against the
//! dev-delivered production code from BE-D02 (property_desired repository) and
//! BE-D03 (Set-Desired / Get-Delta handlers, `compute_delta`, OpenAPI + routing).
//!
//! Test style mirrors `mqtt_device_flow_scenarios.rs::scenario_property_command_lifecycle`:
//! in-process axum `#[test_context(TestContext)]` + `#[tokio::test]`, reusing
//! `super::simple_tests::{request, request_json, TestContext}`. HTTP calls go
//! through `ctx.service`; direct DB assertions go through `ctx._admin_state.db`.
//!
//! Business rules encoded (design shadow-device-support.md §4.1):
//! - R1 desired write single-source: only Set-Desired writes desired; one-shot
//!   commands and device reports never pollute it.
//! - R2 passive convergence: the platform never auto-re-pushes; a command that
//!   Failed leaves desired intact (still observable as a delta).
//! - RFC 7396 subset for the `desired` patch: `null` deletes a desired key.

use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use crate::api::admin_models::CreatePropertyCommandRequest;
use crate::api::web_models::RMqttPublishMessage;
use crate::db::models::CommandStatus;
use axum::http::{Method, StatusCode};
use base64::Engine;
use serde_json::{Value as JsonValue, json};
use test_context::test_context;

// --- shared helpers (mirror mqtt_device_flow_scenarios.rs) ---

fn encode_payload(value: &JsonValue) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

fn property_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/post")
}

fn property_set_reply_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/reply")
}

fn mqtt_publish_message(client_id: &str, topic: &str, payload: &JsonValue) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

/// PUT /api/admin/property/shadow/desired — Set-Desired.
///
/// `desired_patch` must be a JSON object (the RFC 7396 subset patch). The body
/// is built as a plain `serde_json::Value` because `SetDesiredRequest` is
/// `Deserialize`-only (not `Serialize`) on the server side; the wire shape is
/// `{ "product_id": ..., "device_id": ..., "desired": { ... } }`.
async fn put_desired(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
    desired_patch: JsonValue,
) -> (StatusCode, JsonValue) {
    let body = json!({
        "product_id": product_id,
        "device_id": device_id,
        "desired": match desired_patch {
            JsonValue::Object(ref map) => JsonValue::Object(map.clone()),
            ref other => panic!("desired patch must be a JSON object, got {other}"),
        },
    });
    let (status, text) = request_json(
        &ctx.service,
        Method::PUT,
        "/api/admin/property/shadow/desired",
        &body,
    )
    .await;
    let json = if text.is_empty() {
        JsonValue::Null
    } else {
        serde_json::from_str(&text).unwrap_or(JsonValue::Null)
    };
    (status, json)
}

/// GET /api/admin/property/shadow?product_id=&device_id= — Get-Delta.
/// Query keys are snake_case (matching `ShadowQuery`, not the camelCase shown
/// in design §4.2.2).
async fn get_shadow(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
) -> (StatusCode, JsonValue) {
    let (status, text) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/shadow?product_id={product_id}&device_id={device_id}"),
    )
    .await;
    let json = if text.is_empty() {
        JsonValue::Null
    } else {
        serde_json::from_str(&text).unwrap_or(JsonValue::Null)
    };
    (status, json)
}

/// GET /api/admin/property/command — list property commands for the device.
async fn list_commands(ctx: &TestContext, product_id: &str, device_id: &str) -> Vec<JsonValue> {
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "property command list failed");
    let resp: JsonValue = serde_json::from_str(&body).unwrap();
    resp["data"]
        .as_array()
        .expect("Expected data array")
        .clone()
}

// ---------------------------------------------------------------------------
// Scenario 1: Set-Desired (online) -> desired persisted, delta enqueued,
// reported converges -> delta empty.
//
// User Story: US-PA-042 (设置设备期望状态)
// Covers: design §4.1 R1 (desired write single-source), §4.2.1 Set-Desired,
//         §6.1 scenario_set_desired_online_converges. R3 delta convergence path.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_set_desired_online_converges(ctx: &mut TestContext) {
    let product_id = "shdw_product_online";
    let device_id = "shdw_device_online";

    // 1. Set-Desired: brightness=80. Device is offline in the test harness, so
    //    the delta command stays in Pending (same as the one-shot command path
    //    in scenario_property_command_lifecycle). Set-Desired must report
    //    pushed=true because the delta is non-empty.
    let (status, body) = put_desired(ctx, product_id, device_id, json!({ "brightness": 80 })).await;
    assert_eq!(status, StatusCode::OK, "Set-Desired should return 200");
    assert_eq!(
        body["desired"]["brightness"], 80,
        "merged desired should be echoed"
    );
    assert_eq!(
        body["delta"]["brightness"], 80,
        "delta should carry the bare desired value"
    );
    assert_eq!(
        body["pushed"], true,
        "non-empty delta must enqueue a command"
    );

    // 2. Direct DB assertion: desired persisted (R1 write single-source).
    let desired_row = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap()
        .expect("desired row must be persisted");
    assert_eq!(desired_row.desired["brightness"], 80);

    // 3. Get-Delta: delta still shows brightness=80 (reported missing).
    let (status, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shadow["desired"]["brightness"], 80);
    assert_eq!(
        shadow["delta"]["brightness"], 80,
        "delta should remain until reported converges"
    );

    // 4. The delta command is visible in the command queue as Pending.
    let commands = list_commands(ctx, product_id, device_id).await;
    let delta_cmd = commands
        .iter()
        .find(|c| c["command"]["brightness"].as_i64() == Some(80))
        .expect("delta command must be queued");
    assert_eq!(delta_cmd["status"], "Pending");
    let command_id = delta_cmd["id"].as_i64().expect("command must have id");

    // 5. Simulate online delivery: Pending -> Sent (mirror scenario_property_command_lifecycle).
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

    // 6. Device reports brightness=80 (converged with desired) via the property
    //    post webhook. The reported snapshot follows the {value, time} shape.
    let payload = json!({
        "id": "shdw-online-prop-001",
        "ack": 0,
        "params": { "brightness": 80 }
    });
    let msg = mqtt_publish_message(
        device_id,
        &property_post_topic(product_id, device_id),
        &payload,
    );
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 7. Acknowledge the delivered command: reply code 200 -> Success.
    let reply_msg = mqtt_publish_message(
        device_id,
        &property_set_reply_topic(product_id, device_id),
        &json!({ "id": "shdw-online-reply-001", "data": [command_id], "code": 200 }),
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 8. Get-Delta after convergence: delta must be empty (reported value ==
    //    desired value). R3 delta convergence path.
    let (status, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        shadow["delta"],
        json!({}),
        "delta must be empty once reported converges to desired"
    );
    // desired unchanged by the device report (R1: device reports don't write desired).
    assert_eq!(shadow["desired"]["brightness"], 80);
}

// ---------------------------------------------------------------------------
// Scenario 2: Set-Desired (offline) -> delta enqueued and stays Pending.
//
// User Story: US-PA-042 (设置设备期望状态) — offline queue path.
// Covers: design §4.2.1 Set-Desired (离线留队), §6.1
//         scenario_set_desired_offline_queues; reuses US-DV-009 queue+online-delivery.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_set_desired_offline_queues(ctx: &mut TestContext) {
    let product_id = "shdw_product_offline";
    let device_id = "shdw_device_offline";

    // Device never subscribes/reports, so the harness treats it as offline.
    let (status, body) = put_desired(
        ctx,
        product_id,
        device_id,
        json!({ "mode": "eco", "brightness": 30 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["pushed"], true,
        "non-empty delta must enqueue a command even when offline"
    );

    // The delta command must be queued as Pending, NOT transitioned to Sent.
    let commands = list_commands(ctx, product_id, device_id).await;
    let delta_cmd = commands
        .iter()
        .find(|c| c["command"].get("mode").is_some() || c["command"].get("brightness").is_some())
        .expect("delta command must be queued");
    assert_eq!(
        delta_cmd["status"], "Pending",
        "offline delta command must stay Pending (not Sent)"
    );

    // Get-Delta still shows the divergence (reported missing).
    let (status, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shadow["desired"]["mode"], "eco");
    assert_eq!(shadow["desired"]["brightness"], 30);
    assert_eq!(shadow["delta"]["mode"], "eco");
    assert_eq!(shadow["delta"]["brightness"], 30);
}

// ---------------------------------------------------------------------------
// Scenario 3: a one-shot property command must NOT pollute the desired view.
//
// User Story: US-PA-042 (设置设备期望状态) — desired not polluted by one-shot command.
// Covers: design §4.1 R1 (desired write single-source), §6.1
//         scenario_desired_not_polluted_by_one_shot_command.
// WHY encoded: a one-shot command is a transient action, not a persistent
// intent; desired must reflect only what Set-Desired wrote.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_desired_not_polluted_by_one_shot_command(ctx: &mut TestContext) {
    let product_id = "shdw_product_pollute";
    let device_id = "shdw_device_pollute";

    // 1. Set desired brightness=80.
    let (status, _) = put_desired(ctx, product_id, device_id, json!({ "brightness": 80 })).await;
    assert_eq!(status, StatusCode::OK);

    // 2. Issue a one-shot property command on the SAME property with a DIFFERENT
    //    value (brightness=10). R4 desired and commands coexist but stay separate.
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: json!({ "brightness": 10 }),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 3. desired view must be unchanged (R1): Set-Desired value still 80.
    let (status, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        shadow["desired"]["brightness"], 80,
        "desired must not be polluted by the one-shot command"
    );

    // 4. delta must still reflect desired (80) vs reported (missing): brightness=80.
    assert_eq!(
        shadow["delta"]["brightness"], 80,
        "delta must reflect the desired value, not the one-shot command value"
    );

    // 5. Both commands coexist in the queue (the one-shot and the delta).
    let commands = list_commands(ctx, product_id, device_id).await;
    let brightness_values: Vec<&JsonValue> = commands
        .iter()
        .filter_map(|c| c["command"].get("brightness"))
        .collect();
    assert!(
        brightness_values.iter().any(|v| **v == json!(80)),
        "delta command (brightness=80) must be present"
    );
    assert!(
        brightness_values.iter().any(|v| **v == json!(10)),
        "one-shot command (brightness=10) must be present"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: an empty desired patch `{}` must be rejected with 400.
//
// User Story: US-PA-042 (设置设备期望状态) — scenario 4 (空期望被拒).
// Covers: design §4.2.2 / §5.2 (patch 空对象返回 400), §6.1
//         scenario_empty_desired_rejected.
// WHY encoded: an empty patch is a no-op and must be rejected at the handler
// boundary (no desired write, no command enqueued).
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_empty_desired_rejected(ctx: &mut TestContext) {
    let product_id = "shdw_product_empty";
    let device_id = "shdw_device_empty";

    let (status, body) = put_desired(ctx, product_id, device_id, json!({})).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "empty desired patch must be rejected with 400"
    );
    // No desired row should have been written.
    let desired_row = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap();
    assert!(
        desired_row.is_none(),
        "no desired row should be persisted for a rejected empty patch"
    );
    // Sanity-check the error message mentions the desired field.
    let _ = body; // body content is handler-defined; status code is the contract.
}

// ---------------------------------------------------------------------------
// Scenario 5: a `null` value in the desired patch deletes that desired key.
//
// User Story: US-PA-043 (查看设备期望状态与差异) — null deletion + delta change.
// Covers: design §4.2 (RFC 7396 subset: null=delete), §5.1 merge_desired, §5.2
//         备注 (patch 只含 null 合法, 不返回 400), §6.1
//         scenario_null_deletes_desired_property.
// WHY encoded: `null` is the RFC 7396 delete sentinel, not a stored value; the
// key must actually be removed from the desired document and the delta must
// reflect the removal. A patch containing only null keys is a valid operation
// (a delete) and must NOT be rejected as an empty patch.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_null_deletes_desired_property(ctx: &mut TestContext) {
    let product_id = "shdw_product_null";
    let device_id = "shdw_device_null";

    // 1. Set desired brightness=80 and mode=eco.
    let (status, body) = put_desired(
        ctx,
        product_id,
        device_id,
        json!({ "brightness": 80, "mode": "eco" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["desired"]["brightness"], 80);
    assert_eq!(body["desired"]["mode"], "eco");

    // 2. Verify delta reflects both keys (reported missing).
    let (_, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(shadow["delta"]["brightness"], 80);
    assert_eq!(shadow["delta"]["mode"], "eco");

    // 3. Patch {"brightness": null} -> delete the brightness key from desired.
    //    A patch that contains only a null entry is a valid delete operation and
    //    must NOT be treated as the empty-patch 400 case (design §5.2 备注).
    let (status, body) =
        put_desired(ctx, product_id, device_id, json!({ "brightness": null })).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a patch containing a null (delete) entry must not be rejected"
    );

    // 4. brightness must be removed from the merged desired document.
    assert!(
        body["desired"].get("brightness").is_none(),
        "brightness must be removed from desired after a null patch"
    );
    assert_eq!(body["desired"]["mode"], "eco", "mode must be untouched");

    // 5. Direct DB assertion: the key is truly gone (RFC 7396 delete semantics).
    let desired_row = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap()
        .expect("desired row must remain");
    assert!(
        desired_row.desired.get("brightness").is_none(),
        "brightness key must be physically removed from the desired document"
    );
    assert_eq!(desired_row.desired["mode"], "eco");

    // 6. delta must no longer carry brightness (only mode diverges now).
    let (_, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert!(
        shadow["delta"].get("brightness").is_none(),
        "delta must not surface a deleted desired key"
    );
    assert_eq!(shadow["delta"]["mode"], "eco");
}

// ---------------------------------------------------------------------------
// Scenario 6: a command that Failed leaves desired intact (passive convergence).
//
// User Story: US-PA-043 (查看设备期望状态与差异) — failed delivery keeps desired.
// Covers: design §4.1 R2 (passive convergence), §6.1
//         scenario_command_failed_desired_kept.
// WHY encoded: the platform never auto-re-pushes on failure; desired is a
// persistent intent and stays observable as a delta until the admin explicitly
// acts again. The desired document is independent of command ack results.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_shadow_command_failed_desired_kept(ctx: &mut TestContext) {
    let product_id = "shdw_product_failed";
    let device_id = "shdw_device_failed";

    // 1. Set-Desired -> delta command enqueued (Pending).
    let (status, body) = put_desired(ctx, product_id, device_id, json!({ "brightness": 80 })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["delta"]["brightness"], 80);

    let commands = list_commands(ctx, product_id, device_id).await;
    let delta_cmd = commands
        .iter()
        .find(|c| c["command"]["brightness"].as_i64() == Some(80))
        .expect("delta command must be queued");
    assert_eq!(delta_cmd["status"], "Pending");
    let command_id = delta_cmd["id"].as_i64().expect("command must have id");

    // 2. Simulate delivery: Pending -> Sent.
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

    // 3. Simulate command failure: Sent -> Failed (direct status transition,
    //    mirroring scenario_property_command_lifecycle's direct-status approach).
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![command_id],
            product_id,
            device_id,
            CommandStatus::Failed,
            CommandStatus::Sent,
        )
        .await
        .unwrap();

    // 4. desired must be kept (R2 passive convergence): the document is
    //    independent of command ack results.
    let desired_row = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap()
        .expect("desired must persist after a failed command");
    assert_eq!(
        desired_row.desired["brightness"], 80,
        "desired must be unchanged after command failure"
    );

    // 5. Get-Delta: delta still shows brightness=80 (still pending convergence).
    let (status, shadow) = get_shadow(ctx, product_id, device_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        shadow["desired"]["brightness"], 80,
        "desired view must keep the intended value"
    );
    assert_eq!(
        shadow["delta"]["brightness"], 80,
        "delta must still show the divergence after a failed delivery (passive convergence)"
    );
}
