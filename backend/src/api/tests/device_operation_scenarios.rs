//! Scenario tests for the unified device-operation read model.
//!
//! Covers:
//! - `Database::query_property_commands_by_source` additive `source` filter:
//!   the filter must enter BOTH the data query and the count query, and must
//!   apply before pagination.
//! - `Database::query_device_operations`: UNION ALL across `property_command`
//!   (source=0 -> `directPropertyWrite`, source=1 -> `targetSync`) and
//!   `action_invocation` (-> `actionInvocation`), merged into a single timeline
//!   ordered by `updated_time DESC, operation_id DESC`, with the count taken
//!   from the same filtered UNION ALL subquery.
//! - Stable composite operation IDs (`property:{id}` / `action:{id}`) that stay
//!   unique across pages.
//! - `query_property_commands_by_source` with `source=None` returns every
//!   source unchanged (the no-source path stays compatible).
//!
//! Test style mirrors `action_invocation_scenarios.rs`: scenarios run under
//! `#[test_context(TestContext)]` and drive the repository directly through
//! `ctx._admin_state.db` (seed + assertions); one scenario additionally
//! smoke-tests the `GET /api/admin/device/operation` handler wiring through
//! `ctx.service`.

use super::simple_tests::{TestContext, request};
use crate::api::admin_models::{DeviceOperationQuery, DeviceOperationType};
use crate::db::models::{CommandSource, CommandStatus};
use axum::http::{Method, StatusCode};
use serde_json::{Value as JsonValue, json};
use std::collections::HashSet;
use test_context::test_context;

fn operation_query(
    product_id: &str,
    device_id: &str,
    operation_type: Option<DeviceOperationType>,
    status: Option<CommandStatus>,
    page: i64,
    page_size: i64,
) -> DeviceOperationQuery {
    DeviceOperationQuery {
        product_id: product_id.to_string(),
        device_id: Some(device_id.to_string()),
        operation_type,
        status,
        page,
        page_size,
    }
}

// ===========================================================================
// Scenario 1: the `source` filter applies before pagination AND count.
//
// User Story: US-PA-020 (device detail page; the Target/Direct tabs rely on
//             the source filter to split one-shot writes from desired deltas).
// Intent: if the filter only entered the data query but not the count query
//         (or vice versa), `pagination.total` would disagree with the returned
//         rows and the frontend pager would render phantom pages.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_source_filter_applies_before_pagination_and_count(
    ctx: &mut TestContext,
) {
    let product_id = "ops_src_product";
    let device_id = "ops_src_device";

    for brightness in [10, 20, 30] {
        ctx._admin_state
            .db
            .insert_property_command(
                product_id,
                device_id,
                &json!({ "brightness": brightness }),
                CommandSource::OneShot,
            )
            .await
            .unwrap();
    }
    for brightness in [40, 50] {
        ctx._admin_state
            .db
            .insert_property_command(
                product_id,
                device_id,
                &json!({ "brightness": brightness }),
                CommandSource::DesiredDelta,
            )
            .await
            .unwrap();
    }

    // OneShot filter: data rows and total must agree (both filtered).
    let (rows, total) = ctx
        ._admin_state
        .db
        .query_property_commands_by_source(
            product_id,
            Some(device_id),
            None,
            Some(CommandSource::OneShot),
            1,
            10,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "only the OneShot rows must be returned");
    assert!(
        rows.iter().all(|r| r.source == CommandSource::OneShot),
        "source filter must enter the data query"
    );
    assert_eq!(
        total, 3,
        "source filter must also enter the count query (pagination.total)"
    );

    // Complementary DesiredDelta filter.
    let (rows, total) = ctx
        ._admin_state
        .db
        .query_property_commands_by_source(
            product_id,
            Some(device_id),
            None,
            Some(CommandSource::DesiredDelta),
            1,
            10,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "only the DesiredDelta rows must be returned");
    assert!(
        rows.iter().all(|r| r.source == CommandSource::DesiredDelta),
        "source filter must enter the data query"
    );
    assert_eq!(total, 2, "count query must apply the same source filter");

    // page_size smaller than the filtered set: the filter must apply BEFORE
    // pagination, so no unfiltered row may leak into any page and the total
    // must still reflect the filtered set on every page.
    let (page1, total1) = ctx
        ._admin_state
        .db
        .query_property_commands_by_source(
            product_id,
            Some(device_id),
            None,
            Some(CommandSource::OneShot),
            1,
            2,
        )
        .await
        .unwrap();
    let (page2, total2) = ctx
        ._admin_state
        .db
        .query_property_commands_by_source(
            product_id,
            Some(device_id),
            None,
            Some(CommandSource::OneShot),
            2,
            2,
        )
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 1, "third OneShot row lands on page 2");
    assert_eq!(total1, 3);
    assert_eq!(total2, 3, "total is page-independent");
    assert!(
        page1
            .iter()
            .chain(page2.iter())
            .all(|r| r.source == CommandSource::OneShot),
        "pagination must not leak rows the source filter excluded"
    );
}

