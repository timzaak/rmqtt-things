//! Scenario tests for MQTT device flow, mirroring demo/e2e/mqtt-device-flow-demo.e2e.ts.
//!
//! Covers:
//! - Property upload via webhook → query via admin API
//! - Event upload via webhook → query via admin API
//! - Property command lifecycle: create → send → reply → status = Success
//! - Full integrated device flow

use super::simple_tests::TestContext;
use super::simple_tests::{
    create_test_database, drop_test_schema, request, request_json, test_s3_endpoint,
};
use crate::api::admin_models::CreatePropertyCommandRequest;
use crate::api::handlers::{AppState, S3Client};
use crate::api::web_models::RMqttPublishMessage;
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{
    Config, MqttConfig, PropertyCommandConfig, PropertyCommandPublishConfig, S3Config,
};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Router;
use axum::http::{Method, StatusCode};
use base64::Engine;
use serde_json::{Value as JsonValue, json};
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::{TempDir, tempdir};
use test_context::AsyncTestContext;
use test_context::test_context;
use tokio::sync::Mutex;

fn encode_payload(value: &serde_json::Value) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

fn property_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/post")
}

fn event_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/test/post")
}

fn service_set_reply_topic(product_id: &str, device_id: &str, service_type: &str) -> String {
    format!("/{product_id}/{device_id}/thing/service/{service_type}/set_reply")
}

fn mqtt_publish_message(
    client_id: &str,
    topic: &str,
    payload: &serde_json::Value,
) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

/// Verifies: device posts properties via webhook → admin API returns them.
///
/// Mirrors demo step: `device.postProperties({ temperature, humidity, power })`
/// → `GET /api/admin/property` returns the posted values.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_post_and_query(ctx: &mut TestContext) {
    let product_id = "scenario_product_prop";
    let device_id = "scenario_device_prop";
    let topic = property_post_topic(product_id, device_id);

    let temperature: f64 = 25.5;
    let humidity: f64 = 60.0;

    let payload = json!({
        "id": "prop-test-001",
        "ack": 0,
        "params": {
            "temperature": temperature,
            "humidity": humidity,
            "power": true
        }
    });

    let msg = mqtt_publish_message(device_id, &topic, &payload);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let props = &resp["data"][0]["properties"];
    assert_eq!(props["temperature"]["value"], temperature);
    assert_eq!(props["humidity"]["value"], humidity);
    assert_eq!(props["power"]["value"], true);
}

/// Verifies: device posts event via webhook → admin API returns it.
///
/// Mirrors demo step: `device.postEvent({ event, marker })`
/// → `GET /api/admin/event` contains the event with the marker.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_event_post_and_query(ctx: &mut TestContext) {
    let product_id = "scenario_product_event";
    let device_id = "scenario_device_event";
    let topic = event_post_topic(product_id, device_id);

    let marker = format!(
        "marker-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let payload = json!({
        "id": "event-test-001",
        "ack": 0,
        "params": {
            "event": "mqtt_e2e_boot",
            "marker": marker
        }
    });

    let msg = mqtt_publish_message(device_id, &topic, &payload);
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let events = resp["data"].as_array().expect("Expected data array");
    let found = events
        .iter()
        .any(|row| row["events"]["marker"].as_str() == Some(marker.as_str()));
    assert!(found, "Event with marker '{marker}' not found in response");
}

