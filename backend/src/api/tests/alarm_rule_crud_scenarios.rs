//! Scenario tests for alarm rule CRUD APIs.
//!
//! Covers:
//! - Create alarm rule (property threshold type)
//! - List alarm rules with and without product filter
//! - Get alarm rule by id
//! - Update alarm rule
//! - Enable / disable alarm rule
//! - Delete alarm rule
//! - Negative scenarios: invalid trigger type, nonexistent rule

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
        "description": "Test product for alarm rule scenarios"
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

/// User story: US-PA-029
/// Covers: Scenario 1 -- Admin creates a property threshold alarm rule.
///         The response returns 201 with data containing id, product_id, and enabled=true.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_alarm_rule_property_type(ctx: &mut TestContext) {
    let model_no = "alarm_create_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Temperature too high", 50);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;

    assert_eq!(status, StatusCode::CREATED);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert!(
        resp["data"]["id"].is_number(),
        "Response data must contain id"
    );
    assert_eq!(resp["data"]["product_id"], model_no);
    assert_eq!(resp["data"]["enabled"], true);
    assert_eq!(resp["data"]["trigger_type"], "property");
    assert_eq!(
        resp["data"]["duration_minutes"], 0,
        "duration_minutes must default to 0 when omitted"
    );
    assert!(
        resp["data"]["clear_condition"].is_null(),
        "clear_condition must default to null when omitted"
    );
}

/// User story: US-PA-030
/// Covers: Scenario 1 -- Admin lists alarm rules; after creating 2 rules the list
///         returns at least 2 entries with correct pagination.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarm_rules(ctx: &mut TestContext) {
    let model_no = "alarm_list_prod";
    create_test_product(ctx, model_no).await;

    // Create 2 rules
    for i in 1..=2 {
        let body = make_property_rule_body(model_no, &format!("List rule {i}"), 40 + i);
        let (status, _) =
            request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        "/api/admin/alarm-rule?page=1&page_size=50",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.len() >= 2,
        "Expected at least 2 rules, got {}",
        data.len()
    );
    let total = resp["pagination"]["total"]
        .as_i64()
        .expect("pagination.total required");
    assert!(total >= 2, "Expected pagination.total >= 2, got {total}");
}

/// User story: US-PA-030
/// Covers: Scenario 2 -- Admin filters alarm rules by product_id;
///         only rules belonging to the specified product are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarm_rules_filter_by_product(ctx: &mut TestContext) {
    let model_a = "alarm_filter_prod_a";
    let model_b = "alarm_filter_prod_b";
    create_test_product(ctx, model_a).await;
    create_test_product(ctx, model_b).await;

    // Create 1 rule for product A, 2 rules for product B
    let body_a = make_property_rule_body(model_a, "Filter rule A", 30);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body_a).await;
    assert_eq!(status, StatusCode::CREATED);

    for i in 1..=2 {
        let body_b = make_property_rule_body(model_b, &format!("Filter rule B-{i}"), 40 + i);
        let (status, _) =
            request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body_b).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // Filter by product A
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule?product_id={model_a}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["product_id"] == model_a),
        "All returned rules must belong to product {model_a}"
    );
    assert!(!data.is_empty(), "Expected at least 1 rule for product A");

    // Filter by product B
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule?product_id={model_b}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["product_id"] == model_b),
        "All returned rules must belong to product {model_b}"
    );
    assert!(data.len() >= 2, "Expected at least 2 rules for product B");
}

/// User story: US-PA-030
/// Covers: Scenario 1 -- Admin gets alarm rule detail by id;
///         the returned object contains all rule fields.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_get_alarm_rule_by_id(ctx: &mut TestContext) {
    let model_no = "alarm_get_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Get detail rule", 75);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["id"], rule_id);
    assert_eq!(resp["data"]["product_id"], model_no);
    assert_eq!(resp["data"]["name"], "Get detail rule");
    assert_eq!(resp["data"]["trigger_type"], "property");
    assert_eq!(resp["data"]["enabled"], true);
    assert_eq!(
        resp["data"]["duration_minutes"], 0,
        "duration_minutes must be 0 by default"
    );
    assert!(
        resp["data"]["clear_condition"].is_null(),
        "clear_condition must be null by default"
    );
}

/// User story: US-PA-031
/// Covers: Scenario 1 -- Admin updates alarm rule condition value;
///         GET detail confirms the value has been updated.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_alarm_rule(ctx: &mut TestContext) {
    let model_no = "alarm_update_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Update rule", 50);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // Update the condition.value to 99
    let update_body = json!({
        "condition": { "operator": ">", "value": 99 }
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}"),
        &update_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify via GET
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["condition"]["value"], 99);
}