// ===========================================================================
// Scenario 2: the three operation kinds merge into one updated_time-ordered
// timeline with the correct type/name mapping.
//
// User Story: US-PA-020 (device detail Operations tab shows one timeline for
//             direct writes, target syncs and action invocations).
// Intent: type mapping contract (source=0 -> directPropertyWrite / `Set
//         properties`, source=1 -> targetSync / `Sync target`, action ->
//         actionInvocation / service_type) and ordering contract
//         (`updated_time DESC, operation_id DESC`) — the frontend summary rule
//         depends on both the literal names and the stable order.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_operations_merge_three_types_in_updated_order(ctx: &mut TestContext) {
    let product_id = "ops_merge_product";
    let device_id = "ops_merge_device";

    let p_oneshot = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "power": true }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    let p_delta = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 50 }),
            CommandSource::DesiredDelta,
        )
        .await
        .unwrap();
    let a_reboot = ctx
        ._admin_state
        .db
        .insert_action_invocation(
            product_id,
            device_id,
            "reboot",
            &json!({ "delaySeconds": 5 }),
        )
        .await
        .unwrap();

    // Touch rows in a known order to control updated_time: action first, then
    // the OneShot command; the DesiredDelta row keeps its (oldest) insert time.
    // Each status update is its own statement, so the timestamps strictly
    // increase in call order (same pattern mqtt_device_flow_scenarios relies
    // on for created_time ordering).
    let touched = ctx
        ._admin_state
        .db
        .update_action_invocation_status(
            a_reboot,
            product_id,
            device_id,
            "reboot",
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();
    assert_eq!(touched, 1, "the action status touch must take effect");
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![p_oneshot],
            product_id,
            device_id,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(product_id, device_id, None, None, 1, 10))
        .await
        .unwrap();
    assert_eq!(
        resp.pagination.total, 3,
        "all three kinds merge into one count"
    );
    assert_eq!(resp.data.len(), 3, "single merged timeline");

    // updated_time DESC: last touched (OneShot property) first, then the
    // action, then the untouched DesiredDelta row.
    let first = &resp.data[0];
    assert_eq!(first.operation_id, format!("property:{p_oneshot}"));
    assert_eq!(
        first.operation_type,
        DeviceOperationType::DirectPropertyWrite
    );
    assert_eq!(first.name, "Set properties", "source=0 literal name");
    assert_eq!(first.payload, json!({ "power": true }));
    assert_eq!(first.status, CommandStatus::Sent);

    let second = &resp.data[1];
    assert_eq!(second.operation_id, format!("action:{a_reboot}"));
    assert_eq!(second.operation_type, DeviceOperationType::ActionInvocation);
    assert_eq!(
        second.name, "reboot",
        "action name must be the service_type"
    );
    assert_eq!(second.payload, json!({ "delaySeconds": 5 }));
    assert_eq!(second.status, CommandStatus::Sent);

    let third = &resp.data[2];
    assert_eq!(third.operation_id, format!("property:{p_delta}"));
    assert_eq!(third.operation_type, DeviceOperationType::TargetSync);
    assert_eq!(third.name, "Sync target", "source=1 literal name");
    assert_eq!(third.status, CommandStatus::Pending);

    // Secondary sort key: two rows flipped in ONE statement share the exact
    // same updated_time (CURRENT_TIMESTAMP is constant per statement), so
    // their relative order must come from `operation_id DESC`.
    let p_tie_a = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "tie": "a" }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    let p_tie_b = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "tie": "b" }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![p_tie_a, p_tie_b],
            product_id,
            device_id,
            CommandStatus::Failed,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(product_id, device_id, None, None, 1, 10))
        .await
        .unwrap();
    assert_eq!(resp.pagination.total, 5);
    let tie_a = format!("property:{p_tie_a}");
    let tie_b = format!("property:{p_tie_b}");
    let (tie_first, tie_second) = if tie_a > tie_b {
        (tie_a, tie_b)
    } else {
        (tie_b, tie_a)
    };
    assert_eq!(
        resp.data[0].operation_id, tie_first,
        "equal updated_time must fall back to operation_id DESC"
    );
    assert_eq!(resp.data[1].operation_id, tie_second);

    // Handler wiring smoke: GET /api/admin/device/operation must expose the
    // same read model (serde camelCase: operationId, pagination.total).
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/device/operation?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let http_resp: JsonValue = serde_json::from_str(&body).unwrap();
    assert_eq!(http_resp["pagination"]["total"], 5);
    let http_rows = http_resp["data"].as_array().expect("data array");
    assert_eq!(http_rows.len(), 5);
    assert_eq!(
        http_rows[0]["operationId"],
        JsonValue::String(tie_first),
        "handler must serve the same ordering as the repository"
    );
}

