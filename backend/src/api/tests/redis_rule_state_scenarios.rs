//! Integration tests for RedisRuleStateStore (dedup, duration, graceful degradation).
//!
//! User Story: As a backend developer, I need to verify that RedisRuleStateStore
//! correctly implements the RuleStateStore trait with behavioral parity to
//! InMemoryRuleStateStore, and gracefully degrades when Redis is unavailable,
//! so that alarm rule evaluation remains correct in both healthy and degraded states.
//!
//! Covers:
//! - Dedup: no previous trigger, within window, after window, zero/negative throttle, key namespace isolation
//! - Duration: first check (JustStarted), not yet met, met after window, reset clears tracking,
//!   zero minutes returns Met, full lifecycle cycle
//! - Graceful degradation: check_dedup returns true on error, check_duration returns NotStarted on error,
//!   mark_triggered returns Err on error

use crate::rule_engine::cache::{DurationCheckResult, RedisRuleStateStore, RuleStateStore};
use redis::AsyncCommands;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Redis URL for integration tests. Defaults to local Redis.
fn redis_url() -> String {
    std::env::var("TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string())
}

/// Create a RedisRuleStateStore connected to the test Redis instance.
fn make_redis_store() -> Arc<dyn RuleStateStore> {
    let store =
        RedisRuleStateStore::new(&redis_url()).expect("Failed to create RedisRuleStateStore");
    Arc::new(store)
}

/// Create a RedisRuleStateStore with an unreachable URL to simulate Redis being down.
fn make_failing_redis_store() -> Arc<dyn RuleStateStore> {
    let store = RedisRuleStateStore::new("redis://255.255.255.255:1/")
        .expect("Failed to create failing RedisRuleStateStore");
    Arc::new(store)
}

/// Get a direct Redis connection for key cleanup.
async fn redis_connection() -> redis::aio::MultiplexedConnection {
    let client = redis::Client::open(redis_url()).expect("Failed to open Redis client for cleanup");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("Failed to get Redis connection for cleanup")
}

/// Clean up dedup and duration keys for the given (rule_id, device_id) pair.
async fn cleanup_keys(rule_id: i64, device_id: &str) {
    let mut conn = redis_connection().await;
    let dedup_key = format!("alarm:dedup:{}:{}", rule_id, device_id);
    let duration_key = format!("alarm:duration:{}:{}", rule_id, device_id);
    let _: () = conn.del(&dedup_key).await.unwrap_or(());
    let _: () = conn.del(&duration_key).await.unwrap_or(());
}

/// Helper to verify a Redis key does not exist (for post-cleanup assertions).
async fn assert_key_absent(key: &str) {
    let mut conn = redis_connection().await;
    let exists: bool = conn.exists(key).await.unwrap_or(false);
    assert!(!exists, "Key {} should have been cleaned up", key);
}

// ---------------------------------------------------------------------------
// Dedup tests
// ---------------------------------------------------------------------------

// Covers: check_dedup returns false when no prior trigger exists in Redis,
// meaning the alarm is not suppressed.
#[tokio::test]
async fn scenario_redis_dedup_no_previous_trigger_returns_false() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_id: i64 = 90_001;
    let device_id = "device_test_001";

    let is_dup = store.check_dedup(rule_id, device_id, 10, now).await;
    assert!(
        !is_dup,
        "Expected false: no previous trigger means not in dedup window"
    );

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_dedup returns true when checked within the throttle window
// after mark_triggered, suppressing duplicate alarms.
#[tokio::test]
async fn scenario_redis_dedup_within_window_returns_true() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 90_002;
    let device_id = "device_test_002";

    store.mark_triggered(rule_id, device_id, 10).await.unwrap();

    // Check 5 minutes later, still within the 10-minute window.
    let is_dup = store
        .check_dedup(rule_id, device_id, 10, t0 + Duration::minutes(5))
        .await;
    assert!(is_dup, "Expected true: still within dedup window");

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_dedup returns false when checked after the throttle window
// expires, allowing a new alarm trigger.
#[tokio::test]
async fn scenario_redis_dedup_after_window_returns_false() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 90_003;
    let device_id = "device_test_003";

    store.mark_triggered(rule_id, device_id, 10).await.unwrap();

    // Check 15 minutes later, past the 10-minute window.
    let is_dup = store
        .check_dedup(rule_id, device_id, 10, t0 + Duration::minutes(15))
        .await;
    assert!(!is_dup, "Expected false: dedup window has expired");

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_dedup with throttle_minutes=0 always returns false,
// meaning no dedup suppression regardless of prior triggers.
#[tokio::test]
async fn scenario_redis_dedup_zero_throttle_always_passes() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_id: i64 = 90_004;
    let device_id = "device_test_004";

    store.mark_triggered(rule_id, device_id, 10).await.unwrap();

    let is_dup = store.check_dedup(rule_id, device_id, 0, now).await;
    assert!(!is_dup, "Expected false: zero throttle means no dedup");

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_dedup with negative throttle_minutes always returns false,
// treated the same as zero (no dedup suppression).
#[tokio::test]
async fn scenario_redis_dedup_negative_throttle_always_passes() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_id: i64 = 90_005;
    let device_id = "device_test_005";

    store.mark_triggered(rule_id, device_id, 10).await.unwrap();

    let is_dup = store.check_dedup(rule_id, device_id, -5, now).await;
    assert!(!is_dup, "Expected false: negative throttle means no dedup");

    cleanup_keys(rule_id, device_id).await;
}

