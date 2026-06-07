//! Scenario tests for alarm lifecycle transitions and clear condition auto-clear.
//!
//! Covers:
//! - active -> acknowledged -> cleared lifecycle (US-PA-040)
//! - active -> cleared direct (US-PA-041)
//! - Conflict on double clear / ack cleared / double ack (US-PA-040 state machine)
//! - Clear nonexistent alarm -> 404 (US-PA-041 negative)
//! - List alarms filter by status (active/acknowledged/cleared)
//! - Status and acknowledged mutual exclusion -> 400
//! - Alarm response contains status and cleared_at fields
//! - Duration=0 triggers instantly via webhook (US-PA-038 backward compat)
//! - Clear condition auto-clears active alarm (US-PA-039)

use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use axum::http::{Method, StatusCode};
use serde_json::json;
use test_context::test_context;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Insert a test alarm record directly into the database and return its id.
/// The DB default for `status` is 'active'.
async fn insert_test_alarm(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
    level: i16,
    message: &str,
) -> i64 {
    ctx._admin_state
        .db
        .alarm()
        .insert_alarm(
            1, // rule_id -- dummy, not under test
            "test rule",
            product_id,
            device_id,
            level,
            Some(message),
            None,
            "property",
        )
        .await
        .expect("insert_test_alarm failed")
}

/// Create a product and return its model_no (used as product_id in alarm rules).
async fn create_test_product(ctx: &TestContext, model_no: &str) {
    let create_req = json!({
        "name": format!("Product for {model_no}"),
        "model_no": model_no,
        "description": "Test product for alarm lifecycle scenarios"
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/product",
        &create_req,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Failed to create test product {model_no}"
    );
}

/// Extract the rule id from a create response.
fn extract_rule_id(resp: &serde_json::Value) -> i64 {
    resp["data"]["id"]
        .as_i64()
        .expect("Rule response must contain data.id")
}

/// Poll GET /api/admin/alarm?status={status}&product_id={product_id} until at least one alarm
/// is found, or max_attempts is exhausted (panics on timeout).
///
/// Required because evaluate_and_trigger runs inside tokio::spawn, creating an async gap
/// between the HTTP 204 response and actual alarm creation/clearing.
async fn wait_for_alarms(
    ctx: &TestContext,
    status: &str,
    product_id: &str,
    max_attempts: usize,
    interval_ms: u64,
) -> Vec<serde_json::Value> {
    for _ in 0..max_attempts {
        let (resp_status, body) = request(
            &ctx.service,
            Method::GET,
            &format!(
                "/api/admin/alarm?status={status}&product_id={product_id}&page=1&page_size=50"
            ),
        )
        .await;
        assert_eq!(
            resp_status,
            StatusCode::OK,
            "wait_for_alarms: list alarms must return 200"
        );

        let resp: serde_json::Value =
            serde_json::from_str(&body).expect("wait_for_alarms: invalid JSON");
        let data = resp["data"]
            .as_array()
            .expect("wait_for_alarms: data must be array");
        if !data.is_empty() {
            return data.clone();
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
    }
    panic!(
        "timed out waiting for alarms with status={status}, product_id={product_id} after {max_attempts} attempts"
    );
}

/// Build an RMqttPublishMessage payload for property post with the given property value.
fn build_property_post_message(
    product_id: &str,
    device_id: &str,
    property_name: &str,
    value: serde_json::Value,
) -> crate::api::web_models::RMqttPublishMessage {
    use base64::Engine;
    let topic = format!("/{product_id}/{device_id}/thing/event/property/post");
    let payload = json!({
        "id": "test-property-post",
        "ack": 0,
        "params": { property_name: value }
    });
    let encoded_payload =
        base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&payload).unwrap());
    crate::api::web_models::RMqttPublishMessage {
        client_id: device_id.to_string(),
        topic,
        payload: encoded_payload,
        ..Default::default()
    }
}

// ===========================================================================
// a. Active -> Acknowledged -> Cleared lifecycle
// ===========================================================================