// ===========================================================================
// Scenario 3: operation_type and status filters apply before pagination, both
// in the data query and in the UNION ALL count subquery.
//
// User Story: US-PA-020 (Operations tab type/status dropdowns; wrong totals
//             or leaked rows would mislead the operator about device state).
// Intent: the type filter maps to per-branch predicates (property: source=N;
//         action: 1=0 for property types and vice versa), so a filter bug on
//         either branch shows up as leaked rows or a wrong total here.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_operations_filter_type_and_status_before_pagination(
    ctx: &mut TestContext,
) {
    let product_id = "ops_filter_product";
    let device_id = "ops_filter_device";

    let dp_pending = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "power": true }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    let dp_sent = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "power": false }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .update_property_command_status(
            &vec![dp_sent],
            product_id,
            device_id,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();
    let _ts_pending = ctx
        ._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 80 }),
            CommandSource::DesiredDelta,
        )
        .await
        .unwrap();
    let act_sent = ctx
        ._admin_state
        .db
        .insert_action_invocation(product_id, device_id, "reboot", &json!({}))
        .await
        .unwrap();
    let touched = ctx
        ._admin_state
        .db
        .update_action_invocation_status(
            act_sent,
            product_id,
            device_id,
            "reboot",
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();
    assert_eq!(touched, 1);
    ctx._admin_state
        .db
        .insert_action_invocation(product_id, device_id, "unlock", &json!({}))
        .await
        .unwrap();

    // Type filter only: directPropertyWrite.
    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(
            product_id,
            device_id,
            Some(DeviceOperationType::DirectPropertyWrite),
            None,
            1,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(resp.pagination.total, 2);
    assert_eq!(resp.data.len(), 2);
    assert!(
        resp.data
            .iter()
            .all(|op| op.operation_type == DeviceOperationType::DirectPropertyWrite),
        "type filter must exclude targetSync and actionInvocation rows"
    );

    // Type filter only: actionInvocation.
    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(
            product_id,
            device_id,
            Some(DeviceOperationType::ActionInvocation),
            None,
            1,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(resp.pagination.total, 2);
    assert!(
        resp.data
            .iter()
            .all(|op| op.operation_type == DeviceOperationType::ActionInvocation)
    );

    // Type filter only: targetSync.
    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(
            product_id,
            device_id,
            Some(DeviceOperationType::TargetSync),
            None,
            1,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(resp.pagination.total, 1);
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].name, "Sync target");

    // Status filter only (applies to BOTH union branches).
    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(
            product_id,
            device_id,
            None,
            Some(CommandStatus::Pending),
            1,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.pagination.total, 3,
        "Pending rows: OneShot property + DesiredDelta property + unlock action"
    );
    assert!(
        resp.data
            .iter()
            .all(|op| op.status == CommandStatus::Pending)
    );

    // Combined type + status filter.
    let resp = ctx
        ._admin_state
        .db
        .query_device_operations(&operation_query(
            product_id,
            device_id,
            Some(DeviceOperationType::DirectPropertyWrite),
            Some(CommandStatus::Sent),
            1,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(resp.pagination.total, 1);
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].operation_id, format!("property:{dp_sent}"));

    // Filters apply BEFORE pagination: a small page must not leak excluded
    // rows, and the total must stay filter-scoped on every page.
    let mut filtered_ids: Vec<String> = Vec::new();
    for page in [1, 2] {
        let resp = ctx
            ._admin_state
            .db
            .query_device_operations(&operation_query(
                product_id,
                device_id,
                Some(DeviceOperationType::DirectPropertyWrite),
                None,
                page,
                1,
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.pagination.total, 2,
            "total must stay filter-scoped regardless of page"
        );
        assert_eq!(resp.data.len(), 1);
        filtered_ids.push(resp.data[0].operation_id.clone());
    }
    filtered_ids.sort();
    let mut expected_ids = vec![
        format!("property:{dp_pending}"),
        format!("property:{dp_sent}"),
    ];
    expected_ids.sort();
    assert_eq!(
        filtered_ids, expected_ids,
        "pagination must not leak rows excluded by the type filter"
    );

    // Same for the status filter across both union branches.
    let mut status_ids: HashSet<String> = HashSet::new();
    for page in [1, 2] {
        let resp = ctx
            ._admin_state
            .db
            .query_device_operations(&operation_query(
                product_id,
                device_id,
                None,
                Some(CommandStatus::Sent),
                page,
                1,
            ))
            .await
            .unwrap();
        assert_eq!(resp.pagination.total, 2);
        assert_eq!(resp.data.len(), 1);
        status_ids.insert(resp.data[0].operation_id.clone());
    }
    assert_eq!(
        status_ids,
        HashSet::from([format!("property:{dp_sent}"), format!("action:{act_sent}")]),
        "status filter must hold on both union branches across pages"
    );
}

// ===========================================================================
// Scenario 4: composite operation IDs stay unique and correctly prefixed
// across pages.
//
// User Story: US-PA-020 (the Operations tab routes cancel/delete by the
//             `property:` / `action:` prefix — a duplicate or mis-prefixed ID
//             would cancel the wrong row).
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_operations_keep_composite_ids_unique(ctx: &mut TestContext) {
    let product_id = "ops_ids_product";
    let device_id = "ops_ids_device";

    let mut property_ids: HashSet<i64> = HashSet::new();
    for (command, source) in [
        (json!({ "power": true }), CommandSource::OneShot),
        (json!({ "power": false }), CommandSource::OneShot),
        (json!({ "brightness": 60 }), CommandSource::DesiredDelta),
    ] {
        let id = ctx
            ._admin_state
            .db
            .insert_property_command(product_id, device_id, &command, source)
            .await
            .unwrap();
        property_ids.insert(id);
    }
    let mut action_ids: HashSet<i64> = HashSet::new();
    for service_type in ["reboot", "unlock"] {
        let id = ctx
            ._admin_state
            .db
            .insert_action_invocation(product_id, device_id, service_type, &json!({}))
            .await
            .unwrap();
        action_ids.insert(id);
    }

    // Walk all pages with a small page_size: IDs must stay unique and stable
    // across page boundaries (no row repeated, no row dropped).
    let mut seen: HashSet<String> = HashSet::new();
    let mut prefix_by_id: Vec<(String, DeviceOperationType)> = Vec::new();
    for page in [1, 2, 3] {
        let resp = ctx
            ._admin_state
            .db
            .query_device_operations(&operation_query(product_id, device_id, None, None, page, 2))
            .await
            .unwrap();
        assert_eq!(resp.pagination.total, 5, "total stable on every page");
        for op in resp.data {
            assert!(
                seen.insert(op.operation_id.clone()),
                "operation_id {} appears twice across pages",
                op.operation_id
            );
            prefix_by_id.push((op.operation_id, op.operation_type));
        }
    }
    assert_eq!(seen.len(), 5, "3 property + 2 action rows, all unique");

    for (operation_id, operation_type) in prefix_by_id {
        let (prefix, raw_id) = operation_id
            .split_once(':')
            .unwrap_or_else(|| panic!("operation_id {operation_id} must be composite"));
        let db_id: i64 = raw_id
            .parse()
            .unwrap_or_else(|_| panic!("operation_id {operation_id} suffix must be the DB id"));
        match operation_type {
            DeviceOperationType::DirectPropertyWrite | DeviceOperationType::TargetSync => {
                assert_eq!(prefix, "property", "property rows use the property: prefix");
                assert!(
                    property_ids.contains(&db_id),
                    "property:{db_id} must reference a seeded property_command row"
                );
            }
            DeviceOperationType::ActionInvocation => {
                assert_eq!(prefix, "action", "action rows use the action: prefix");
                assert!(
                    action_ids.contains(&db_id),
                    "action:{db_id} must reference a seeded action_invocation row"
                );
            }
        }
    }
}

// ===========================================================================
// Scenario 5: `query_property_commands_by_source` with `source=None` keeps
// the pre-feature (no-source) behavior — regression guard.
//
// User Story: US-PA-020 (existing command-history callers must not change
//             behavior when the source filter is introduced).
// Intent: `source=None` must be a true no-op — return all sources and a
//         full-range total.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_existing_property_query_without_source_remains_compatible(ctx: &mut TestContext) {
    let product_id = "ops_compat_product";
    let device_id = "ops_compat_device";

    for (command, source) in [
        (json!({ "power": true }), CommandSource::OneShot),
        (json!({ "power": false }), CommandSource::OneShot),
        (json!({ "brightness": 70 }), CommandSource::DesiredDelta),
    ] {
        ctx._admin_state
            .db
            .insert_property_command(product_id, device_id, &command, source)
            .await
            .unwrap();
    }

    let (rows, total) = ctx
        ._admin_state
        .db
        .query_property_commands_by_source(product_id, Some(device_id), None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "no-source query returns every source");
    assert_eq!(total, 3, "no-source count covers the full range");
    let mut sources: Vec<CommandSource> = rows.iter().map(|r| r.source).collect();
    sources.sort_by_key(|s| *s as i16);
    sources.dedup();
    assert_eq!(
        sources,
        vec![CommandSource::OneShot, CommandSource::DesiredDelta],
        "both sources must survive the legacy no-source path"
    );
}