// Covers: Different (rule_id, device_id) pairs use isolated Redis keys,
// so dedup state for one pair does not affect another.
#[tokio::test]
async fn scenario_redis_dedup_key_namespace_isolation() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_a: i64 = 90_006;
    let rule_b: i64 = 90_007;
    let device_a = "device_isolation_a";
    let device_b = "device_isolation_b";

    // Mark rule_a/device_a as triggered.
    store.mark_triggered(rule_a, device_a, 10).await.unwrap();

    // rule_b/device_b should NOT be in dedup window.
    let is_dup_b = store.check_dedup(rule_b, device_b, 10, now).await;
    assert!(
        !is_dup_b,
        "Expected false: different (rule, device) pair is isolated"
    );

    // rule_a/device_a SHOULD be in dedup window.
    let is_dup_a = store.check_dedup(rule_a, device_a, 10, now).await;
    assert!(
        is_dup_a,
        "Expected true: original pair should be in dedup window"
    );

    cleanup_keys(rule_a, device_a).await;
    cleanup_keys(rule_b, device_b).await;
}

// ---------------------------------------------------------------------------
// Duration tests
// ---------------------------------------------------------------------------

// Covers: First check_duration call for a (rule, device) pair returns
// JustStarted, and the Redis key is created.
#[tokio::test]
async fn scenario_redis_duration_first_check_returns_just_started() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_001;
    let device_id = "device_dur_001";

    let result = store.check_duration(rule_id, device_id, 5, now).await;
    assert_eq!(result, DurationCheckResult::JustStarted);

    // Verify the Redis key was created.
    let key = format!("alarm:duration:{}:{}", rule_id, device_id);
    let mut conn = redis_connection().await;
    let exists: bool = conn.exists(&key).await.unwrap();
    assert!(exists, "Duration key should exist after JustStarted");

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_duration returns NotYetMet when the elapsed time is less than
// the required duration window.
#[tokio::test]
async fn scenario_redis_duration_not_yet_met() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_002;
    let device_id = "device_dur_002";

    // Start tracking at t0.
    store.check_duration(rule_id, device_id, 5, t0).await;

    // Check 3 minutes later (less than the 5-minute duration).
    let result = store
        .check_duration(rule_id, device_id, 5, t0 + Duration::minutes(3))
        .await;
    assert_eq!(result, DurationCheckResult::NotYetMet);

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_duration returns Met when the elapsed time equals or exceeds
// the required duration window.
#[tokio::test]
async fn scenario_redis_duration_met_after_window() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_003;
    let device_id = "device_dur_003";

    // Start tracking at t0.
    store.check_duration(rule_id, device_id, 5, t0).await;

    // Check 6 minutes later (exceeds the 5-minute duration).
    let result = store
        .check_duration(rule_id, device_id, 5, t0 + Duration::minutes(6))
        .await;
    assert_eq!(result, DurationCheckResult::Met);

    cleanup_keys(rule_id, device_id).await;
}

// Covers: reset_duration clears the Redis key so a subsequent check_duration
// returns JustStarted again, allowing the duration timer to restart.
#[tokio::test]
async fn scenario_redis_duration_reset_clears_key() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_004;
    let device_id = "device_dur_004";

    // Start tracking.
    store.check_duration(rule_id, device_id, 5, t0).await;

    // Reset the tracking state.
    store.reset_duration(rule_id, device_id).await.unwrap();

    // Verify the key is gone.
    let key = format!("alarm:duration:{}:{}", rule_id, device_id);
    assert_key_absent(&key).await;

    // After reset, should behave as if tracking never started.
    let result = store.check_duration(rule_id, device_id, 5, t0).await;
    assert_eq!(result, DurationCheckResult::JustStarted);

    cleanup_keys(rule_id, device_id).await;
}