/// User story: US-PA-040
/// Covers: Scenario -- Full lifecycle: active alarm is acknowledged, then cleared.
///         After ack, status becomes "acknowledged". After clear, status becomes "cleared"
///         and cleared_at is present.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_alarm_lifecycle_active_to_acknowledged_to_cleared(ctx: &mut TestContext) {
    let product_id = "lc_a2a2c";
    let id = insert_test_alarm(ctx, product_id, "dev_lifecycle", 1, "Lifecycle alarm").await;

    // Acknowledge
    let (status, body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "ack must return 200");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        resp["data"]["status"], "acknowledged",
        "status must be acknowledged after ack"
    );

    // Clear
    let (status, body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/clear"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "clear must return 200");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        resp["data"]["status"], "cleared",
        "status must be cleared after clear"
    );
    assert!(
        resp["data"]["cleared_at"].is_string(),
        "cleared_at must be present as RFC3339 string after clear"
    );
}

// ===========================================================================
// b. Active -> Cleared direct
// ===========================================================================

/// User story: US-PA-041
/// Covers: Scenario -- Active alarm is cleared directly without acknowledging first.
///         After clear, status becomes "cleared" and cleared_at is a non-null RFC3339 string.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_alarm_lifecycle_active_to_cleared_direct(ctx: &mut TestContext) {
    let product_id = "lc_a2c_dir";
    let id = insert_test_alarm(ctx, product_id, "dev_direct_clear", 1, "Direct clear alarm").await;

    let (status, body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/clear"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "clear must return 200");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        resp["data"]["status"], "cleared",
        "status must be cleared after direct clear"
    );
    let cleared_at = resp["data"]["cleared_at"]
        .as_str()
        .expect("cleared_at must be a non-null RFC3339 string");
    assert!(!cleared_at.is_empty(), "cleared_at must not be empty");
}

// ===========================================================================
// c. Clear already cleared -> 409
// ===========================================================================

/// User story: US-PA-041 state machine
/// Covers: Scenario -- Clearing an already-cleared alarm returns 409 Conflict.
///         The clear endpoint must reject idempotent 200 on cleared alarms.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_clear_already_cleared_alarm_returns_409(ctx: &mut TestContext) {
    let product_id = "lc_clr_409";
    let id = insert_test_alarm(ctx, product_id, "dev_clr_409", 1, "Clear conflict").await;

    // First clear succeeds
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/clear"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "First clear should return 200");

    // Second clear must return 409 Conflict
    let (status, _body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/clear"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Re-clearing must return 409 Conflict, got {status}"
    );
}

// ===========================================================================
// d. Ack cleared -> 409
// ===========================================================================

/// User story: US-PA-040 state machine
/// Covers: Scenario -- Acknowledging a cleared alarm returns 409 Conflict.
///         Only active alarms can be acknowledged.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_ack_cleared_alarm_returns_409(ctx: &mut TestContext) {
    let product_id = "lc_ack_clr";
    let id = insert_test_alarm(ctx, product_id, "dev_ack_clr", 1, "Ack cleared alarm").await;

    // Clear the alarm first
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/clear"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "clear should return 200");

    // Attempt to ack the cleared alarm
    let (status, _body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Ack on cleared alarm must return 409 Conflict, got {status}"
    );
}

// ===========================================================================
// e. Ack already acknowledged -> 409
// ===========================================================================

/// User story: US-PA-040 state machine
/// Covers: Scenario -- Re-acknowledging an already-acknowledged alarm returns 409 Conflict.
///         The ack endpoint rejects non-active alarms.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_ack_already_acknowledged_returns_409(ctx: &mut TestContext) {
    let product_id = "lc_ack_ack";
    let id = insert_test_alarm(ctx, product_id, "dev_ack_ack", 1, "Double ack alarm").await;

    // First ack succeeds
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "First ack should return 200");

    // Second ack must return 409 Conflict
    let (status, _body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Re-acknowledging must return 409 Conflict, got {status}"
    );
}

// ===========================================================================
// f. Clear nonexistent -> 404
// ===========================================================================

