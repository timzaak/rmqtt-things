//! Extended scenario tests for alarm API validation and backward compatibility.
//!
//! Covers:
//! - Backward compat: rule without duration_minutes defaults to 0 (US-PA-038)
//! - Backward compat: existing alarm has status=="active" from DB default (US-PA-040)
//! - Negative: clear nonexistent alarm returns 404 (US-PA-041)
//! - Default clear_condition is null when omitted (US-PA-039)
//! - Update rule to set clear_condition (US-PA-039)
//! - Non-property rule accepts duration_minutes on update without error (US-PA-038)
//! - Backward compat: acknowledged filter still works with status-based query (US-PA-040)

use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use axum::http::{Method, StatusCode};
use serde_json::json;
use test_context::test_context;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a product and return its model_no (used as product_id in alarm rules).
async fn create_test_product(ctx: &TestContext, model_no: &str) {
    let create_req = json!({
        "name": format!("Product for {model_no}"),
        "model_no": model_no,
        "description": "Test product for alarm extended scenarios"
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

/// Extract the rule id from a create response.
fn extract_rule_id(resp: &serde_json::Value) -> i64 {
    resp["data"]["id"]
        .as_i64()
        .expect("Rule response must contain data.id")
}

// ===========================================================================
// a. Backward compat: rule without duration_minutes defaults to 0
// ===========================================================================

/// User story: US-PA-038 backward compat
/// Covers: Creating a property rule with `duration_minutes` omitted (no field in JSON body)
///         results in duration_minutes == 0 in the response. This ensures existing rules
///         that were created before the duration feature retain instant-trigger behavior.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_backward_compat_rule_without_duration_triggers_instantly(ctx: &mut TestContext) {
    let model_no = "ext_compat_dur";
    create_test_product(ctx, model_no).await;

    // Create property rule WITHOUT duration_minutes field
    let body = json!({
        "product_id": model_no,
        "name": "Compat rule without duration",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;

    assert_eq!(status, StatusCode::CREATED);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(
        resp["data"]["duration_minutes"], 0,
        "duration_minutes must default to 0 when omitted -- backward compat for instant trigger"
    );
    assert!(
        resp["data"]["clear_condition"].is_null(),
        "clear_condition must default to null when omitted"
    );
}

// ===========================================================================
// b. Backward compat: existing alarm has status=="active"
// ===========================================================================

/// User story: US-PA-040 migration
/// Covers: Inserting an alarm via the DB `insert_alarm` method (which now defaults status to
///         'active' per migration) and verifying via the API that status == "active".
///         This confirms the DB migration correctly sets status defaults for new records.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_backward_compat_existing_alarm_has_status(ctx: &mut TestContext) {
    let product_id = "ext_compat_status";

    // Insert alarm directly via DB (no API involvement)
    let id = insert_test_alarm(
        ctx,
        product_id,
        "dev_compat_status",
        1,
        "Migration compat alarm",
    )
    .await;

    // Fetch via API list endpoint
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let alarm = data
        .iter()
        .find(|r| r["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("Alarm {id} not found in list response"));

    assert_eq!(
        alarm["status"], "active",
        "DB-inserted alarm must have status=active via API (migration default)"
    );
    assert!(
        alarm["cleared_at"].is_null(),
        "Active alarm must have cleared_at=null"
    );
}

// ===========================================================================
// c. Update nonexistent alarm clear returns 404
// ===========================================================================

/// User story: US-PA-041 negative
/// Covers: PATCH /api/admin/alarm/999999/clear on a nonexistent alarm returns 404 Not Found.
///         The clear endpoint must check existence before checking status.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_nonexistent_alarm_clear_returns_404(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::PATCH, "/api/admin/alarm/999999/clear").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Clearing nonexistent alarm must return 404"
    );
}

// ===========================================================================
// d. Create rule with clear_condition null by default
// ===========================================================================

/// User story: US-PA-039
/// Covers: Creating a property rule without `clear_condition` in the request body
///         results in data.clear_condition == null in the response. This confirms the
///         default behavior: no auto-clear condition.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_rule_with_clear_condition_null_by_default(ctx: &mut TestContext) {
    let model_no = "ext_clear_null";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Rule without clear condition",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;

    assert_eq!(status, StatusCode::CREATED);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert!(
        resp["data"]["clear_condition"].is_null(),
        "clear_condition must be null by default when omitted in create request"
    );
}

// ===========================================================================
// e. Update rule set clear_condition
// ===========================================================================

/// User story: US-PA-039
/// Covers: Create a property rule without clear_condition, then PATCH to set
///         clear_condition to {"operator": "<=", "value": 25}. Verify via GET that
///         clear_condition is stored correctly.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_rule_set_clear_condition(ctx: &mut TestContext) {
    let model_no = "ext_set_clear";
    create_test_product(ctx, model_no).await;

    // Create without clear_condition
    let body = json!({
        "product_id": model_no,
        "name": "Rule to add clear condition",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // Verify initially null
    assert!(
        create_resp["data"]["clear_condition"].is_null(),
        "clear_condition must be null before update"
    );

    // PATCH to set clear_condition
    let update_body = json!({
        "clear_condition": { "operator": "<=", "value": 25 }
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "PATCH to set clear_condition must return 200"
    );

    // GET and verify clear_condition is set correctly
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert!(
        resp["data"]["clear_condition"].is_object(),
        "clear_condition must be an object after update"
    );
    assert_eq!(resp["data"]["clear_condition"]["operator"], "<=");
    assert_eq!(resp["data"]["clear_condition"]["value"], 25);
}

// ===========================================================================
// f. Update non-property rule accepts duration without error
// ===========================================================================

/// Covers: Update endpoint rejects duration_minutes > 0 on non-property rules,
///         consistent with create-time validation. The handler checks existing.trigger_type
///         and returns 400 if duration_minutes > 0 is sent for a non-property rule.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_non_property_rule_accepts_duration_without_error(ctx: &mut TestContext) {
    let model_no = "ext_event_dur";
    create_test_product(ctx, model_no).await;

    // Create event-type rule without duration or clear_condition
    let body = json!({
        "product_id": model_no,
        "name": "Event rule for duration update",
        "trigger_type": "event",
        "trigger_config": { "event_identifier": "error" },
        "condition": { "operator": "always" },
        "actions": [{ "type": "alarm", "level": "info" }],
        "throttle_minutes": 0
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Event rule creation must succeed"
    );
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // PATCH to set duration_minutes = 5 on non-property rule
    // Update handler validates trigger_type and rejects duration_minutes > 0 for event rules.
    let update_body = json!({ "duration_minutes": 5 });
    let (status, _resp_body) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "PATCH duration_minutes > 0 on event rule must return 400 (trigger_type validation on update)"
    );
}

// ===========================================================================
// g. Backward compat: acknowledged filter still works
// ===========================================================================

/// User story: US-PA-040 compatibility
/// Covers: Insert 3 alarms with different statuses (active, acknowledged, cleared).
///         GET with acknowledged=false returns only the active alarm (status == "active").
///         GET with acknowledged=true returns BOTH acknowledged AND cleared alarms
///         (design mapping: acknowledged=true means status != 'active', inclusive of cleared).
///         This ensures the legacy acknowledged query parameter still works correctly
///         after the status migration.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_backward_compat_acknowledged_filter_still_works(ctx: &mut TestContext) {
    let product_id = "ext_compat_ack";

    // Insert active alarm (default status from DB)
    let id_active = insert_test_alarm(
        ctx,
        product_id,
        "dev_active_ack",
        0,
        "Active alarm for ack compat",
    )
    .await;

    // Insert alarm and ack it (status becomes "acknowledged")
    let id_acknowledged = insert_test_alarm(
        ctx,
        product_id,
        "dev_acked_ack",
        1,
        "Acked alarm for ack compat",
    )
    .await;
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id_acknowledged}/ack"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Ack must succeed for setup");

    // Insert alarm and clear it (status becomes "cleared")
    let id_cleared = insert_test_alarm(
        ctx,
        product_id,
        "dev_cleared_ack",
        2,
        "Cleared alarm for ack compat",
    )
    .await;
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id_cleared}/clear"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Clear must succeed for setup");

    // Test acknowledged=false: should return only active alarms (status == "active")
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&acknowledged=false&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let returned_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();

    assert!(
        returned_ids.contains(&id_active),
        "acknowledged=false must include active alarm"
    );
    assert!(
        !returned_ids.contains(&id_acknowledged),
        "acknowledged=false must exclude acknowledged alarm"
    );
    assert!(
        !returned_ids.contains(&id_cleared),
        "acknowledged=false must exclude cleared alarm"
    );

    // Test acknowledged=true: should return acknowledged AND cleared alarms (status != "active")
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&acknowledged=true&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    let returned_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();

    assert!(
        returned_ids.contains(&id_acknowledged),
        "acknowledged=true must include acknowledged alarm"
    );
    assert!(
        returned_ids.contains(&id_cleared),
        "acknowledged=true must include cleared alarm (status != 'active' is inclusive of cleared)"
    );
    assert!(
        !returned_ids.contains(&id_active),
        "acknowledged=true must exclude active alarm"
    );
}
