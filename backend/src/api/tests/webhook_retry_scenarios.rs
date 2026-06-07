//! Scenario tests for the webhook retry state machine's 5 transition paths,
//! plus 4 integration tests exercising the full retry loop flow.
//!
//! Covers (design doc sections 4.7.1, 4.7.2, 4.7.3, 5.1):
//! - NULL -> 0 on successful webhook delivery
//! - NULL -> 1 (pending retry) on webhook failure
//! - 1 -> 0 on retry success (mark_webhook_success)
//! - 1 -> 1 with decremented retries_left (decrement_retry_and_schedule_next)
//! - Exhausted retries (retries_left=0) excluded from pending query
//!
//! Integration tests (retry loop simulation):
//! - retry loop succeeds on second attempt (query -> send -> mark_success)
//! - retry loop decrements on failure (query -> send fails -> decrement)
//! - exhausted alarm not returned by query_pending_retries
//! - future-scheduled alarm not returned by query_pending_retries

use super::simple_tests::TestContext;
use crate::rule_engine::TriggerContext;
use crate::rule_engine::actions::{ActionExecutor, send_webhook};
use crate::rule_engine::evaluator::TriggerType;
use serde_json::json;
use std::time::Duration as StdDuration;
use test_context::test_context;
use time::{Duration as TimeDuration, OffsetDateTime};

// ---------------------------------------------------------------------------
// 1. NULL -> 0: webhook succeeds on first attempt
// ---------------------------------------------------------------------------

/// User story: Webhook retry state machine (design 4.7.1, path 1)
/// Covers: Initial webhook call returns HTTP 200. The alarm record's webhook_status
///         must be set to 0 (terminal success state) with no retry fields set.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_webhook_success_writes_status_zero(ctx: &mut TestContext) {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/webhook")
        .with_status(200)
        .create_async()
        .await;

    let repo = ctx._admin_state.db.alarm();
    let webhook_url = server.url();

    let actions = json!([
        { "type": "alarm", "level": "warning", "message": "Test alarm" },
        { "type": "webhook", "url": format!("{webhook_url}/webhook") }
    ])
    .as_array()
    .unwrap()
    .clone();

    let trigger_ctx = TriggerContext {
        product_id: "wr_test_prod".to_string(),
        device_id: "wr_test_dev".to_string(),
        trigger_type: TriggerType::Property,
        trigger_value: json!({"temperature": 90}),
    };

    ActionExecutor::execute_actions(&actions, &trigger_ctx, &repo, 1, "test rule")
        .await
        .expect("execute_actions must succeed");

    mock.assert_async().await;

    // Verify: webhook_status = 0 (success terminal state)
    let alarms = repo
        .query_alarms(
            Some("wr_test_prod"),
            Some("wr_test_dev"),
            None,
            None,
            None,
            1,
            10,
        )
        .await
        .expect("query_alarms must succeed")
        .0;
    assert_eq!(alarms.len(), 1, "Expected exactly one alarm record");
    let alarm = &alarms[0];
    assert_eq!(
        alarm.webhook_status,
        Some(0),
        "webhook_status must be 0 after successful delivery"
    );
}

// ---------------------------------------------------------------------------
// 2. NULL -> 1: webhook fails, retry state is written
// ---------------------------------------------------------------------------

/// User story: Webhook retry state machine (design 4.7.1, path 2)
/// Covers: Initial webhook call returns HTTP 500. The alarm record's webhook_status
///         must be 1 (pending retry), retries_left must equal webhook_max_retries (3),
///         and webhook_next_retry_at must be set to a future timestamp.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_webhook_failure_writes_retry_state(ctx: &mut TestContext) {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/webhook")
        .with_status(500)
        .create_async()
        .await;

    let repo = ctx._admin_state.db.alarm();
    let webhook_url = server.url();

    let actions = json!([
        { "type": "alarm", "level": "critical", "message": "Test alarm fail" },
        { "type": "webhook", "url": format!("{webhook_url}/webhook") }
    ])
    .as_array()
    .unwrap()
    .clone();

    let trigger_ctx = TriggerContext {
        product_id: "wr_fail_prod".to_string(),
        device_id: "wr_fail_dev".to_string(),
        trigger_type: TriggerType::Property,
        trigger_value: json!({"temperature": 99}),
    };

    ActionExecutor::execute_actions(&actions, &trigger_ctx, &repo, 2, "fail rule")
        .await
        .expect("execute_actions must succeed");

    mock.assert_async().await;

    // Verify: webhook_status = 1, retries_left = 3, next_retry_at is set and in the future
    let alarms = repo
        .query_alarms(
            Some("wr_fail_prod"),
            Some("wr_fail_dev"),
            None,
            None,
            None,
            1,
            10,
        )
        .await
        .expect("query_alarms must succeed")
        .0;
    assert_eq!(alarms.len(), 1, "Expected exactly one alarm record");
    let alarm = &alarms[0];
    assert_eq!(
        alarm.webhook_status,
        Some(1),
        "webhook_status must be 1 (pending retry) after failure"
    );
    assert_eq!(
        alarm.webhook_retries_left, 3,
        "retries_left must equal max_retries (3)"
    );
    assert!(
        alarm.webhook_next_retry_at.is_some(),
        "next_retry_at must be set after failure"
    );
}