/// User story: US-PA-041 negative
/// Covers: Scenario -- Clearing a nonexistent alarm returns 404 Not Found.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_clear_nonexistent_alarm_returns_404(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::PATCH, "/api/admin/alarm/999999/clear").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Clear nonexistent must return 404"
    );
}

// ===========================================================================
// g. List alarms filter by status
// ===========================================================================

/// User story: US-PA-040
/// Covers: Scenario -- Admin inserts 3 alarms in different statuses, then filters by each
///         status value. Only alarms matching the requested status are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_filter_by_status(ctx: &mut TestContext) {
    let product_id = "lc_flt_status";

    // Insert active alarm
    let id_active = insert_test_alarm(ctx, product_id, "dev_active", 0, "Active alarm").await;

    // Insert alarm and ack it (status = acknowledged)
    let id_ack = insert_test_alarm(ctx, product_id, "dev_acked", 1, "Acked alarm").await;
    request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id_ack}/ack"),
    )
    .await;

    // Insert alarm and clear it (status = cleared)
    let id_cleared = insert_test_alarm(ctx, product_id, "dev_cleared", 2, "Cleared alarm").await;
    request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id_cleared}/clear"),
    )
    .await;

    // Filter by status=active
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?status=active&product_id={product_id}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let active_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();
    assert!(
        active_ids.contains(&id_active),
        "Active alarm must appear in status=active results"
    );
    assert!(
        !active_ids.contains(&id_ack),
        "Acknowledged alarm must not appear in status=active results"
    );
    assert!(
        !active_ids.contains(&id_cleared),
        "Cleared alarm must not appear in status=active results"
    );

    // Filter by status=acknowledged
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/alarm?status=acknowledged&product_id={product_id}&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let ack_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();
    assert!(
        ack_ids.contains(&id_ack),
        "Acknowledged alarm must appear in status=acknowledged results"
    );

    // Filter by status=cleared
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?status=cleared&product_id={product_id}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let cleared_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();
    assert!(
        cleared_ids.contains(&id_cleared),
        "Cleared alarm must appear in status=cleared results"
    );
}

// ===========================================================================
// h. Status and acknowledged mutual exclusion -> 400
// ===========================================================================

/// User story: US-PA-040 validation
/// Covers: Scenario -- Passing both status and acknowledged query params returns 400 Bad Request.
///         These filters are mutually exclusive.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_status_and_acknowledged_mutual_exclusion(ctx: &mut TestContext) {
    let (status, _body) = request(
        &ctx.service,
        Method::GET,
        "/api/admin/alarm?status=active&acknowledged=false&page=1&page_size=50",
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "status + acknowledged must return 400 Bad Request, got {status}"
    );
}

// ===========================================================================
// i. Alarm response contains status and cleared_at
// ===========================================================================

/// User story: US-PA-040
/// Covers: Scenario -- Alarm list response records contain the status field (string)
///         and cleared_at field (null for active alarms).
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_alarm_response_contains_status_and_cleared_at(ctx: &mut TestContext) {
    let product_id = "lc_resp_fields";
    insert_test_alarm(ctx, product_id, "dev_resp_fields", 1, "Field check alarm").await;

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(!data.is_empty(), "Expected at least one alarm record");

    let record = &data[0];
    assert!(
        record["status"].is_string(),
        "Alarm record must contain status field as string"
    );
    assert_eq!(
        record["status"], "active",
        "Newly inserted alarm must have status=active"
    );
    assert!(
        record["cleared_at"].is_null(),
        "Active alarm must have cleared_at=null, got {:?}",
        record["cleared_at"]
    );
}

// ===========================================================================
// j. Duration=0 triggers instantly via webhook
// ===========================================================================

