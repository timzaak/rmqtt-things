//! Scenario tests for alarm rule duration condition and clear condition APIs.
//!
//! Covers:
//! - Create rule with duration_minutes and clear_condition
//! - Default duration_minutes is 0 when omitted
//! - Update rule to add/change duration_minutes and clear_condition
//! - Update rule to clear (null) clear_condition
//! - Non-property trigger type rejects duration_minutes > 0
//! - Non-property trigger type rejects clear_condition
//! - Negative duration_minutes returns 400

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
        "description": "Test product for alarm duration scenarios"
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

/// Build a minimal property-threshold alarm rule JSON body.
fn make_property_rule_body(
    product_id: &str,
    name: &str,
    threshold_value: i64,
) -> serde_json::Value {
    json!({
        "product_id": product_id,
        "name": name,
        "description": "Auto-generated property threshold rule",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": threshold_value },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5
    })
}

/// Extract the rule id from a create response.
fn extract_rule_id(resp: &serde_json::Value) -> i64 {
    resp["data"]["id"]
        .as_i64()
        .expect("Rule response must contain data.id")
}

// ===========================================================================
// Positive scenarios
// ===========================================================================

/// User story: US-PA-038
/// Covers: Create property rule with duration_minutes and clear_condition.
///         The response returns 201 with data.duration_minutes == 2 and
///         data.clear_condition present.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_rule_with_duration_and_clear_condition(ctx: &mut TestContext) {
    let model_no = "alarm_dur_create";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Duration rule",
        "description": "Rule with duration and clear condition",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5,
        "duration_minutes": 2,
        "clear_condition": { "operator": "<", "value": 30 }
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;

    assert_eq!(status, StatusCode::CREATED);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["duration_minutes"], 2);
    assert!(
        resp["data"]["clear_condition"].is_object(),
        "clear_condition must be present as an object"
    );
    assert_eq!(resp["data"]["clear_condition"]["operator"], "<");
    assert_eq!(resp["data"]["clear_condition"]["value"], 30);
}

/// User story: US-PA-038 backward compat
/// Covers: Create property rule without duration_minutes field;
///         the response returns 201 with data.duration_minutes == 0 (DB default).
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_rule_duration_default_zero(ctx: &mut TestContext) {
    let model_no = "alarm_dur_default";
    create_test_product(ctx, model_no).await;

    // Omit duration_minutes entirely
    let body = make_property_rule_body(model_no, "Default duration rule", 50);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;

    assert_eq!(status, StatusCode::CREATED);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(
        resp["data"]["duration_minutes"], 0,
        "duration_minutes must default to 0 when omitted"
    );
}

/// User story: US-PA-038
/// Covers: Create rule with duration_minutes=0, then PATCH to set duration_minutes=5;
///         GET the rule and verify duration_minutes == 5.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_rule_add_duration(ctx: &mut TestContext) {
    let model_no = "alarm_dur_update";
    create_test_product(ctx, model_no).await;

    // Create with default duration
    let body = make_property_rule_body(model_no, "Update duration rule", 50);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // PATCH to set duration_minutes = 5
    let update_body = json!({ "duration_minutes": 5 });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // GET to verify
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["duration_minutes"], 5);
}

/// User story: US-PA-038
/// Covers: Create rule with clear_condition, then PATCH to set clear_condition = null;
///         GET the rule and verify clear_condition is null.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_rule_clear_duration_set_null(ctx: &mut TestContext) {
    let model_no = "alarm_dur_null";
    create_test_product(ctx, model_no).await;

    // Create with clear_condition
    let body = json!({
        "product_id": model_no,
        "name": "Clear condition null rule",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "throttle_minutes": 5,
        "clear_condition": { "operator": "<", "value": 30 }
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // PATCH to set clear_condition = null
    // UpdateAlarmRuleRequest.clear_condition: Some(None) means clear to null
    let update_body = json!({ "clear_condition": null });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // GET to verify clear_condition is null
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert!(
        resp["data"]["clear_condition"].is_null(),
        "clear_condition must be null after PATCH with null"
    );
}

// ===========================================================================
// Negative scenarios
// ===========================================================================

/// User story: US-PA-038 validation
/// Covers: Creating an event-type rule with duration_minutes: 5 returns 400 Bad Request.
///         Only property trigger type supports duration > 0.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_non_property_rule_rejects_duration(ctx: &mut TestContext) {
    let model_no = "alarm_dur_event";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Event rule with duration",
        "trigger_type": "event",
        "trigger_config": { "event_identifier": "error" },
        "condition": { "operator": "always" },
        "actions": [{ "type": "alarm", "level": "info" }],
        "duration_minutes": 5
    });
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Non-property rule with duration_minutes > 0 must return 400"
    );
}

/// User story: US-PA-039 validation
/// Covers: Creating a device_online rule with clear_condition returns 400 Bad Request.
///         Only property trigger type supports clear_condition.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_non_property_rule_rejects_clear_condition(ctx: &mut TestContext) {
    let model_no = "alarm_dur_clear";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Device online with clear condition",
        "trigger_type": "device_online",
        "trigger_config": {},
        "condition": { "operator": "always" },
        "actions": [{ "type": "alarm", "level": "info" }],
        "clear_condition": { "operator": "always" }
    });
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Non-property rule with clear_condition must return 400"
    );
}

/// User story: US-PA-038 validation
/// Covers: Creating a property rule with duration_minutes: -1 returns 400 Bad Request.
///         Error message mentions "duration_minutes 必须 >= 0".
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_negative_duration_returns_400(ctx: &mut TestContext) {
    let model_no = "alarm_dur_neg";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Negative duration rule",
        "trigger_type": "property",
        "trigger_config": { "property_name": "temperature" },
        "condition": { "operator": ">", "value": 50 },
        "actions": [{ "type": "alarm", "level": "warning" }],
        "duration_minutes": -1
    });
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let error_msg = resp["error"].as_str().unwrap_or("").to_lowercase();
    assert!(
        error_msg.contains("duration_minutes"),
        "Error message must mention duration_minutes, got: {error_msg}"
    );
}

/// User story: US-PA-038 validation
/// Covers: PATCH a property rule with duration_minutes: -5 returns 400 Bad Request.
///         Update validation must also reject negative values.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_rule_negative_duration_returns_400(ctx: &mut TestContext) {
    let model_no = "alarm_dur_neg_update";
    create_test_product(ctx, model_no).await;

    // Create a valid property rule first
    let body = make_property_rule_body(model_no, "Neg update rule", 50);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // PATCH with negative duration
    let update_body = json!({ "duration_minutes": -5 });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "PATCH with negative duration_minutes must return 400"
    );
}