/// User story: US-PA-032
/// Covers: Scenario 1 -- Admin disables an alarm rule;
///         GET detail confirms enabled=false.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_disable_alarm_rule(ctx: &mut TestContext) {
    let model_no = "alarm_disable_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Disable rule", 60);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // Disable
    let status_body = json!({ "enabled": false });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}/status"),
        &status_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify via GET
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["enabled"], false);
}

/// User story: US-PA-032
/// Covers: Scenario 2 -- Admin re-enables a previously disabled alarm rule;
///         GET detail confirms enabled=true.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_enable_alarm_rule(ctx: &mut TestContext) {
    let model_no = "alarm_enable_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Enable rule", 60);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // Disable first
    let disable_body = json!({ "enabled": false });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}/status"),
        &disable_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Re-enable
    let enable_body = json!({ "enabled": true });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm-rule/{rule_id}/status"),
        &enable_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify via GET
    let (status, resp_body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    assert_eq!(resp["data"]["enabled"], true);
}

/// User story: US-PA-033
/// Covers: Scenario 1 -- Admin deletes an alarm rule;
///         DELETE returns 204 and subsequent GET returns 404.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_delete_alarm_rule(ctx: &mut TestContext) {
    let model_no = "alarm_delete_prod";
    create_test_product(ctx, model_no).await;

    let body = make_property_rule_body(model_no, "Delete rule", 80);
    let (status, resp_body) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let create_resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
    let rule_id = extract_rule_id(&create_resp);

    // Delete
    let (status, _) = request(
        &ctx.service,
        Method::DELETE,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // GET should return 404
    let (status, _) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm-rule/{rule_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ===========================================================================
// Negative scenarios
// ===========================================================================

/// User story: US-PA-029
/// Covers: Negative -- Creating a rule with an invalid trigger_type returns 400 Bad Request.
///         Note: This test asserts that the API validates trigger_type. If the current
///         implementation does not validate trigger_type, this test will fail and the
///         validation gap should be addressed in a dev item.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_rule_invalid_trigger_type(ctx: &mut TestContext) {
    let model_no = "alarm_invalid_prod";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Invalid trigger type rule",
        "trigger_type": "invalid",
        "trigger_config": {},
        "condition": { "operator": "always" },
        "actions": [{ "type": "alarm", "level": "info" }]
    });
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// User story: US-PA-030
/// Covers: Negative -- GET a nonexistent alarm rule by id returns 404 Not Found.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_get_nonexistent_rule(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::GET, "/api/admin/alarm-rule/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// User story: US-PA-031
/// Covers: Negative -- PATCH a nonexistent alarm rule returns 404 Not Found.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_update_nonexistent_rule(ctx: &mut TestContext) {
    let update_body = json!({
        "condition": { "operator": ">", "value": 10 }
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        "/api/admin/alarm-rule/999999",
        &update_body,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// User story: US-PA-033
/// Covers: Negative -- DELETE a nonexistent alarm rule returns 404 Not Found.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_delete_nonexistent_rule(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::DELETE, "/api/admin/alarm-rule/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Negative: device_online / device_offline triggers must use the `always`
/// condition operator. The rule engine evaluates device-status triggers
/// against a fixed Null actual_value (rule_engine/mod.rs), so any non-always
/// operator would silently make the rule never fire. The API must reject such
/// misconfiguration up front (PRD alarm-rule-engine.md §5.3, P2-13).
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_device_online_rule_non_always_rejected(ctx: &mut TestContext) {
    let model_no = "alarm_dev_online_prod";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Device online with bogus condition",
        "trigger_type": "device_online",
        "trigger_config": {},
        // A comparison operator makes no sense for a device-status trigger;
        // the rule would never fire.
        "condition": { "operator": ">", "value": 1 },
        "actions": [{ "type": "alarm", "level": "info" }]
    });
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "device_online rule with a non-always condition must be rejected"
    );
}

/// Positive companion: device_online with `always` is accepted.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_create_device_online_rule_always_accepted(ctx: &mut TestContext) {
    let model_no = "alarm_dev_online_ok_prod";
    create_test_product(ctx, model_no).await;

    let body = json!({
        "product_id": model_no,
        "name": "Device online always rule",
        "trigger_type": "device_online",
        "trigger_config": {},
        "condition": { "operator": "always" },
        "actions": [{ "type": "alarm", "level": "info" }]
    });
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/alarm-rule", &body).await;
    assert_eq!(status, StatusCode::CREATED);
}