/// User story: US-PA-038 backward compat integration
/// Covers: Scenario -- Property rule with duration_minutes=0 triggers an alarm instantly
///         when a property report exceeds the threshold. Uses wait_for_alarms helper
///         because evaluate_and_trigger runs in tokio::spawn.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_duration_rule_with_zero_triggers_instantly_via_webhook(ctx: &mut TestContext) {
    let product_id = "lc_dur_zero";
    let device_id = "dev_dur_zero";

    // Create product
    create_test_product(ctx, product_id).await;

    // Create property rule with duration_minutes=0 (backward compat default) and condition >80
    let rule_body = json!({
        "product_id": product_id,
        "name": "Instant trigger rule",
        "description": "Rule with duration_minutes=0 for instant trigger",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 80 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5,
        "duration_minutes": 0
    });
    let (status, resp_body) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/alarm-rule",
        &rule_body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Rule creation must succeed");
    let rule_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&rule_resp);

    // Send property report with value 90 (exceeds threshold 80) via property post endpoint
    let msg = build_property_post_message(product_id, device_id, "temperature", json!(90));
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "Property post must return 204"
    );

    // Wait for alarm to appear (async gap: evaluate_and_trigger runs in tokio::spawn)
    let alarms = wait_for_alarms(ctx, "active", product_id, 20, 100).await;
    assert!(!alarms.is_empty(), "Expected at least one active alarm");

    let alarm = &alarms[0];
    assert_eq!(alarm["status"], "active", "Alarm must have status=active");
    assert_eq!(
        alarm["rule_id"], rule_id,
        "Alarm rule_id must match created rule"
    );
}

// ===========================================================================
// k. Clear condition auto-clears active alarm
// ===========================================================================

/// User story: US-PA-039 auto-clear flow
/// Covers: Scenario -- Property rule with condition >80 and clear_condition <80.
///         Send value 90 to trigger alarm, then send value 70 to auto-clear.
///         Uses wait_for_alarms helper for both steps due to tokio::spawn async gap.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_clear_condition_auto_clears_active_alarm(ctx: &mut TestContext) {
    let product_id = "lc_auto_clear";
    let device_id = "dev_auto_clear";

    // Create product
    create_test_product(ctx, product_id).await;

    // Create property rule with condition >80 and clear_condition <80
    let rule_body = json!({
        "product_id": product_id,
        "name": "Auto-clear rule",
        "description": "Rule with clear condition for auto-clear",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 80 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5,
        "clear_condition": { "operator": "<", "value": 80 }
    });
    let (status, resp_body) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/alarm-rule",
        &rule_body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Rule creation must succeed");
    let rule_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&rule_resp);

    // Step 1: Send property value 90 (exceeds threshold 80) to trigger alarm
    let msg_trigger = build_property_post_message(product_id, device_id, "temperature", json!(90));
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/post",
        &msg_trigger,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "Trigger property post must return 204"
    );

    // Wait for active alarm to be created (async gap)
    let active_alarms = wait_for_alarms(ctx, "active", product_id, 20, 100).await;
    assert!(
        !active_alarms.is_empty(),
        "Expected at least one active alarm after trigger"
    );
    let alarm = &active_alarms[0];
    assert_eq!(
        alarm["status"], "active",
        "Alarm must have status=active after trigger"
    );
    assert_eq!(
        alarm["rule_id"], rule_id,
        "Alarm rule_id must match created rule"
    );
    let alarm_id = alarm["id"].as_i64().expect("Alarm must have numeric id");

    // Step 2: Send property value 70 (below threshold 80, satisfies clear_condition)
    let msg_clear = build_property_post_message(product_id, device_id, "temperature", json!(70));
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/post",
        &msg_clear,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "Clear property post must return 204"
    );

    // Wait for alarm to be auto-cleared (async gap)
    let cleared_alarms = wait_for_alarms(ctx, "cleared", product_id, 20, 100).await;
    assert!(
        !cleared_alarms.is_empty(),
        "Expected at least one cleared alarm after auto-clear"
    );

    // Find the specific alarm by id
    let cleared_alarm = cleared_alarms
        .iter()
        .find(|a| a["id"].as_i64() == Some(alarm_id))
        .unwrap_or_else(|| panic!("Alarm {alarm_id} not found in cleared results"));

    assert_eq!(
        cleared_alarm["status"], "cleared",
        "Alarm must have status=cleared after auto-clear"
    );
    assert!(
        cleared_alarm["cleared_at"].is_string(),
        "Auto-cleared alarm must have cleared_at as RFC3339 string"
    );
}
