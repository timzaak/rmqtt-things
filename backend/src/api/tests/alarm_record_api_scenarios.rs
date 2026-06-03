//! Scenario tests for alarm record Admin APIs.
//!
//! Covers:
//! - List alarm records with pagination and multi-dimensional filters
//!   (product, device, level, acknowledged status)
//! - Acknowledge an alarm record
//! - Negative scenarios: already-acknowledged conflict, nonexistent alarm

use super::simple_tests::TestContext;
use super::simple_tests::request;
use axum::http::{Method, StatusCode};
use test_context::test_context;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Insert a test alarm record directly into the database and return its id.
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
        )
        .await
        .expect("insert_test_alarm failed")
}

/// Acknowledge an alarm directly via the database helper (for setup).
async fn ack_alarm_direct(ctx: &TestContext, id: i64) {
    ctx._admin_state
        .db
        .alarm()
        .ack_alarm(id)
        .await
        .expect("ack_alarm_direct failed");
}

// ===========================================================================
// List query scenarios
// ===========================================================================

/// User story: US-PA-034
/// Covers: Scenario 1 -- Admin lists alarm records; after inserting 3 records the list
///         returns at least 3 entries and pagination.total >= 3, ordered by created_at DESC.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarm_records(ctx: &mut TestContext) {
    let product_id = "alarm_list_prod";
    for i in 1..=3 {
        insert_test_alarm(
            ctx,
            product_id,
            &format!("dev_{i}"),
            0,
            &format!("Alarm {i}"),
        )
        .await;
    }

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        "/api/admin/alarm?page=1&page_size=50",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.len() >= 3,
        "Expected at least 3 alarm records, got {}",
        data.len()
    );
    let total = resp["pagination"]["total"]
        .as_i64()
        .expect("pagination.total required");
    assert!(total >= 3, "Expected pagination.total >= 3, got {total}");

    // Verify each record has status and cleared_at fields
    for record in data {
        assert!(
            record["status"].is_string(),
            "Alarm record must contain status field as string"
        );
        assert!(
            record["cleared_at"].is_null() || record["cleared_at"].is_string(),
            "cleared_at must be null or RFC3339 string"
        );
    }

    // Verify descending order by created_at
    let timestamps: Vec<&str> = data
        .iter()
        .filter_map(|r| r["created_at"].as_str())
        .collect();
    for window in timestamps.windows(2) {
        assert!(
            window[0] >= window[1],
            "Alarm records must be ordered by created_at DESC"
        );
    }
}

/// User story: US-PA-034
/// Covers: Scenario 2 -- Admin filters alarm records by product_id;
///         only records belonging to the specified product are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_filter_by_product(ctx: &mut TestContext) {
    let product_a = "alarm_fprod_a";
    let product_b = "alarm_fprod_b";

    insert_test_alarm(ctx, product_a, "dev_a1", 0, "Alarm A-1").await;
    insert_test_alarm(ctx, product_a, "dev_a2", 1, "Alarm A-2").await;
    insert_test_alarm(ctx, product_b, "dev_b1", 2, "Alarm B-1").await;

    // Filter by product A
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_a}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["product_id"] == product_a),
        "All returned records must belong to product {product_a}"
    );
    assert!(data.len() >= 2, "Expected at least 2 records for product A");

    // Filter by product B
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_b}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["product_id"] == product_b),
        "All returned records must belong to product {product_b}"
    );
    assert!(!data.is_empty(), "Expected at least 1 record for product B");
}

/// User story: US-PA-034
/// Covers: Scenario 1 (device dimension) -- Admin filters alarm records by device_id;
///         only records belonging to the specified device are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_filter_by_device(ctx: &mut TestContext) {
    let product_id = "alarm_fdev_prod";
    let device_x = "device_x";
    let device_y = "device_y";

    insert_test_alarm(ctx, product_id, device_x, 0, "Alarm for X").await;
    insert_test_alarm(ctx, product_id, device_y, 1, "Alarm for Y").await;
    insert_test_alarm(ctx, product_id, device_x, 2, "Another alarm for X").await;

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?device_id={device_x}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["device_id"] == device_x),
        "All returned records must belong to device {device_x}"
    );
    assert!(data.len() >= 2, "Expected at least 2 records for device X");
}