// ---------------------------------------------------------------------------
// 3. 1 -> 0: retry succeeds, mark_webhook_success
// ---------------------------------------------------------------------------

/// User story: Webhook retry state machine (design 4.7.1, path 3)
/// Covers: An alarm with webhook_status=1 and retries_left=2 has its retry succeed.
///         Calling mark_webhook_success must set webhook_status=0, retries_left=0,
///         and webhook_next_retry_at=NULL (terminal success state).
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_webhook_retry_success_marks_zero(ctx: &mut TestContext) {
    let repo = ctx._admin_state.db.alarm();

    // Insert alarm and set it to retry-pending state
    let alarm_id = repo
        .insert_alarm(
            3,
            "retry success rule",
            "wr_rs_prod",
            "wr_rs_dev",
            1,
            Some("Retry success alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    repo.update_alarm_webhook_status_with_retry(
        alarm_id,
        1,
        2,
        Some(time::OffsetDateTime::now_utc()),
    )
    .await
    .expect("set retry state must succeed");

    // Simulate successful retry
    repo.mark_webhook_success(alarm_id)
        .await
        .expect("mark_webhook_success must succeed");

    // Verify terminal success state
    let alarm = repo
        .get_alarm_by_id(alarm_id)
        .await
        .expect("get_alarm_by_id must succeed")
        .expect("alarm must exist");
    assert_eq!(
        alarm.webhook_status,
        Some(0),
        "webhook_status must be 0 (success) after mark_webhook_success"
    );
    assert_eq!(
        alarm.webhook_retries_left, 0,
        "retries_left must be 0 after mark_webhook_success"
    );
    assert!(
        alarm.webhook_next_retry_at.is_none(),
        "next_retry_at must be NULL after mark_webhook_success"
    );
}

// ---------------------------------------------------------------------------
// 4. 1 -> 1: retry fails, retries_left decremented
// ---------------------------------------------------------------------------

/// User story: Webhook retry state machine (design 4.7.1, path 4)
/// Covers: An alarm with webhook_status=1 and retries_left=2 has its retry fail.
///         Calling decrement_retry_and_schedule_next must decrement retries_left to 1
///         and update webhook_next_retry_at to a future timestamp.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_webhook_retry_failure_decrements(ctx: &mut TestContext) {
    let repo = ctx._admin_state.db.alarm();

    // Insert alarm in retry-pending state with retries_left=2
    let alarm_id = repo
        .insert_alarm(
            4,
            "retry decrement rule",
            "wr_dec_prod",
            "wr_dec_dev",
            1,
            Some("Retry decrement alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    let original_next_retry = time::OffsetDateTime::now_utc();
    repo.update_alarm_webhook_status_with_retry(alarm_id, 1, 2, Some(original_next_retry))
        .await
        .expect("set retry state must succeed");

    // Simulate failed retry -- decrement and reschedule
    repo.decrement_retry_and_schedule_next(alarm_id)
        .await
        .expect("decrement_retry_and_schedule_next must succeed");

    // Verify: retries_left decremented, next_retry_at updated to a future time
    let alarm = repo
        .get_alarm_by_id(alarm_id)
        .await
        .expect("get_alarm_by_id must succeed")
        .expect("alarm must exist");
    assert_eq!(
        alarm.webhook_status,
        Some(1),
        "webhook_status must remain 1 (still pending retry)"
    );
    assert_eq!(
        alarm.webhook_retries_left, 1,
        "retries_left must be decremented from 2 to 1"
    );
    let next_retry = alarm
        .webhook_next_retry_at
        .expect("next_retry_at must be set after decrement");
    assert!(
        next_retry > original_next_retry,
        "next_retry_at must be updated to a later time after decrement"
    );
}

// ---------------------------------------------------------------------------
// 5. Exhausted: retries_left=0, not in pending query
// ---------------------------------------------------------------------------

/// User story: Webhook retry state machine (design 4.7.1, path 5)
/// Covers: An alarm with webhook_status=1 and retries_left=0 represents an exhausted
///         retry state (terminal failure). query_pending_retries must NOT return it,
///         confirming the background retry task will not attempt further retries.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_webhook_retries_exhausted_no_more_retries(ctx: &mut TestContext) {
    let repo = ctx._admin_state.db.alarm();

    // Insert alarm in exhausted retry state: status=1, retries_left=0
    let exhausted_id = repo
        .insert_alarm(
            5,
            "exhausted rule",
            "wr_exh_prod",
            "wr_exh_dev",
            2,
            Some("Exhausted retries alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    // Set to terminal failure state: status=1, retries_left=0, next_retry_at in the past
    let past_time = time::OffsetDateTime::now_utc() - time::Duration::minutes(5);
    repo.update_alarm_webhook_status_with_retry(exhausted_id, 1, 0, Some(past_time))
        .await
        .expect("set exhausted state must succeed");

    // Also insert a pending alarm (status=1, retries_left>0) to verify it IS returned
    let pending_id = repo
        .insert_alarm(
            5,
            "pending rule",
            "wr_exh_prod",
            "wr_exh_dev2",
            1,
            Some("Pending retry alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    repo.update_alarm_webhook_status_with_retry(pending_id, 1, 1, Some(past_time))
        .await
        .expect("set pending state must succeed");

    // Query pending retries
    let pending = repo
        .query_pending_retries()
        .await
        .expect("query_pending_retries must succeed");

    let pending_ids: Vec<i64> = pending.iter().map(|a| a.id).collect();

    // Exhausted alarm must NOT appear
    assert!(
        !pending_ids.contains(&exhausted_id),
        "Exhausted alarm (retries_left=0) must NOT appear in pending query"
    );

    // Pending alarm must appear (sanity check that query works)
    assert!(
        pending_ids.contains(&pending_id),
        "Pending alarm (retries_left>0) must appear in pending query"
    );
}

// ===========================================================================
// Integration tests: retry loop simulation (design 4.7.3, 5.1)
// ===========================================================================
//
// These tests simulate a single retry loop iteration by calling the same
// sequence the background task uses: query_pending_retries -> send_webhook
// -> mark_webhook_success / decrement_retry_and_schedule_next.
// They do NOT spawn the actual background task.

// ---------------------------------------------------------------------------
// 6. Retry loop: succeeds on second attempt
// ---------------------------------------------------------------------------

/// User story: Webhook retry background task iteration (design 4.7.3, 5.1)
/// Covers: An alarm whose initial webhook delivery failed (status=1, retries_left=3)
///         is found by query_pending_retries. The retry webhook call returns HTTP 200.
///         After mark_webhook_success, the alarm reaches the terminal success state
///         (status=0, retries_left=0, next_retry_at=NULL).
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_retry_loop_succeeds_on_second_attempt(ctx: &mut TestContext) {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/retry-webhook")
        .with_status(200)
        .create_async()
        .await;

    let repo = ctx._admin_state.db.alarm();
    let webhook_url = format!("{}/retry-webhook", server.url());

    // Create a rule with a webhook action so retry_single_webhook can find the URL
    let rule_id = repo
        .create_rule(&crate::api::alarm_models::CreateAlarmRuleRequest {
            product_id: "rl_succeed_prod".to_string(),
            name: "retry loop succeed rule".to_string(),
            description: None,
            trigger_type: "property".to_string(),
            trigger_config: json!({}),
            condition: json!({"property": "temperature", "operator": ">", "value": 80}),
            actions: json!([
                { "type": "alarm", "level": "warning", "message": "Retry loop test" },
                { "type": "webhook", "url": webhook_url }
            ]),
            throttle_minutes: 0,
            duration_minutes: 0,
            clear_condition: None,
        })
        .await
        .expect("create_rule must succeed");

    // Insert alarm in retry-pending state: status=1, retries_left=3, next_retry_at=NOW()
    let alarm_id = repo
        .insert_alarm(
            rule_id,
            "retry loop succeed rule",
            "rl_succeed_prod",
            "rl_succeed_dev",
            1,
            Some("Retry loop succeed alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    repo.update_alarm_webhook_status_with_retry(alarm_id, 1, 3, Some(OffsetDateTime::now_utc()))
        .await
        .expect("set retry state must succeed");

    // Step 1: query_pending_retries -- alarm must be found
    let pending = repo
        .query_pending_retries()
        .await
        .expect("query_pending_retries must succeed");
    let found = pending.iter().find(|a| a.id == alarm_id);
    assert!(
        found.is_some(),
        "Alarm must appear in query_pending_retries"
    );
    let alarm = found.unwrap();

    // Step 2: get rule actions and send webhook (simulating retry_single_webhook)
    let actions_json = repo
        .get_rule_actions(alarm.rule_id)
        .await
        .expect("get_rule_actions must succeed")
        .expect("rule actions must exist");
    let parsed =
        crate::rule_engine::actions::parse_actions(actions_json.as_array().unwrap_or(&vec![]));
    let webhook_url = parsed
        .iter()
        .find_map(|a| match a {
            crate::rule_engine::actions::AlarmAction::Webhook { url } => Some(url.clone()),
            _ => None,
        })
        .expect("rule must have a webhook action");

    let payload = json!({
        "rule_name": alarm.rule_name,
        "product_id": alarm.product_id,
        "device_id": alarm.device_id,
        "trigger_type": alarm.trigger_type,
        "trigger_value": alarm.trigger_value,
    });

    // Webhook succeeds (mock returns 200)
    send_webhook(&webhook_url, &payload, StdDuration::from_secs(5))
        .await
        .expect("send_webhook must succeed");

    // Step 3: mark_webhook_success
    repo.mark_webhook_success(alarm_id)
        .await
        .expect("mark_webhook_success must succeed");

    mock.assert_async().await;

    // Verify terminal success state
    let alarm = repo
        .get_alarm_by_id(alarm_id)
        .await
        .expect("get_alarm_by_id must succeed")
        .expect("alarm must exist");
    assert_eq!(
        alarm.webhook_status,
        Some(0),
        "webhook_status must be 0 after successful retry"
    );
    assert_eq!(
        alarm.webhook_retries_left, 0,
        "retries_left must be 0 after mark_webhook_success"
    );
    assert!(
        alarm.webhook_next_retry_at.is_none(),
        "next_retry_at must be NULL after mark_webhook_success"
    );
}

// ---------------------------------------------------------------------------
// 7. Retry loop: decrements on failure
// ---------------------------------------------------------------------------

/// User story: Webhook retry background task iteration (design 4.7.3, 5.1)
/// Covers: An alarm in retry state (status=1, retries_left=3) has its retry
///         webhook call fail (HTTP 500). decrement_retry_and_schedule_next
///         must reduce retries_left to 2 and set next_retry_at to a future time.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_retry_loop_decrements_on_failure(ctx: &mut TestContext) {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/retry-fail-webhook")
        .with_status(500)
        .create_async()
        .await;

    let repo = ctx._admin_state.db.alarm();
    let webhook_url = format!("{}/retry-fail-webhook", server.url());

    let rule_id = repo
        .create_rule(&crate::api::alarm_models::CreateAlarmRuleRequest {
            product_id: "rl_dec_prod".to_string(),
            name: "retry loop decrement rule".to_string(),
            description: None,
            trigger_type: "property".to_string(),
            trigger_config: json!({}),
            condition: json!({"property": "temperature", "operator": ">", "value": 90}),
            actions: json!([
                { "type": "alarm", "level": "critical", "message": "Retry loop decrement" },
                { "type": "webhook", "url": webhook_url }
            ]),
            throttle_minutes: 0,
            duration_minutes: 0,
            clear_condition: None,
        })
        .await
        .expect("create_rule must succeed");

    let alarm_id = repo
        .insert_alarm(
            rule_id,
            "retry loop decrement rule",
            "rl_dec_prod",
            "rl_dec_dev",
            2,
            Some("Retry loop decrement alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    let before_retry = OffsetDateTime::now_utc();
    repo.update_alarm_webhook_status_with_retry(alarm_id, 1, 3, Some(OffsetDateTime::now_utc()))
        .await
        .expect("set retry state must succeed");

    // query_pending_retries -- alarm must be found
    let pending = repo
        .query_pending_retries()
        .await
        .expect("query_pending_retries must succeed");
    let found = pending.iter().find(|a| a.id == alarm_id);
    assert!(
        found.is_some(),
        "Alarm must appear in query_pending_retries"
    );
    let alarm = found.unwrap();

    // Get webhook URL from rule actions and send (will fail with 500)
    let actions_json = repo
        .get_rule_actions(alarm.rule_id)
        .await
        .expect("get_rule_actions must succeed")
        .expect("rule actions must exist");
    let parsed =
        crate::rule_engine::actions::parse_actions(actions_json.as_array().unwrap_or(&vec![]));
    let webhook_url = parsed
        .iter()
        .find_map(|a| match a {
            crate::rule_engine::actions::AlarmAction::Webhook { url } => Some(url.clone()),
            _ => None,
        })
        .expect("rule must have a webhook action");

    let payload = json!({
        "rule_name": alarm.rule_name,
        "product_id": alarm.product_id,
        "device_id": alarm.device_id,
        "trigger_type": alarm.trigger_type,
        "trigger_value": alarm.trigger_value,
    });

    // Webhook fails
    let result = send_webhook(&webhook_url, &payload, StdDuration::from_secs(5)).await;
    assert!(
        result.is_err(),
        "send_webhook must fail when server returns 500"
    );

    mock.assert_async().await;

    // decrement_retry_and_schedule_next
    repo.decrement_retry_and_schedule_next(alarm_id)
        .await
        .expect("decrement_retry_and_schedule_next must succeed");

    // Verify state: retries_left decremented, next_retry_at in the future
    let alarm = repo
        .get_alarm_by_id(alarm_id)
        .await
        .expect("get_alarm_by_id must succeed")
        .expect("alarm must exist");
    assert_eq!(
        alarm.webhook_status,
        Some(1),
        "webhook_status must remain 1 (still pending retry)"
    );
    assert_eq!(
        alarm.webhook_retries_left, 2,
        "retries_left must be decremented from 3 to 2"
    );
    let next_retry = alarm
        .webhook_next_retry_at
        .expect("next_retry_at must be set after decrement");
    assert!(
        next_retry > before_retry,
        "next_retry_at must be in the future after decrement"
    );
}

// ---------------------------------------------------------------------------
// 8. Retry loop: exhausted alarm not in pending query
// ---------------------------------------------------------------------------

/// User story: Webhook retry background task iteration (design 4.7.3, 5.1)
/// Covers: An alarm with webhook_status=1 and retries_left=0 is in a terminal
///         failure state. query_pending_retries must NOT return it, ensuring
///         the background task stops retrying after exhausting all attempts.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_retry_loop_exhausted_stops_retrying(ctx: &mut TestContext) {
    let repo = ctx._admin_state.db.alarm();

    // Insert alarm in exhausted state: status=1, retries_left=0, next_retry_at=NOW()
    let alarm_id = repo
        .insert_alarm(
            10,
            "exhausted retry loop rule",
            "rl_exh_prod",
            "rl_exh_dev",
            1,
            Some("Exhausted retry loop alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    repo.update_alarm_webhook_status_with_retry(alarm_id, 1, 0, Some(OffsetDateTime::now_utc()))
        .await
        .expect("set exhausted state must succeed");

    // query_pending_retries must NOT include this alarm
    let pending = repo
        .query_pending_retries()
        .await
        .expect("query_pending_retries must succeed");

    let found = pending.iter().any(|a| a.id == alarm_id);
    assert!(
        !found,
        "Exhausted alarm (retries_left=0) must NOT appear in query_pending_retries, \
         ensuring the retry loop will not attempt further retries"
    );
}

// ---------------------------------------------------------------------------
// 9. Retry loop: skips future-scheduled retry
// ---------------------------------------------------------------------------

/// User story: Webhook retry background task iteration (design 4.7.3, 5.1)
/// Covers: An alarm with next_retry_at in the future (1 hour from now) must NOT
///         be returned by query_pending_retries. The background task must only
///         process alarms whose scheduled retry time has arrived.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_retry_loop_skips_future_retry(ctx: &mut TestContext) {
    let repo = ctx._admin_state.db.alarm();

    // Insert alarm with next_retry_at 1 hour in the future
    let alarm_id = repo
        .insert_alarm(
            11,
            "future retry rule",
            "rl_future_prod",
            "rl_future_dev",
            1,
            Some("Future retry alarm"),
            None,
            "property",
        )
        .await
        .expect("insert_alarm must succeed");

    let future_retry_at = OffsetDateTime::now_utc() + TimeDuration::hours(1);
    repo.update_alarm_webhook_status_with_retry(alarm_id, 1, 2, Some(future_retry_at))
        .await
        .expect("set future retry state must succeed");

    // query_pending_retries must NOT include this alarm
    let pending = repo
        .query_pending_retries()
        .await
        .expect("query_pending_retries must succeed");

    let found = pending.iter().any(|a| a.id == alarm_id);
    assert!(
        !found,
        "Alarm with next_retry_at in the future must NOT appear in query_pending_retries, \
         ensuring the retry loop only processes due alarms"
    );
}
