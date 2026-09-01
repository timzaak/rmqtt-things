//! Scenario tests for the property history chart endpoints
//! (`GET /api/admin/property/history/keys` and `/series`).
//!
//! Covers US-PA-052 (以折线图查看设备属性历史趋势) against the chart
//! discovery/series handlers, the every-Nth real-record sampling and the
//! effective-time axis (`COALESCE(reported_time, created_time)`).
//!
//! Test style mirrors `shadow_scenarios.rs`: in-process axum
//! `#[test_context(TestContext)]` + `#[tokio::test]`, reusing
//! `super::simple_tests::{request, TestContext}`. Reports with controlled
//! `reported_time` are seeded through the production write path
//! (`upsert_property_latest`); rows that path cannot produce (NULL
//! `reported_time`, >1000-row bulk seed) go through schema-qualified raw SQL
//! on `ctx._admin_pool`.
//!
//! Business rules encoded:
//! - R1 only all-numeric keys qualify for the chart (type-drift keys excluded).
//! - R2 every returned point maps 1:1 to a real reported record; sampling
//!   outputs a subset of real rows, never synthesized points.
//! - R3 strict closed `[start_time, end_time]` bounds; outside data never leaks.
//! - R4 over-cap ranges are downsampled with an explicit
//!   `downsampled/stride/totalPoints` contract instead of being truncated.

use super::simple_tests::TestContext;
use super::simple_tests::request;
use crate::api::admin_handlers::{normalize_series_keys, parse_series_range, series_stride};
use axum::http::{Method, StatusCode};
use serde_json::{Map, Value as JsonValue, json};
use test_context::test_context;
use time::OffsetDateTime;
use time::ext::NumericalDuration;

// --- shared helpers ---

fn object(value: &JsonValue) -> Map<String, JsonValue> {
    value
        .as_object()
        .expect("seed payload must be an object")
        .clone()
}

/// Seed one property report through the production write path so that
/// `reported_time` (and thus the chart time axis) is fully controlled.
async fn seed_report(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
    params: &JsonValue,
    ts: OffsetDateTime,
) {
    ctx._admin_state
        .db
        .upsert_property_latest(product_id, device_id, object(params), ts)
        .await
        .unwrap();
}

fn rfc3339(ts: OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

fn keys_uri(product_id: &str, device_id: &str, lookback_days: Option<i32>) -> String {
    match lookback_days {
        Some(days) => format!(
            "/api/admin/property/history/keys?product_id={product_id}&device_id={device_id}&lookback_days={days}"
        ),
        None => format!(
            "/api/admin/property/history/keys?product_id={product_id}&device_id={device_id}"
        ),
    }
}

fn series_uri(product_id: &str, device_id: &str, keys: &[&str], start: &str, end: &str) -> String {
    let keys_q = keys
        .iter()
        .map(|k| format!("keys={k}"))
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "/api/admin/property/history/series?product_id={product_id}&device_id={device_id}&{keys_q}&start_time={start}&end_time={end}"
    )
}

async fn get_json(ctx: &TestContext, uri: &str) -> (StatusCode, JsonValue) {
    let (status, text) = request(&ctx.service, Method::GET, uri).await;
    let json = if text.is_empty() {
        JsonValue::Null
    } else {
        serde_json::from_str(&text).unwrap_or(JsonValue::Null)
    };
    (status, json)
}