/// Verifies: admin creates property command → webhook simulates delivery →
/// device replies via webhook → command status becomes Success.
///
/// Mirrors demo steps:
/// 1. `POST /api/admin/property/command` → 201
/// 2. Manually set status to Sent (simulates MQTT delivery)
/// 3. `POST /api/thing/service/set_reply` with `{id:"property:{cmd_id}", code:200}`
/// 4. `GET /api/admin/property/command` → status = "Success"
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_command_lifecycle(ctx: &mut TestContext) {
    use crate::db::models::CommandStatus;

    let product_id = "scenario_product_cmd";
    let device_id = "scenario_device_cmd";

    // 1. Create property command
    let command_value = json!({ "power": false, "brightness": 42 });
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: command_value.clone(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Query command to get its id, verify status is Pending
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commands = resp["data"].as_array().expect("Expected data array");
    assert_eq!(commands.len(), 1);
    let command_id = commands[0]["id"].as_i64().expect("Command must have id");
    assert_eq!(commands[0]["status"], "Pending");

    // 3. Simulate MQTT delivery: update status from Pending → Sent
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

    // 4. Device replies via the unified service_set_reply webhook with code 200.
    // The property-private `/property/set_reply` route was deleted along with its
    // batch reply DTO (the old `{data: Vec<i64>}` shape); the reply now
    // correlates by the top-level string `id` ("property:{db_id}") and routes
    // through `POST /api/thing/service/set_reply`.
    let reply_topic = service_set_reply_topic(product_id, device_id, "property");
    let reply_payload = json!({
        "id": format!("property:{command_id}"),
        "code": 200
    });
    let reply_msg = mqtt_publish_message(device_id, &reply_topic, &reply_payload);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 5. Verify command status is now Success
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commands = resp["data"].as_array().expect("Expected data array");
    let updated = commands
        .iter()
        .find(|c| c["id"].as_i64() == Some(command_id))
        .expect("Command should exist");
    assert_eq!(updated["status"], "Success");
}

/// Full integrated device flow: property post → event post → command create →
/// command delivery → command reply → all verified.
///
/// Mirrors the entire `mqtt-device-flow-demo.e2e.ts` test without MQTT client.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_full_device_flow(ctx: &mut TestContext) {
    use crate::db::models::CommandStatus;

    let product_id = "scenario_product_full";
    let device_id = "scenario_device_full";

    // --- Step 1: Post properties ---
    let temperature: f64 = 20.0 + (rand_float() * 10.0);
    let prop_topic = property_post_topic(product_id, device_id);
    let prop_payload = json!({
        "id": "full-prop-001",
        "ack": 0,
        "params": {
            "temperature": temperature,
            "humidity": 51,
            "power": true
        }
    });
    let msg = mqtt_publish_message(device_id, &prop_topic, &prop_payload);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/thing/property/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Property post failed");

    // Verify properties
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let temp_val = &resp["data"][0]["properties"]["temperature"]["value"];
    let temp: f64 = temp_val
        .as_f64()
        .unwrap_or_else(|| panic!("Expected number, got {temp_val}"));
    assert!(
        (temp - temperature).abs() < f64::EPSILON,
        "Expected temperature {temperature}, got {temp}"
    );

    // --- Step 2: Post event ---
    let marker = format!(
        "full-flow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let event_topic = event_post_topic(product_id, device_id);
    let event_payload = json!({
        "id": "full-event-001",
        "ack": 0,
        "params": {
            "event": "mqtt_e2e_boot",
            "marker": marker
        }
    });
    let msg = mqtt_publish_message(device_id, &event_topic, &event_payload);
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Event post failed");

    // Verify event
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let found = resp["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["events"]["marker"].as_str() == Some(marker.as_str()));
    assert!(found, "Event with marker not found");

    // --- Step 3: Create property command ---
    let command_value = json!({ "power": false, "brightness": 42 });
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: command_value.clone(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Command creation failed");

    // Get command id
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let command_id = resp["data"][0]["id"]
        .as_i64()
        .expect("Command must have id");

    // Simulate delivery: Pending → Sent
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

    // --- Step 4: Device replies via the unified service_set_reply webhook ---
    let reply_topic = service_set_reply_topic(product_id, device_id, "property");
    let reply_payload = json!({
        "id": format!("property:{command_id}"),
        "code": 200
    });
    let msg = mqtt_publish_message(device_id, &reply_topic, &reply_payload);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "Reply post failed");

    // --- Step 5: Verify command status is Success ---
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cmd = resp["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_i64() == Some(command_id))
        .expect("Command must exist");
    assert_eq!(cmd["status"], "Success", "Command status should be Success");
}

fn rand_float() -> f64 {
    use std::time::SystemTime;
    let ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (ns % 1000) as f64 / 1000.0
}

// ===========================================================================
// Pending merge-order regression (last-write-wins, all callers)
//
// WHY this block exists: `update_pending_commands_to_sent`'s `RETURNING` is
// unordered; `send_property_command_to_device` (utils.rs) must sort the returned
// Vec by `created_time ASC, id ASC` before the shallow merge so that, for same
// key written by multiple Pending commands, the LAST write wins. This is a
// regression guard: it asserts that the merged value delivered to the device
// (captured from the RMQTT HTTP publish call) equals the last-written value,
// and that the fix applies identically to ALL THREE callers of
// `send_property_command_to_device`:
//   1. create_property_command        (POST /api/admin/property/command)
//   2. service_set_subscribe          (POST /api/thing/service/set_subscribe)
//   3. set_property_desired           (PUT /api/admin/property/shadow/desired)
//
// The default `TestContext` points rmqtt_client at an unreachable URL, so a
// publish call there fails and no payload can be observed. `MergeOrderTestContext`
// instead points rmqtt at a mockito server that (a) answers
// `GET /subscriptions?clientid=` with a matching subscription so the
// `is_subscribed_to_properties` gate opens for the HTTP callers, and (b) accepts
// `POST /mqtt/publish` while appending the parsed `{id, params}` envelope into
// an `Arc<Mutex<Vec<CapturedPublish>>>` cloned between the mock (which needs a
// `Send + Sync + 'static` capture callback) and the test context. The Vec is
// required because each Pending row is published independently.
// ===========================================================================

/// Test context for merge-order regression. Mirrors `simple_tests::TestContext`
/// but reroutes the RMQTT HTTP base URL to a mockito server so the published
/// `property/set` payload can be captured.
struct MergeOrderTestContext {
    service: Router,
    /// Every captured `POST /mqtt/publish` body, parsed into the spec envelope
    /// fields (`{id, params}`). Property publishes are single-row: each
    /// Pending row is now published independently with `id = "property:{db_id}"`
    /// and `params` = the bare business object (no legacy `{ids, data}` batch
    /// blob), so the regression assertions need the full publish history rather
    /// than just the latest.
    captured_messages: Arc<Mutex<Vec<CapturedPublish>>>,
    _admin_pool: PgPool,
    schema_name: String,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _mock_server: mockito::ServerGuard,
    _temp_dir: TempDir,
}

/// One captured `POST /mqtt/publish`: the platform correlation `id`
/// (`property:{db_id}`) and the per-row `params` object.
#[derive(Debug, Clone)]
struct CapturedPublish {
    id: String,
    params: JsonValue,
}

impl MergeOrderTestContext {
    /// Drain and return all captured publishes, clearing the buffer so
    /// subsequent triggers start from a known-empty state.
    async fn take_published_messages(&self) -> Vec<CapturedPublish> {
        self.captured_messages.lock().await.drain(..).collect()
    }
}

impl AsyncTestContext for MergeOrderTestContext {
    async fn setup() -> MergeOrderTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        // mockito server standing in for the RMQTT HTTP API. Two endpoints:
        //  - GET /subscriptions?clientid=<x> -> returns a subscription whose
        //    topic matches `{product}/{device}/thing/service/property/set`,
        //    so `is_subscribed_to_properties` reports the device as online.
        //  - POST /mqtt/publish -> 200, while appending the parsed envelope
        //    (`{id, params}`) into `captured_messages`. Each
        //    Pending row independently, so the buffer is a Vec (append-only)
        //    and tests drain the full history via `take_published_messages`.
        let mut server = mockito::Server::new_async().await;
        let captured: Arc<Mutex<Vec<CapturedPublish>>> = Arc::new(Mutex::new(Vec::new()));

        // NOT-SUBSCRIBED mock (registered FIRST so mockito matches it first for
        // the gated-skip-drain scenario). Devices whose clientid carries the
        // `gate-notsub` marker report ZERO subscriptions, exercising the
        // subscription-readiness gate added to service_set_subscribe /
        // device_connect (design offline-queued-delivery-drain-timing.md §4).
        // The drain must be skipped and rows stay Pending.
        server
            .mock(
                "GET",
                mockito::Matcher::Regex(
                    r"^/subscriptions\?.*clientid=[^&]*gate-notsub".to_string(),
                ),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        // Match any clientid; the query string carries it. Regex matches the
        // path prefix so the query value is irrelevant to the mock.
        server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/subscriptions\?".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            // topic_filter is reported as `topic` (see Subscription field alias).
            // Use a wildcard filter so `mqtt_topic_matches` accepts any
            // product/device pair used by the tests below.
            .with_body(r#"[{"topic_filter":"+/+/thing/service/property/set","qos":2}]"#)
            .create_async()
            .await;

        let captured_for_publish = captured.clone();
        server
            .mock("POST", "/mqtt/publish")
            .with_status(200)
            .with_body("")
            .with_body_from_request(move |req| {
                // Capture each published spec envelope. The `/mqtt/publish`
                // request body is a `PublishRequest` whose `payload` field is a
                // STRINGIFIED `MqttPayload` JSON (`{ id, ack, params }`), not a
                // nested object. Parse the outer body, then parse the `payload`
                // string, then extract `id` + `params` and append a
                // `CapturedPublish`. On parse failure we append a Null-params
                // entry so the test fails loudly rather than silently
                // under-counting publishes.
                let body = req.body().map(|b| b.as_slice()).unwrap_or(&[]);
                let outer: JsonValue = serde_json::from_slice(body).unwrap_or(JsonValue::Null);
                let payload_str = outer.get("payload").and_then(|v| v.as_str()).unwrap_or("");
                let payload: JsonValue =
                    serde_json::from_str(payload_str).unwrap_or(JsonValue::Null);
                let entry = CapturedPublish {
                    id: payload
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    params: payload.get("params").cloned().unwrap_or(JsonValue::Null),
                };
                // The mockito capture callback is sync and must be
                // Send+Sync+'static, so it cannot await the async Mutex.
                // try_lock avoids blocking the mock thread; on contention we
                // drop the capture rather than block (each scenario uses a
                // fresh context, so contention is absent in practice).
                if let Ok(mut guard) = captured_for_publish.try_lock() {
                    guard.push(entry);
                }
                Vec::new()
            })
            .expect_at_least(1)
            .create_async()
            .await;

        // Build config with mqtt.url pointing at the mockito server.
        let s3_config = S3Config {
            endpoint: test_s3_endpoint(),
            region: "us-east-1".to_string(),
            access_key: "fake_access".to_string(),
            secret_key: "fake_secret".to_string(),
            bucket: "fake_bucket".to_string(),
            directories: vec!["/*".to_string()],
            expired_seconds: 60,
        };
        let temp_dir = tempdir().unwrap();
        let mut config = Config {
            s3: Some(s3_config),
            mqtt: MqttConfig {
                url: server.url(),
                property_command: PropertyCommandConfig {
                    publish: PropertyCommandPublishConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                        topic: "${productId}/$clientid/thing/service/property/set".to_string(),
                        retries: 0,
                    },
                },
                ..Config::default().mqtt
            },
            ..Config::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let s3_client = config.s3.as_ref().map(|s3| S3Client::new(s3).unwrap());

        let app_state = Arc::new(AppState {
            db: db_service.clone(),
            rmqtt_client: rmqtt_client.clone(),
            config: config.clone(),
            cache: schema_cache.clone(),
            s3_client: s3_client.clone(),
        });
        let admin_state = Arc::new(AdminAppState {
            db: db_service,
            rmqtt_client,
            config: config.clone(),
            cache: schema_cache,
            s3_client,
            rule_cache: crate::rule_engine::RuleCache::new_in_memory(),
            task_set: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
        });

        let router = create_router(
            config,
            app_state.clone(),
            admin_state.clone(),
            None,
            crate::api::tests::simple_tests::empty_factory_auth_state(),
        );

        MergeOrderTestContext {
            service: router,
            captured_messages: captured,
            _admin_pool: admin_pool,
            schema_name,
            _app_state: app_state,
            _admin_state: admin_state,
            _mock_server: server,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

// ---------------------------------------------------------------------------
// Regression: create_property_command caller — last-write-wins merge order.
//
// Covers: sort_by created_time ASC, id ASC before shallow merge; the fix
//         must affect ALL callers.
// Caller 1/3: POST /api/admin/property/command -> create_property_command,
//         which calls send_property_command_to_device when the device is
//         reported as subscribed (simulated via the mockito /subscriptions
//         endpoint).
// WHY: with three Pending commands on the SAME key `brightness` (values
//      10, 20, 30 in created_time order), the merged value delivered to the
//      device MUST be 30 (last write wins). Without the sort the merge order
//      is non-deterministic, so this guards the deterministic fix.
// ---------------------------------------------------------------------------
#[test_context(MergeOrderTestContext)]
#[tokio::test]
async fn scenario_merge_order_create_command(ctx: &mut MergeOrderTestContext) {
    use crate::db::models::CommandSource;
    let product_id = "mrg_product_create";
    let device_id = "mrg_device_create";

    // Insert three Pending commands on the same key with strictly increasing
    // created_time (natural DB-sequence order from consecutive inserts).
    // NOTE: create_property_command itself inserts only ONE command per call and
    // would also immediately drain; to set up multiple Pending on the same key
    // we insert the first two directly via DB (Pending), then trigger the
    // caller with the third via the HTTP endpoint so the drain happens through
    // the real create_property_command path.
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 10 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 20 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();

    // Third command goes through the real caller; the device is "subscribed"
    // (mockito /subscriptions) so create_property_command drains all three
    // Pending -> Sent and publishes the merged payload.
    let cmd_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: json!({ "brightness": 30 }),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &cmd_req,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "command creation should return 201"
    );

    // Property publishes are single-row: each Pending row is published
    // independently with `id = "property:{db_id}"` and `params` = the bare
    // business object (no legacy `{ids, data}` batch blob). The sort fix
    // (created_time ASC, id ASC) is now expressed as the ORDER in which the
    // rows are claimed and published: 10, then 20, then 30.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        3,
        "all three Pending commands must be drained (one publish per row, no batching)"
    );
    // Each publish carries its own bare `params` object (single-row envelope).
    let brightnesses: Vec<&JsonValue> = published.iter().map(|m| &m.params["brightness"]).collect();
    assert_eq!(
        brightnesses,
        vec![&json!(10), &json!(20), &json!(30)],
        "rows must be drained in created_time ASC, id ASC order (10, 20, 30); \
         a non-deterministic claim order would surface a different sequence"
    );
    // Each `id` is the platform correlation id `property:{db_id}` (string, not
    // a `data:[id]` array). They must all be distinct.
    let ids: Vec<&str> = published.iter().map(|m| m.id.as_str()).collect();
    assert!(
        ids.iter().all(|id| id.starts_with("property:")),
        "every publish id must be 'property:{{db_id}}'; got {ids:?}"
    );
    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "the three correlation ids must be distinct"
    );
}

// ---------------------------------------------------------------------------
// Regression: service_set_subscribe caller — last-write-wins merge order.
//
// Covers: last-write-wins merge order, all callers.
// Caller 2/3: POST /api/thing/service/set_subscribe (handlers.rs) — the
//         US-DV-004/009 online-convergence hook (the deleted property-private
//         route was unified into this single service hook). It calls
//         send_property_command_to_device UNCONDITIONALLY (no subscription
//         gate), making it the cleanest trigger for the merge-order regression.
// WHY: the offline-convergence hook drains whatever is queued for the device
//      when it (re)subscribes; the merged value must be the last write.
// ---------------------------------------------------------------------------
#[test_context(MergeOrderTestContext)]
#[tokio::test]
async fn scenario_merge_order_property_set_subscribe(ctx: &mut MergeOrderTestContext) {
    use crate::db::models::CommandSource;
    let product_id = "mrg_product_subscribe";
    let device_id = "mrg_device_subscribe";

    // Queue three Pending commands on the same key with increasing created_time.
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 10 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 20 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 30 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();

    // Trigger the caller via the UNIFIED service_set_subscribe webhook
    // (the deleted `/property/set_subscribe` route was replaced with
    // `/service/set_subscribe`, which drains both property and action pending).
    // productId comes from the WebHook `username` (the device subscribes to the
    // wildcard `+/{device}/thing/service/+/set`, so productId is not in the
    // topic); the topic's second segment must equal the clientId.
    let subscribe_body = json!({
        "clientid": device_id,
        "username": product_id,
        "topic": format!("+/{device_id}/thing/service/+/set"),
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_subscribe",
        &subscribe_body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "service_set_subscribe should return 204"
    );

    // Publishes are single-row: three rows -> three independent
    // spec envelopes, drained in created_time ASC, id ASC order.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        3,
        "all three Pending commands must be drained (one publish per row)"
    );
    let brightnesses: Vec<&JsonValue> = published.iter().map(|m| &m.params["brightness"]).collect();
    assert_eq!(
        brightnesses,
        vec![&json!(10), &json!(20), &json!(30)],
        "rows must be drained in created_time ASC, id ASC order (10, 20, 30); \
         the sort fix must apply to the service_set_subscribe caller too"
    );
    assert!(
        published.iter().all(|m| m.id.starts_with("property:")),
        "every publish id must be 'property:{{db_id}}'"
    );
}

// ---------------------------------------------------------------------------
// Regression: set_property_desired caller — last-write-wins merge order.
//
// Covers: last-write-wins merge order, all callers.
// Caller 3/3: PUT /api/admin/property/shadow/desired -> set_property_desired
//         It inserts the delta as a Pending command and, when the
//         device is subscribed, calls send_property_command_to_device.
// WHY: Set-Desired is the newest caller added on top of the shared send path;
//      the sort fix must keep last-write-wins semantics here as well.
//      We pre-seed two older Pending deltas on the same key, then trigger the
//      third via the real Set-Desired endpoint so the drain goes through the
//      set_property_desired caller.
// ---------------------------------------------------------------------------
#[test_context(MergeOrderTestContext)]
#[tokio::test]
async fn scenario_merge_order_set_desired(ctx: &mut MergeOrderTestContext) {
    use crate::db::models::CommandSource;
    let product_id = "mrg_product_desired";
    let device_id = "mrg_device_desired";

    // Two older Pending deltas on the same key (direct DB insert, increasing
    // created_time). These represent previously-set desired deltas not yet
    // drained.
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 10 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 20 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();

    // Third delta via the real Set-Desired caller. Device is "subscribed"
    // (mockito /subscriptions) so the handler drains all three Pending and
    // publishes the merged command.
    let body = json!({
        "product_id": product_id,
        "device_id": device_id,
        "desired": { "brightness": 30 },
    });
    let (status, resp_text) = request_json(
        &ctx.service,
        Method::PUT,
        "/api/admin/property/shadow/desired",
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Set-Desired should return 200");
    let resp: JsonValue = serde_json::from_str(&resp_text).unwrap_or(JsonValue::Null);
    assert_eq!(
        resp["delta"]["brightness"], 30,
        "delta must carry the bare desired value"
    );
    assert_eq!(resp["pushed"], true, "non-empty delta must enqueue+push");

    // Publishes are single-row: the three Pending deltas (two seeded
    // + the one Set-Desired just enqueued) drain as three independent spec
    // envelopes in created_time ASC, id ASC order: 10, 20, 30.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        3,
        "all three Pending deltas must be drained (one publish per row)"
    );
    let brightnesses: Vec<&JsonValue> = published.iter().map(|m| &m.params["brightness"]).collect();
    assert_eq!(
        brightnesses,
        vec![&json!(10), &json!(20), &json!(30)],
        "rows must be drained in created_time ASC, id ASC order (10, 20, 30); \
         the sort fix must apply to the set_property_desired caller too"
    );
    assert!(
        published.iter().all(|m| m.id.starts_with("property:")),
        "every publish id must be 'property:{{db_id}}'"
    );
}

// ---------------------------------------------------------------------------
// Regression: subscription-readiness gate on the offline-queue drain.
//
// Covers: design offline-queued-delivery-drain-timing.md §4 方案 A / A.1.
//
// WHY (Rule 9): the `client_subscribe` webhook (auto-subscription) fires during
// CONNECT, BEFORE the device's SUBACK is registered in the broker's
// subscription table. Draining at that moment loses the message. The gate
// (added to service_set_subscribe and device_connect) queries the broker's
// live `/subscriptions` and skips the drain when the device is not yet
// subscribed, leaving rows Pending for a later trigger. These two scenarios
// pin that contract: (1) not-subscribed -> no publish, rows stay Pending;
// (2) device_connect fallback drains when the device IS subscribed.
//
// The MergeOrderTestContext mockito server reports an EMPTY subscription list
// for any clientid containing the `gate-notsub` marker (registered first so
// mockito matches it first), and a wildcard subscription for everyone else.
// ---------------------------------------------------------------------------

#[test_context(MergeOrderTestContext)]
#[tokio::test]
async fn scenario_service_set_subscribe_skips_drain_when_not_subscribed(
    ctx: &mut MergeOrderTestContext,
) {
    use crate::db::models::{CommandSource, CommandStatus};
    let product_id = "gate_product";
    // clientid carries the `gate-notsub` marker so the mockito /subscriptions
    // mock returns [] (not subscribed), exercising the skip-drain gate.
    let device_id = "gate-notsub-device";

    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 99 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();

    // Trigger the drain via the service_set_subscribe webhook. productId comes
    // from the WebHook username (the subscribe topic's first segment is `+`).
    let subscribe_body = json!({
        "clientid": device_id,
        "username": product_id,
        "topic": format!("+/{device_id}/thing/service/+/set"),
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_subscribe",
        &subscribe_body,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Gate must have skipped the drain: no publish, row stays Pending.
    let published = ctx.take_published_messages().await;
    assert!(
        published.is_empty(),
        "service_set_subscribe must NOT drain when the device is not yet subscribed at the broker"
    );
    let (_, pending_count) = ctx
        ._app_state
        .db
        .query_property_commands_by_source(
            product_id,
            Some(device_id),
            Some(CommandStatus::Pending),
            None,
            1,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        pending_count, 1,
        "the Pending row must remain Pending (not flipped to Sent) when the gate skips the drain"
    );
}

#[test_context(MergeOrderTestContext)]
#[tokio::test]
async fn scenario_device_connect_fallback_drains_when_subscribed(ctx: &mut MergeOrderTestContext) {
    use crate::db::models::CommandSource;
    let product_id = "gate_product";
    // No `gate-notsub` marker -> mockito reports the wildcard subscription,
    // so the device_connect fallback drain gate opens.
    let device_id = "gate-sub-device";

    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 7 }),
            CommandSource::OneShot,
        )
        .await
        .unwrap();

    // Trigger the fallback drain via the client_connected webhook.
    // ipaddress MUST contain a port (database.rs upsert_device_status_connect
    // splits on ':'); mirror the real RMQTT webhook shape.
    let connect_body = json!({
        "node": 1,
        "ipaddress": "127.0.0.1:12345",
        "clientid": device_id,
        "product_id": product_id,
        "device_id": device_id,
        "username": product_id,
        "keepalive": 60,
        "proto_ver": 4,
        "clean_start": true,
        "connected_at": 0,
        "session_present": false,
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/device/connect",
        &connect_body,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Fallback drain must have published the queued row.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        1,
        "device_connect must drain the queued property command when the device is subscribed"
    );
    assert_eq!(published[0].params["brightness"], json!(7));
    assert!(
        published[0].id.starts_with("property:"),
        "published id must be 'property:{{db_id}}'"
    );
}