/// User story: US-PA-034
/// Covers: Scenario 3 -- Admin filters alarm records by acknowledged=false;
///         only unacknowledged records are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_filter_by_acknowledged(ctx: &mut TestContext) {
    let product_id = "alarm_fack_prod";

    let id_unack_1 = insert_test_alarm(ctx, product_id, "dev_u1", 0, "Unack 1").await;
    let id_unack_2 = insert_test_alarm(ctx, product_id, "dev_u2", 1, "Unack 2").await;
    let id_ack = insert_test_alarm(ctx, product_id, "dev_a1", 2, "Acked").await;

    // Acknowledge one alarm directly
    ack_alarm_direct(ctx, id_ack).await;

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&acknowledged=false&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["acknowledged"] == false),
        "All returned records must have acknowledged=false"
    );
    // The two unacknowledged alarms should be present
    let returned_ids: Vec<i64> = data.iter().filter_map(|r| r["id"].as_i64()).collect();
    assert!(
        returned_ids.contains(&id_unack_1) && returned_ids.contains(&id_unack_2),
        "Both unacknowledged alarm ids must appear in results"
    );
    assert!(
        !returned_ids.contains(&id_ack),
        "Acknowledged alarm must not appear when filtering acknowledged=false"
    );
}

/// User story: US-PA-034
/// Covers: Scenario 1 (pagination) -- Admin paginates alarm records;
///         page=1&page_size=2 returns exactly 2 items and total >= 5.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_pagination(ctx: &mut TestContext) {
    let product_id = "alarm_page_prod";
    for i in 1..=5 {
        insert_test_alarm(
            ctx,
            product_id,
            &format!("dev_pg_{i}"),
            0,
            &format!("Page alarm {i}"),
        )
        .await;
    }

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&page=1&page_size=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert_eq!(
        data.len(),
        2,
        "Expected exactly 2 items on page 1 with page_size=2"
    );
    let total = resp["pagination"]["total"]
        .as_i64()
        .expect("pagination.total required");
    assert!(total >= 5, "Expected pagination.total >= 5, got {total}");
    // Compute has_next from total, page, page_size since PaginationInfo does not include it
    let has_next = total > 2;
    assert!(
        has_next,
        "Expected has_next=true when total > page * page_size"
    );
}

/// User story: US-PA-034
/// Covers: Scenario 1 (level dimension) -- Admin filters alarm records by level="critical";
///         only critical-level records are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_list_alarms_filter_by_level(ctx: &mut TestContext) {
    let product_id = "alarm_flvl_prod";

    insert_test_alarm(ctx, product_id, "dev_lv_info", 0, "Info alarm").await; // info = 0
    insert_test_alarm(ctx, product_id, "dev_lv_warn", 1, "Warning alarm").await; // warning = 1
    insert_test_alarm(ctx, product_id, "dev_lv_crit", 2, "Critical alarm").await; // critical = 2

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/alarm?product_id={product_id}&level=critical&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let data = resp["data"].as_array().expect("data must be array");
    assert!(
        data.iter().all(|r| r["level"] == "critical"),
        "All returned records must have level=critical"
    );
    assert!(
        !data.is_empty(),
        "Expected at least 1 critical alarm record"
    );
}

// ===========================================================================
// Acknowledge operation scenarios
// ===========================================================================

/// User story: US-PA-035
/// Covers: Scenario 1 -- Admin acknowledges an unacknowledged alarm;
///         PATCH returns 200 and data.acknowledged=true.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_ack_alarm(ctx: &mut TestContext) {
    let product_id = "alarm_ack_prod";
    let id = insert_test_alarm(ctx, product_id, "dev_ack", 1, "Ack this alarm").await;

    let (status, body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(resp["data"]["id"], id);
    assert_eq!(resp["data"]["acknowledged"], true);
    assert_eq!(
        resp["data"]["status"], "acknowledged",
        "After ack, status must transition from active to acknowledged"
    );
}

/// User story: US-PA-035
/// Covers: Scenario 2 -- Admin re-acknowledges an already-acknowledged alarm;
///         PATCH returns 409 Conflict because the status check (status != "active") rejects
///         non-active alarms. The API must NOT accept idempotent 200.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_ack_already_acknowledged_alarm(ctx: &mut TestContext) {
    let product_id = "alarm_reack_prod";
    let id = insert_test_alarm(ctx, product_id, "dev_reack", 1, "Already acked").await;

    // First ack succeeds (status: active -> acknowledged)
    let (status, _) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "First ack should return 200");

    // Second ack must return 409 Conflict (status is "acknowledged", not "active")
    let (status, _body) = request(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/alarm/{id}/ack"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Re-acknowledging must return 409 Conflict (status != active), got {status}"
    );
}

/// User story: US-PA-035
/// Covers: Negative -- Acknowledging a nonexistent alarm returns 404 Not Found.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_ack_nonexistent_alarm(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::PATCH, "/api/admin/alarm/999999/ack").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