/// Parse an RFC3339 point time back to `OffsetDateTime` for exact comparison.
fn point_time(point: &JsonValue) -> OffsetDateTime {
    OffsetDateTime::parse(
        point["time"]
            .as_str()
            .expect("point.time must be an RFC3339 string"),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Scenario 1: keys discovery only returns keys whose samples are ALL numeric.
//
// User Story: US-PA-052 场景3 (非数值属性不可入图), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: R1 (仅数值属性可入图；属性发现数据驱动，无物模型注册表).
// WHY encoded: a type-drift key (number in one report, string in another)
// would draw mixed-semantics data into one line and mislead the admin; any
// non-numeric sample must exclude the whole key.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_keys_discovery_returns_only_all_numeric_keys(ctx: &mut TestContext) {
    let product_id = "chart_product_keys";
    let device_id = "chart_device_keys";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    // temperature: 3 numeric samples; humidity: 2 numeric samples; mode/flag/
    // nested: never numeric; drift: numeric once, string once (type drift).
    let mixed = json!({
        "temperature": 21,
        "humidity": 55,
        "mode": "eco",
        "flag": true,
        "nested": { "inner": 1 }
    });
    seed_report(ctx, product_id, device_id, &mixed, anchor - 3.hours()).await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 22, "humidity": 60, "mode": "comfort" }),
        anchor - 2.hours(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 23 }),
        anchor - 1.hours(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "drift": 7 }),
        anchor - 1.hours(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "drift": "high" }),
        anchor - 30.minutes(),
    )
    .await;

    let (status, body) = get_json(ctx, &keys_uri(product_id, device_id, None)).await;
    assert_eq!(status, StatusCode::OK, "keys discovery should return 200");

    let data = body["data"].as_array().expect("data must be an array");
    let keys: Vec<(&str, i64)> = data
        .iter()
        .map(|k| {
            (
                k["key"].as_str().unwrap(),
                k["sampleCount"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![("temperature", 3), ("humidity", 2)],
        "only all-numeric keys qualify, ordered by sampleCount desc"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: series returns real raw points strictly within the closed range.
//
// User Story: US-PA-052 场景1 主成功 (选属性与时间范围看趋势), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: R2 (数据点一一对应真实上报记录) + R3 (闭区间时间边界).
// WHY encoded: if a boundary record leaks in or an in-range record drops out,
// the admin sees a curve that never happened; bounds are inclusive on both
// ends and outside data must never appear.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_returns_raw_points_within_bounds(ctx: &mut TestContext) {
    let product_id = "chart_product_bounds";
    let device_id = "chart_device_bounds";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let start = anchor - 2.hours();
    let end = anchor + 2.hours();

    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 0 }),
        start - 1.hours(),
    )
    .await; // out (early)
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 10, "voltage": 220.5 }),
        start,
    )
    .await; // boundary in
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 20 }),
        anchor,
    )
    .await; // in
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 30 }),
        end,
    )
    .await; // boundary in
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 99 }),
        end + 1.hours(),
    )
    .await; // out (late)

    // Multi-key request: response order must match the request keys order,
    // and a key reported only once in range yields a single-point series.
    let (status, body) = get_json(
        ctx,
        &series_uri(
            product_id,
            device_id,
            &["voltage", "temperature"],
            &rfc3339(start),
            &rfc3339(end),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "series query should return 200");

    let data = body["data"].as_array().expect("data must be an array");
    assert_eq!(
        data.len(),
        2,
        "one series per requested key, in request order"
    );
    assert_eq!(data[0]["key"], "voltage");
    assert_eq!(data[1]["key"], "temperature");

    let voltage_points = data[0]["points"].as_array().unwrap();
    assert_eq!(
        voltage_points.len(),
        1,
        "voltage was reported once in range"
    );
    // R2: value passthrough keeps the original JSON precision (float, no cast).
    assert_eq!(voltage_points[0]["value"].as_f64(), Some(220.5));
    assert_eq!(point_time(&voltage_points[0]), start);

    let series = &data[1];
    assert_eq!(
        series["totalPoints"], 3,
        "both boundary records count (closed range)"
    );
    assert_eq!(series["downsampled"], false);
    assert_eq!(series["stride"], 1);

    let points = series["points"].as_array().unwrap();
    assert_eq!(
        points.len(),
        3,
        "boundary-in records only; outside data must not leak"
    );
    let expected = [(start, 10), (anchor, 20), (end, 30)];
    for (i, (ts, value)) in expected.iter().enumerate() {
        assert_eq!(
            point_time(&points[i]),
            *ts,
            "point {i} time must be the seeded time"
        );
        assert_eq!(
            points[i]["value"].as_i64(),
            Some(*value),
            "point {i} value must be the seeded raw value"
        );
    }
    // Ascending order (chart x-axis contract).
    assert!(
        point_time(&points[0]) < point_time(&points[1])
            && point_time(&points[1]) < point_time(&points[2])
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: over-cap ranges are downsampled by every-Nth real-record
// sampling, never truncated and never synthesized.
//
// User Story: US-PA-052 场景4 (大时间跨度保持可用), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: R4 (超限按真实记录步长抽样而非截断).
// WHY encoded: a LIMIT-only truncation would silently drop the middle/end of
// the range (the admin misreads the trend); the contract must expose
// downsampled/stride/totalPoints and the returned points must all be real
// seeded records spread across the whole range.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_downsamples_when_over_cap(ctx: &mut TestContext) {
    let product_id = "chart_product_bulk";
    let device_id = "chart_device_bulk";
    let base = (OffsetDateTime::now_utc() - 2.days())
        .replace_nanosecond(0)
        .unwrap();
    let start = base - 1.hours();
    let end = base + 1600.minutes();

    // 1500 rows, one per minute, value == gs (1..=1500): the write path would
    // be far too slow for this volume, so seed with one schema-qualified bulk
    // INSERT on the raw pool.
    let total = 1500i64;
    sqlx::query(&format!(
        r#"INSERT INTO "{}".property_history (product_id, device_id, properties, reported_time)
           SELECT $1, $2, jsonb_build_object('bulk', gs), ($3::timestamptz) + make_interval(mins => gs::int)
           FROM generate_series(1, $4::bigint) AS gs"#,
        ctx.schema_name
    ))
    .bind(product_id)
    .bind(device_id)
    .bind(base)
    .bind(total)
    .execute(&ctx._admin_pool)
    .await
    .unwrap();

    let (status, body) = get_json(
        ctx,
        &series_uri(
            product_id,
            device_id,
            &["bulk"],
            &rfc3339(start),
            &rfc3339(end),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let series = &body["data"].as_array().expect("data must be an array")[0];
    assert_eq!(series["key"], "bulk");
    assert_eq!(
        series["totalPoints"], 1500,
        "count query must see every seeded row"
    );
    assert_eq!(series["downsampled"], true, "over-cap must be flagged");
    assert_eq!(series["stride"], 2, "stride = ceil(1500/1000)");

    let points = series["points"].as_array().unwrap();
    assert_eq!(points.len(), 750, "every 2nd real record: 1500/2 points");
    assert!(
        points.len() <= 1000,
        "hard cap must hold even if count drifts"
    );

    // First point is the earliest record (gs=1), last reaches the tail of the
    // range (gs=1499): sampling keeps range coverage, no mid-range truncation.
    assert_eq!(
        points[0]["value"].as_i64(),
        Some(1),
        "first point must be the earliest record"
    );
    assert_eq!(
        points.last().unwrap()["value"].as_i64(),
        Some(1499),
        "last point must reach the end of the range, not a truncated head"
    );
    assert_eq!(point_time(points.last().unwrap()), base + 1499.minutes());

    // R2: every returned point is a real seeded record (odd gs because
    // row_number aligns with the per-minute time order), times ascending.
    let mut last_time = None;
    for (i, point) in points.iter().enumerate() {
        let value = point["value"]
            .as_i64()
            .expect("value must stay a JSON number");
        assert!(
            (1..=1500).contains(&value) && value % 2 == 1,
            "point {i} must map to a seeded record"
        );
        let t = point_time(point);
        if let Some(prev) = last_time {
            assert!(prev < t, "points must be time-ascending at index {i}");
        }
        last_time = Some(t);
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: an empty range is a valid empty result, not an error.
//
// User Story: US-PA-052 场景2 (空态), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: R3 (范围内无数据展示空态) — the frontend distinguishes empty from
// error; a non-200 here would make the two states indistinguishable.
// Also covers the never-reported device empty keys response (PRD §4.2).
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_empty_range_is_not_error(ctx: &mut TestContext) {
    let product_id = "chart_product_empty";
    let device_id = "chart_device_empty";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    // No reports at all for this device in the queried range.
    let (status, body) = get_json(
        ctx,
        &series_uri(
            product_id,
            device_id,
            &["temperature"],
            &rfc3339(anchor + 10.days()),
            &rfc3339(anchor + 11.days()),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "empty range must be 200, not an error"
    );
    let series = &body["data"].as_array().unwrap()[0];
    assert_eq!(series["key"], "temperature");
    assert_eq!(series["points"].as_array().unwrap().len(), 0);
    assert_eq!(series["totalPoints"], 0);
    assert_eq!(series["downsampled"], false);
    assert_eq!(series["stride"], 1);

    // A device that never reported: keys discovery returns an empty list (the
    // frontend renders a guidance empty state, not an error).
    let (status, body) = get_json(ctx, &keys_uri(product_id, device_id, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// Scenario 5: a key whose in-range samples are all non-numeric yields an empty
// series, not an error.
//
// User Story: US-PA-052 场景3 (非数值属性不可入图), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: R1 applied to the series row filter — the key is visible in the
// table view but chart rows for it are silently skipped, matching the keys
// discovery judgment.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_non_numeric_key_returns_empty_series(ctx: &mut TestContext) {
    let product_id = "chart_product_nonnum";
    let device_id = "chart_device_nonnum";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "mode": "eco" }),
        anchor - 30.minutes(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "mode": "comfort" }),
        anchor - 10.minutes(),
    )
    .await;

    let (status, body) = get_json(
        ctx,
        &series_uri(
            product_id,
            device_id,
            &["mode"],
            &rfc3339(anchor - 1.hours()),
            &rfc3339(anchor + 1.hours()),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "non-numeric key is an empty series, not an error"
    );
    let series = &body["data"].as_array().unwrap()[0];
    assert_eq!(series["key"], "mode");
    assert_eq!(series["points"].as_array().unwrap().len(), 0);
    assert_eq!(series["totalPoints"], 0);
}

// ---------------------------------------------------------------------------
// Scenario 6: rows with NULL reported_time fall back to created_time on the
// chart time axis.
//
// User Story: US-PA-052 场景1 (真实上报数据点), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: effective-time semantics t = COALESCE(reported_time, created_time).
// WHY encoded: the write path always sets reported_time, but legacy/manual
// rows exist; a real reported record must not vanish from the chart just
// because its reported_time column is NULL.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_reports_null_time_falls_back_to_created_time(ctx: &mut TestContext) {
    let product_id = "chart_product_nulltime";
    let device_id = "chart_device_nulltime";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let created = anchor - 5.minutes();

    // The production write path cannot produce a NULL reported_time; insert
    // the legacy-style row directly.
    sqlx::query(&format!(
        r#"INSERT INTO "{}".property_history (product_id, device_id, properties, reported_time, created_time)
           VALUES ($1, $2, $3, NULL, $4::timestamptz)"#,
        ctx.schema_name
    ))
    .bind(product_id)
    .bind(device_id)
    .bind(json!({ "legacy": 42 }))
    .bind(created)
    .execute(&ctx._admin_pool)
    .await
    .unwrap();

    let (status, body) = get_json(
        ctx,
        &series_uri(
            product_id,
            device_id,
            &["legacy"],
            &rfc3339(anchor - 1.hours()),
            &rfc3339(anchor + 1.hours()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let series = &body["data"].as_array().unwrap()[0];
    assert_eq!(
        series["totalPoints"], 1,
        "NULL reported_time row must still be charted"
    );
    let points = series["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(
        point_time(&points[0]),
        created,
        "axis time must fall back to created_time"
    );
    assert_eq!(points[0]["value"].as_i64(), Some(42));
}

// ---------------------------------------------------------------------------
// Scenario 7: invalid series/keys requests are rejected with 400.
//
// User Story: US-PA-052 (查询参数校验), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: §4.2 error matrix — identifier, time format, start >= end, keys
// cardinality/emptiness/length, lookback_days bounds.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_series_rejects_invalid_requests(ctx: &mut TestContext) {
    let product_id = "chart_product_invalid";
    let device_id = "chart_device_invalid";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let (start, end) = (rfc3339(anchor - 1.hours()), rfc3339(anchor + 1.hours()));

    // Invalid identifiers are percent-encoded so the raw URI stays parseable;
    // axum decodes them back before `validate_identifier` rejects them.
    let cases: Vec<(&str, String)> = vec![
        (
            "invalid product_id",
            series_uri("bad%21product", device_id, &["k"], &start, &end),
        ),
        (
            "invalid device_id",
            series_uri(product_id, "bad%20device", &["k"], &start, &end),
        ),
        (
            "bad start_time format",
            series_uri(product_id, device_id, &["k"], "not-a-time", &end),
        ),
        (
            "bad end_time format",
            series_uri(
                product_id,
                device_id,
                &["k"],
                &start,
                "2026-13-99T00:00:00Z",
            ),
        ),
        (
            "start == end",
            series_uri(product_id, device_id, &["k"], &start, &start.clone()),
        ),
        (
            "start > end",
            series_uri(product_id, device_id, &["k"], &end, &start),
        ),
        (
            "empty key",
            series_uri(product_id, device_id, &[""], &start, &end),
        ),
        (
            "too many keys",
            series_uri(
                product_id,
                device_id,
                &["a", "b", "c", "d", "e", "f"],
                &start,
                &end,
            ),
        ),
        (
            "overlong key",
            series_uri(
                product_id,
                device_id,
                &["x".repeat(129).as_str()],
                &start,
                &end,
            ),
        ),
    ];
    for (label, uri) in cases {
        let (status, body) = get_json(ctx, &uri).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "case '{label}' must be rejected with 400"
        );
        assert!(
            body.get("error").is_some(),
            "case '{label}' must return the ApiError shape"
        );
    }

    // keys endpoint validation matrix.
    let keys_cases: Vec<(&str, String)> = vec![
        (
            "invalid identifier",
            keys_uri("bad!product", device_id, None),
        ),
        (
            "lookback_days = 0",
            keys_uri(product_id, device_id, Some(0)),
        ),
        (
            "lookback_days = 367",
            keys_uri(product_id, device_id, Some(367)),
        ),
    ];
    for (label, uri) in keys_cases {
        let (status, body) = get_json(ctx, &uri).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "keys case '{label}' must be rejected with 400"
        );
        assert!(
            body.get("error").is_some(),
            "keys case '{label}' must return the ApiError shape"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 8: the existing paginated property history endpoint is unchanged.
//
// User Story: US-PA-015 (查看设备属性历史，已发布基线，本功能不替代), 来源
//             docs/user-stories/01-platform-admin-user-stories.md
// Covers: 加法式双视图 — chart endpoints are purely
// additive; if this feature ever changed CommonQuery, the legacy SQL or the
// response shape, this anchor fails.
// WHY encoded: the table view is the per-record audit baseline; any drift in
// its pagination contract (data + pagination without total, created_time DESC)
// breaks published acceptance criteria.
// ---------------------------------------------------------------------------
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_existing_property_history_endpoint_unchanged(ctx: &mut TestContext) {
    let product_id = "chart_product_legacy";
    let device_id = "chart_device_legacy";
    let anchor = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 1 }),
        anchor - 3.hours(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 2 }),
        anchor - 2.hours(),
    )
    .await;
    seed_report(
        ctx,
        product_id,
        device_id,
        &json!({ "temperature": 3 }),
        anchor - 1.hours(),
    )
    .await;

    let (status, body) = get_json(
        ctx,
        &format!("/api/admin/property/history?product_id={product_id}&device_id={device_id}&page=1&page_size=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let data = body["data"]
        .as_array()
        .expect("legacy response keeps the data array");
    assert_eq!(data.len(), 2, "page_size is honored");
    // SimplePaginatedResponse has no total field.
    assert!(
        body["pagination"].get("total").is_none(),
        "legacy pagination must stay total-less (SimplePaginationInfo)"
    );
    assert_eq!(body["pagination"]["page"], 1);
    // The legacy shape serializes snake_case (`SimplePaginationInfo` /
    // `PropertyHistory` carry no camelCase rename) — pin the REAL wire format.
    assert_eq!(
        body["pagination"]["page_size"], 2,
        "legacy pagination keys keep snake_case"
    );
    // created_time DESC: the most recently inserted report comes first.
    assert_eq!(data[0]["properties"]["temperature"], 3);
    assert_eq!(data[1]["properties"]["temperature"], 2);
    for item in data {
        for field in [
            "id",
            "product_id",
            "device_id",
            "properties",
            "reported_time",
            "created_time",
        ] {
            assert!(
                item.get(field).is_some(),
                "legacy item must keep field {field}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Pure-function unit tests (no router / DB needed).
// ---------------------------------------------------------------------------

#[test]
fn stride_is_one_up_to_cap_and_ceil_beyond() {
    // ceil(total/1000) with a floor of 1: boundary at exactly the cap stays
    // lossless; one record over doubles nothing but flips the stride to 2.
    assert_eq!(series_stride(0), 1);
    assert_eq!(series_stride(1), 1);
    assert_eq!(series_stride(999), 1);
    assert_eq!(series_stride(1000), 1);
    assert_eq!(series_stride(1001), 2);
    assert_eq!(series_stride(2500), 3);
    assert_eq!(series_stride(1_000_000), 1000);
}

#[test]
fn normalize_series_keys_dedups_and_enforces_bounds() {
    let s = |v: &[&str]| v.iter().map(|k| k.to_string()).collect::<Vec<String>>();

    assert_eq!(
        normalize_series_keys(&s(&["a", "a", "b"])).unwrap(),
        s(&["a", "b"])
    );
    assert_eq!(
        normalize_series_keys(&s(&["a", "b", "c", "d", "e"]))
            .unwrap()
            .len(),
        5,
        "exactly 5 distinct keys are allowed"
    );
    assert!(
        normalize_series_keys(&s(&[])).is_err(),
        "no keys is invalid"
    );
    assert!(
        normalize_series_keys(&s(&[""])).is_err(),
        "empty key string is invalid"
    );
    assert!(
        normalize_series_keys(&s(&["a", "b", "c", "d", "e", "f"])).is_err(),
        "6 keys are invalid"
    );
    assert!(
        normalize_series_keys(&s(&["x".repeat(129).as_str()])).is_err(),
        "overlong key is invalid"
    );
    assert!(
        normalize_series_keys(&s(&["x".repeat(128).as_str()])).is_ok(),
        "128-char key is the allowed maximum"
    );
}

#[test]
fn parse_series_range_rejects_bad_formats_and_inverted_bounds() {
    let valid = "2026-08-01T00:00:00Z";
    assert!(parse_series_range("not-a-time", valid).is_err());
    assert!(parse_series_range(valid, "not-a-time").is_err());
    assert!(
        parse_series_range(valid, valid).is_err(),
        "start == end is rejected"
    );
    assert!(
        parse_series_range("2026-08-02T00:00:00Z", valid).is_err(),
        "start > end is rejected"
    );
    let (start, end) = parse_series_range(valid, "2026-08-01T01:00:00Z").unwrap();
    assert_eq!(
        start,
        OffsetDateTime::parse(valid, &time::format_description::well_known::Rfc3339).unwrap()
    );
    assert_eq!(
        end,
        OffsetDateTime::parse(
            "2026-08-01T01:00:00Z",
            &time::format_description::well_known::Rfc3339
        )
        .unwrap()
    );
}