// Covers: check_duration with duration_minutes=0 immediately returns Met,
// because zero duration means the condition is met instantly.
#[tokio::test]
async fn scenario_redis_duration_zero_minutes_returns_met() {
    let store = make_redis_store();
    let now = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_005;
    let device_id = "device_dur_005";

    let result = store.check_duration(rule_id, device_id, 0, now).await;
    assert_eq!(result, DurationCheckResult::Met);

    // Zero duration should not create a Redis key.
    let key = format!("alarm:duration:{}:{}", rule_id, device_id);
    assert_key_absent(&key).await;
}

// Covers: Full duration lifecycle -- JustStarted -> NotYetMet -> reset
// -> JustStarted (re-entry) -> Met, verifying that tracking state is
// correctly managed across the entire alarm evaluation cycle in Redis.
#[tokio::test]
async fn scenario_redis_duration_full_cycle() {
    let store = make_redis_store();
    let t0 = OffsetDateTime::now_utc();
    let rule_id: i64 = 91_006;
    let device_id = "device_dur_cycle";

    // 1. First condition met -> JustStarted
    let result = store.check_duration(rule_id, device_id, 3, t0).await;
    assert_eq!(result, DurationCheckResult::JustStarted);

    // 2. Not yet met (1 minute in, need 3)
    let result = store
        .check_duration(rule_id, device_id, 3, t0 + Duration::minutes(1))
        .await;
    assert_eq!(result, DurationCheckResult::NotYetMet);

    // 3. Condition breaks -> reset
    store.reset_duration(rule_id, device_id).await.unwrap();

    // 4. Condition re-met at t0+5 -> JustStarted (fresh tracking)
    let result = store
        .check_duration(rule_id, device_id, 3, t0 + Duration::minutes(5))
        .await;
    assert_eq!(result, DurationCheckResult::JustStarted);

    // 5. Duration passes (4 minutes after re-start = t0+9)
    let result = store
        .check_duration(rule_id, device_id, 3, t0 + Duration::minutes(9))
        .await;
    assert_eq!(result, DurationCheckResult::Met);

    cleanup_keys(rule_id, device_id).await;
}

// ---------------------------------------------------------------------------
// Graceful degradation tests (Redis unavailable)
// ---------------------------------------------------------------------------

// Covers: When Redis is unreachable, check_dedup returns true (allow trigger)
// rather than panicking or blocking. This means alarms may fire more often
// than configured (graceful degradation: tolerate duplicates over lost alarms).
#[tokio::test]
async fn scenario_redis_graceful_check_dedup_returns_true_on_error() {
    let store = make_failing_redis_store();
    let now = OffsetDateTime::now_utc();

    // With a positive throttle, Redis failure should still return true (allow trigger).
    let is_dup = store.check_dedup(99_001, "device_degraded", 10, now).await;
    assert!(
        is_dup,
        "Expected true on Redis error: graceful degradation allows trigger (suppresses nothing)"
    );
}

// Covers: When Redis is unreachable, check_duration returns NotStarted rather
// than panicking. This means alarms requiring sustained conditions will not
// fire spuriously when the state store is unavailable.
#[tokio::test]
async fn scenario_redis_graceful_check_duration_returns_not_started_on_error() {
    let store = make_failing_redis_store();
    let now = OffsetDateTime::now_utc();

    let result = store
        .check_duration(99_002, "device_degraded", 5, now)
        .await;
    assert_eq!(
        result,
        DurationCheckResult::NotStarted,
        "Expected NotStarted on Redis error: graceful degradation does not start tracking"
    );
}

// Covers: When Redis is unreachable, mark_triggered returns Err. The caller
// is expected to ignore this error (dedup state is lost, next evaluation
// may re-trigger, which is acceptable for graceful degradation).
#[tokio::test]
async fn scenario_redis_graceful_mark_triggered_returns_err_on_error() {
    let store = make_failing_redis_store();

    let result = store.mark_triggered(99_003, "device_degraded", 10).await;
    assert!(
        result.is_err(),
        "Expected Err on Redis error: mark_triggered signals failure for caller to handle"
    );
}
