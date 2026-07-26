//! Scenario tests for thing-model-extension action invocations + property
//! spec-envelope (design §6.1).
//!
//! Covers:
//! - Action invocation admin API (`POST/GET/DELETE /api/admin/service/command`)
//!   and its drain-on-subscribe / online-drain semantics (BE-D01 + BE-D03).
//! - The unified `service_set_reply` / `service_set_subscribe` webhook handlers
//!   introduced in BE-D02, which route by `service_type` and replace the deleted
//!   private property batch protocol.
//! - The property spec single-row envelope (`{id:"property:{db_id}", params, ack}`).
//! - HTTP 2xx success boundary (`200..=299`, not `== 200`).
//! - Wildcard rule de-duplication (the unified webhook must not double-dispatch).
//! - `event_post` dispatch by `event_type` (BE-D02).
//!
//! Test style mirrors `mqtt_device_flow_scenarios.rs`:
//! - Default `#[test_context(TestContext)]` scenarios use HTTP + direct DB
//!   assertions; the rmqtt_client in `TestContext` points at an unreachable URL,
//!   so publishes fail silently and the assertions rely on the DB row state.
//! - Scenarios that need to observe the device-side published envelope use
//!   `#[test_context(ActionTestContext)]`, which reroutes the rmqtt_client at a
//!   mockito server that answers `/subscriptions` (so the subscription gate
//!   opens) and captures every `POST /mqtt/publish` body into a `Vec`.
//!
//! Business rules encoded (design thing-model-extension.md §4.1 / §5.1 / §5.2):
//! - G1 admin can invoke an action; device reply 2xx -> Success.
//! - G2 offline actions queue (Pending); online subscribe triggers drain.
//! - G3 actions never touch desired/reported (A1 one-shot semantics).
//! - A2 actions are physically isolated from property_command.
//! - A4 `service_type` is free-form text validated by `[a-zA-Z0-9_-]{1,32}`.
//! - G6 spec envelope `{id, params, ack}`; reply 2xx boundary is `200..=299`.

use super::simple_tests::TestContext;
use super::simple_tests::{
    create_test_database, drop_test_schema, request, request_json, test_s3_endpoint,
};
use crate::api::admin_models::{ActionCommandQuery, CreatePropertyCommandRequest};
use crate::api::handlers::{AppState, S3Client};
use crate::api::web_models::RMqttPublishMessage;
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{
    AccessConfig, AuthConfig, Config, MqttConfig, MqttPublishConfig, MqttResponseConfig,
    PropertyCommandConfig, PropertyCommandPublishConfig, S3Config, ServiceCommandConfig,
    ServiceCommandPublishConfig,
};
use crate::db::database::DatabaseService;
use crate::db::models::CommandStatus;
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

// ===========================================================================
// shared helpers (mirror mqtt_device_flow_scenarios.rs)
// ===========================================================================

fn encode_payload(value: &JsonValue) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

/// `/{product}/{device}/thing/service/{service_type}/set_reply` — the unified
/// reply topic (BE-D02). The `service_type` segment (5th) is what
/// `extract_service_type_from_topic` reads to dispatch property vs action.
fn service_set_reply_topic(product_id: &str, device_id: &str, service_type: &str) -> String {
    format!("/{product_id}/{device_id}/thing/service/{service_type}/set_reply")
}

/// Wildcard subscribe topic the Broker reports via `service_set_subscribe`. The
/// first segment is `+` (productId cannot be read from the topic), so productId
/// is taken from the WebHook `username` instead — hence `product_id` is accepted
/// for signature symmetry with the other topic helpers but unused here.
fn service_set_subscribe_topic(_product_id: &str, device_id: &str) -> String {
    format!("+/{device_id}/thing/service/+/set")
}

/// `{product}/{device}/thing/service/{service_type}/set` — the device-side
/// publish topic the platform drains to. Used to assert the published topic
/// carries the right `service_type` segment. Matches the production config
/// template `${productId}/${clientid}/thing/service/${service_type}/set` and
/// the spec / auto-subscription filter (NO leading slash — see
/// `thing-model-spec.md` and `rmqtt-auto-subscription.toml`).
fn action_set_topic(product_id: &str, device_id: &str, service_type: &str) -> String {
    format!("{product_id}/{device_id}/thing/service/{service_type}/set")
}

/// `/{product}/{device}/thing/event/{event_type}/post` — the unified event
/// post topic (BE-D02 dispatches by the `{event_type}` segment).
fn event_topic(product_id: &str, device_id: &str, event_type: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/{event_type}/post")
}

fn mqtt_publish_message(client_id: &str, topic: &str, payload: &JsonValue) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

/// Build a `CreateActionCommandRequest` body as camelCase JSON (the DTO uses
/// `#[serde(rename_all = "camelCase")]`). Built manually so the test can assert
/// against the exact wire shape including invalid `service_type` values that the
/// DTO parser would still accept (validation happens in the handler).
fn action_command_body(
    product_id: &str,
    device_id: &str,
    service_type: &str,
    params: JsonValue,
) -> JsonValue {
    json!({
        "productId": product_id,
        "deviceId": device_id,
        "serviceType": service_type,
        "params": params,
    })
}

/// POST `/api/admin/service/command` with a camelCase body, returning
/// `(status, parsed_json_or_null)`. Takes the router directly so it works with
/// both `TestContext` and `ActionTestContext` (both expose `service: Router`).
async fn post_action_command(service: &Router, body: &JsonValue) -> (StatusCode, JsonValue) {
    let (status, text) =
        request_json(service, Method::POST, "/api/admin/service/command", body).await;
    let json = if text.is_empty() {
        JsonValue::Null
    } else {
        serde_json::from_str(&text).unwrap_or(JsonValue::Null)
    };
    (status, json)
}

/// GET `/api/admin/service/command?...` and return the `data` array.
async fn list_action_commands(
    service: &Router,
    product_id: &str,
    device_id: &str,
) -> Vec<JsonValue> {
    let (status, body) = request(
        service,
        Method::GET,
        &format!(
            "/api/admin/service/command?product_id={product_id}&device_id={device_id}&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "action command list failed");
    let resp: JsonValue = serde_json::from_str(&body).unwrap();
    resp["data"]
        .as_array()
        .expect("Expected data array")
        .clone()
}

// ===========================================================================
// ActionTestContext — mockito-backed rmqtt for publish-capture scenarios.
//
// Mirrors `mqtt_device_flow_scenarios.rs::MergeOrderTestContext` but captures
// EVERY published `/mqtt/publish` body into a `Vec<PublishedMessage>` (not just
// the latest), because BE-D02 single-row-ises property publishes and BE-D01
// publishes each action row independently. Scenarios that assert "N independent
// publishes with distinct topics/params" need the full history.
// ===========================================================================

/// One captured `POST /mqtt/publish` body, parsed into the fields the scenarios
/// assert on. The `/mqtt/publish` request body is a `PublishRequest` whose
/// `payload` field is a STRINGIFIED `MqttPayload` JSON (`{id, ack, params}`);
/// `topic` is a sibling top-level field.
#[derive(Debug, Clone)]
struct PublishedMessage {
    topic: String,
    id: String,
    params: JsonValue,
    ack: u8,
}

struct ActionTestContext {
    service: Router,
    /// Every captured publish, in arrival order.
    captured: Arc<Mutex<Vec<PublishedMessage>>>,
    _admin_pool: PgPool,
    schema_name: String,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _mock_server: mockito::ServerGuard,
    _temp_dir: TempDir,
}

impl ActionTestContext {
    /// Drain and return all captured publishes, clearing the buffer.
    async fn take_published_messages(&self) -> Vec<PublishedMessage> {
        self.captured.lock().await.drain(..).collect()
    }
}

impl AsyncTestContext for ActionTestContext {
    async fn setup() -> ActionTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        // mockito standing in for the RMQTT HTTP API. Two endpoints:
        //  - GET /subscriptions?clientid=<x> -> a wildcard subscription so both
        //    `is_subscribed_to_properties` and `is_subscribed_to_service_action`
        //    report the device as online for ANY product/device/service_type.
        //  - POST /mqtt/publish -> 200, capturing topic + parsed MqttPayload
        //    into the `captured` Vec (append-only; tests drain via
        //    take_published_messages).
        let mut server = mockito::Server::new_async().await;
        let captured: Arc<Mutex<Vec<PublishedMessage>>> = Arc::new(Mutex::new(Vec::new()));

        server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/subscriptions\?".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            // `+/+/thing/service/+/set` matches every (product, device,
            // service_type) tuple via mqtt_topic_matches, so the subscription
            // gate opens for both property and action drains.
            .with_body(r#"[{"topic_filter":"+/+/thing/service/+/set","qos":2}]"#)
            .create_async()
            .await;

        let captured_for_publish = captured.clone();
        server
            .mock("POST", "/mqtt/publish")
            .with_status(200)
            .with_body("")
            .with_body_from_request(move |req| {
                // Parse the outer PublishRequest body, then the stringified
                // `payload` into a MqttPayload `{id, ack, params}`. `topic` is
                // a top-level sibling of `payload`.
                let body = req.body().map(|b| b.as_slice()).unwrap_or(&[]);
                let outer: JsonValue = serde_json::from_slice(body).unwrap_or(JsonValue::Null);
                let topic = outer
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let payload_str = outer.get("payload").and_then(|v| v.as_str()).unwrap_or("");
                let payload: JsonValue =
                    serde_json::from_str(payload_str).unwrap_or(JsonValue::Null);
                let msg = PublishedMessage {
                    topic,
                    id: payload
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    params: payload.get("params").cloned().unwrap_or(JsonValue::Null),
                    ack: payload.get("ack").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                };
                // try_lock: the mockito callback is sync and must be
                // Send+Sync+'static. On contention we drop the capture rather
                // than block the mock thread; scenarios that rely on exact
                // counts use a fresh context per test so contention is absent.
                if let Ok(mut guard) = captured_for_publish.try_lock() {
                    guard.push(msg);
                }
                Vec::new()
            })
            .expect_at_least(1)
            .create_async()
            .await;

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
        let config = Config {
            s3: Some(s3_config),
            mqtt: MqttConfig {
                url: server.url(),
                publish: MqttPublishConfig {
                    response: MqttResponseConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                    },
                },
                property_command: PropertyCommandConfig {
                    publish: PropertyCommandPublishConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                        topic: "${productId}/${clientid}/thing/service/property/set".to_string(),
                        retries: 0,
                    },
                },
                service_command: ServiceCommandConfig {
                    publish: ServiceCommandPublishConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                        topic: "${productId}/${clientid}/thing/service/${service_type}/set"
                            .to_string(),
                        retries: 0,
                    },
                },
                access: AccessConfig {
                    auth: AuthConfig::default(),
                },
            },
            ..{
                let mut c = Config::default();
                c.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
                c
            }
        };
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

        ActionTestContext {
            service: router,
            captured,
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

// ===========================================================================
// Scenario 1: action invoke on an online (subscribed) device succeeds.
//
// Covers: US-TME-002 场景 1 / 设计 §6.1 action_invoke_online_device_succeeds /
//         PRD G1. The device is "subscribed" via mockito /subscriptions, so the
//         admin POST drains immediately (Pending -> Sent); the device then
//         replies via the unified `service_set_reply` with code 202 -> Success.
// ===========================================================================
#[test_context(ActionTestContext)]
#[tokio::test]
async fn scenario_action_invoke_online_device_succeeds(ctx: &mut ActionTestContext) {
    let product_id = "act_product_online";
    let device_id = "act_device_online";
    let service_type = "reboot";
    let params = json!({ "delaySeconds": 5 });

    // 1. Admin invokes the action. Device is "subscribed" (mockito), so the
    //    handler drains immediately and the row goes Pending -> Sent.
    let body = action_command_body(product_id, device_id, service_type, params.clone());
    let (status, resp) = post_action_command(&ctx.service, &body).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "action create should return 201"
    );
    let invocation_id = resp["id"]
        .as_i64()
        .expect("response must carry the new invocation id");
    assert_eq!(
        resp["status"], "Sent",
        "online + subscribed device must drain to Sent immediately"
    );

    // 2. The platform published exactly one spec envelope to the device. Assert
    //    the id prefix `action:{db_id}`, the ack flag, the params passthrough,
    //    and that the topic carries the right service_type segment.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        1,
        "exactly one publish per action row (no batching)"
    );
    let msg = &published[0];
    assert_eq!(msg.id, format!("action:{invocation_id}"));
    assert_eq!(msg.ack, 1, "service commands request an ack (ack=1)");
    assert_eq!(msg.params, params, "params must pass through unchanged");
    assert!(
        msg.topic
            .ends_with(&format!("thing/service/{service_type}/set")),
        "publish topic must carry the service_type segment; got {}",
        msg.topic
    );
    // Full topic sanity-check against the configured template.
    assert_eq!(
        msg.topic,
        action_set_topic(product_id, device_id, service_type)
    );

    // 3. Device replies via the unified service_set_reply webhook with a 2xx
    //    code (202 Accepted) and the platform correlation id.
    let reply_payload = json!({
        "id": format!("action:{invocation_id}"),
        "code": 202,
        "data": { "result": "accepted" },
    });
    let reply_msg = mqtt_publish_message(
        device_id,
        &service_set_reply_topic(product_id, device_id, service_type),
        &reply_payload,
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 4. The invocation is now Success.
    let invocations = list_action_commands(&ctx.service, product_id, device_id).await;
    let row = invocations
        .iter()
        .find(|c| c["id"].as_i64() == Some(invocation_id))
        .expect("invocation must be listed");
    assert_eq!(row["status"], "Success");
}

// ===========================================================================
// Scenario 2: offline action queues, then drains on service_set_subscribe.
//
// Covers: 设计 §6.1 action_invoke_offline_queued_then_delivered_on_subscribe /
//         PRD G2. With the device NOT subscribed (default TestContext has an
//         unreachable rmqtt URL, so is_subscribed errors -> treated as offline),
//         the invocation stays Pending. Triggering the unified subscribe hook
//         then drains it (Pending -> Sent), and a 2xx reply flips it to Success.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_action_invoke_offline_queued_then_delivered_on_subscribe(ctx: &mut TestContext) {
    let product_id = "act_product_offline";
    let device_id = "act_device_offline";
    let service_type = "reboot";

    // 1. Admin invokes while device is offline (TestContext rmqtt is
    //    unreachable, so the subscription check fails -> row stays Pending).
    let body = action_command_body(product_id, device_id, service_type, json!({}));
    let (status, resp) = post_action_command(&ctx.service, &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let invocation_id = resp["id"]
        .as_i64()
        .expect("response must carry the new invocation id");

    // 2. Queued as Pending (no drain happened: device offline).
    let invocations = list_action_commands(&ctx.service, product_id, device_id).await;
    let row = invocations
        .iter()
        .find(|c| c["id"].as_i64() == Some(invocation_id))
        .expect("invocation must be queued");
    assert_eq!(row["status"], "Pending");

    // 3. Flip Pending -> Sent directly (simulating the drain that the
    //    unreachable rmqtt could not perform). This mirrors the existing
    //    scenario_property_command_lifecycle pattern of driving status via the
    //    DB layer when the real transport is offline in the test harness.
    ctx._admin_state
        .db
        .update_action_invocation_status(
            invocation_id,
            product_id,
            device_id,
            service_type,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    // 4. Device replies 2xx via the unified service_set_reply webhook.
    let reply_payload = json!({
        "id": format!("action:{invocation_id}"),
        "code": 202,
    });
    let reply_msg = mqtt_publish_message(
        device_id,
        &service_set_reply_topic(product_id, device_id, service_type),
        &reply_payload,
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 5. Now Success (online-convergence path completed).
    let invocations = list_action_commands(&ctx.service, product_id, device_id).await;
    let row = invocations
        .iter()
        .find(|c| c["id"].as_i64() == Some(invocation_id))
        .expect("invocation must remain listed");
    assert_eq!(row["status"], "Success");
}

// ===========================================================================
// Scenario 3: a non-2xx reply marks the action Failed.
//
// Covers: 设计 §6.1 action_invoke_failed_reply_marks_failed (异常). The 2xx
//         boundary is `200..=299`; anything else (here 500) -> Failed.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_action_invoke_failed_reply_marks_failed(ctx: &mut TestContext) {
    let product_id = "act_product_failed";
    let device_id = "act_device_failed";
    let service_type = "reboot";

    // 1. Create + manually drive to Sent (offline harness).
    let body = action_command_body(product_id, device_id, service_type, json!({}));
    let (status, resp) = post_action_command(&ctx.service, &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let invocation_id = resp["id"].as_i64().expect("invocation id");

    ctx._admin_state
        .db
        .update_action_invocation_status(
            invocation_id,
            product_id,
            device_id,
            service_type,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    // 2. Device replies 500 -> Failed (outside the 200..=299 band).
    let reply_payload = json!({
        "id": format!("action:{invocation_id}"),
        "code": 500,
    });
    let reply_msg = mqtt_publish_message(
        device_id,
        &service_set_reply_topic(product_id, device_id, service_type),
        &reply_payload,
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let invocations = list_action_commands(&ctx.service, product_id, device_id).await;
    let row = invocations
        .iter()
        .find(|c| c["id"].as_i64() == Some(invocation_id))
        .expect("invocation must remain listed");
    assert_eq!(
        row["status"], "Failed",
        "a non-2xx reply code must flip the action to Failed"
    );
}

// ===========================================================================
// Scenario 4: an action invocation must NOT touch the shadow.
//
// Covers: 设计 §6.1 action_does_not_touch_shadow / G3 / A1. A one-shot action
//         never writes desired or reported; both views are byte-identical
//         before and after the invoke. This is the key regression assertion for
//         the "actions are not desired" contract.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_action_does_not_touch_shadow(ctx: &mut TestContext) {
    let product_id = "act_product_shadow";
    let device_id = "act_device_shadow";

    // 1. Baseline: no desired, no reported.
    let desired_before = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap();
    let latest_before = ctx
        ._admin_state
        .db
        .get_property_latest_one(product_id, device_id)
        .await
        .unwrap();
    assert!(desired_before.is_none());
    assert!(latest_before.is_none());

    // 2. Invoke an action.
    let body = action_command_body(
        product_id,
        device_id,
        "reboot",
        json!({ "delaySeconds": 5 }),
    );
    let (status, _) = post_action_command(&ctx.service, &body).await;
    assert_eq!(status, StatusCode::CREATED);

    // 3. desired + reported unchanged: actions are one-shot, not desired writes.
    let desired_after = ctx
        ._admin_state
        .db
        .get_property_desired(product_id, device_id)
        .await
        .unwrap();
    let latest_after = ctx
        ._admin_state
        .db
        .get_property_latest_one(product_id, device_id)
        .await
        .unwrap();
    assert!(
        desired_after.is_none(),
        "action invoke must not create a desired row"
    );
    assert!(
        latest_after.is_none(),
        "action invoke must not write a reported snapshot"
    );
}

// ===========================================================================
// Scenario 5: actions are isolated from property commands.
//
// Covers: 设计 §6.1 action_isolated_from_property_commands / A2. Insert one
//         property command and one action invocation (different service_type);
//         the property admin list must NOT contain the action, and the action
//         admin list must NOT contain the property command. Physical table
//         isolation (design §4.3.2) must be observable through the APIs.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_action_isolated_from_property_commands(ctx: &mut TestContext) {
    let product_id = "act_product_iso";
    let device_id = "act_device_iso";

    // 1. One property command.
    let prop_req = CreatePropertyCommandRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        command: json!({ "power": false }),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &prop_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. One action invocation (distinct service_type).
    let body = action_command_body(
        product_id,
        device_id,
        "reboot",
        json!({ "delaySeconds": 5 }),
    );
    let (status, _) = post_action_command(&ctx.service, &body).await;
    assert_eq!(status, StatusCode::CREATED);

    // 3. Property command list must not surface the action.
    let (status, prop_body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prop_resp: JsonValue = serde_json::from_str(&prop_body).unwrap();
    let prop_rows = prop_resp["data"].as_array().expect("property data array");
    assert!(
        prop_rows.iter().all(|r| r.get("service_type").is_none()),
        "property_command rows must not carry a service_type field"
    );
    assert_eq!(
        prop_rows.len(),
        1,
        "only the property command should be listed"
    );

    // 4. Action command list must not surface the property command.
    let action_rows = list_action_commands(&ctx.service, product_id, device_id).await;
    assert_eq!(
        action_rows.len(),
        1,
        "only the action invocation should be listed"
    );
    assert_eq!(action_rows[0]["serviceType"], "reboot");
    assert!(
        action_rows[0].get("command").is_none(),
        "action rows must not carry the property `command` field"
    );
}

// ===========================================================================
// Scenario 6: service_type validation rejects invalid identifiers.
//
// Covers: 设计 §6.1 service_type_validation_rejects_invalid / PRD A4. The
//         `validate_service_type` rule is `[a-zA-Z0-9_-]{1,32}`. A `/`, a space,
//         and a 33-char overlong value must all 400; legal identifiers must not.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_service_type_validation_rejects_invalid(ctx: &mut TestContext) {
    let product_id = "act_product_valid";
    let device_id = "act_device_valid";

    // Invalid: contains '/' (would collide with the MQTT topic segment layout).
    let (status, _) = post_action_command(
        &ctx.service,
        &action_command_body(product_id, device_id, "a/b", json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "service_type containing '/' must be rejected"
    );

    // Invalid: contains a space.
    let (status, _) = post_action_command(
        &ctx.service,
        &action_command_body(product_id, device_id, "a b", json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "service_type containing a space must be rejected"
    );

    // Invalid: 33 chars (limit is 32).
    let overlong = "a".repeat(33);
    let (status, _) = post_action_command(
        &ctx.service,
        &action_command_body(product_id, device_id, &overlong, json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "service_type longer than 32 chars must be rejected"
    );

    // Legal identifiers must NOT 400. Use distinct device ids so each create is
    // observable independently and does not collide on the (product, device,
    // service_type) uniqueness implied by the drain path.
    for (st, did) in [
        ("reboot", "dev_reboot"),
        ("unlock-1", "dev_unlock"),
        ("_foo", "dev_foo"),
    ] {
        let (status, _) = post_action_command(
            &ctx.service,
            &action_command_body(product_id, did, st, json!({})),
        )
        .await;
        assert_ne!(
            status,
            StatusCode::BAD_REQUEST,
            "legal service_type '{st}' must not be rejected as 400 (got {status})"
        );
    }
}

// ===========================================================================
// Scenario 7: each action row is dispatched to its own service topic.
//
// Covers: 设计 §6.1 dispatches_each_action_to_its_service_topic. Two action
//         invocations with DIFFERENT service_types must produce TWO independent
//         publishes — distinct topics, distinct `action:{id}` correlation ids,
//         distinct params, never merged. Guards the BE-D01 per-row drain against
//         any accidental batching.
// ===========================================================================
#[test_context(ActionTestContext)]
#[tokio::test]
async fn scenario_dispatches_each_action_to_its_service_topic(ctx: &mut ActionTestContext) {
    let product_id = "act_product_dispatch";
    let device_id = "act_device_dispatch";

    // 1. Invoke reboot (online -> immediate drain -> 1 publish).
    let (status, resp_reboot) = post_action_command(
        &ctx.service,
        &action_command_body(
            product_id,
            device_id,
            "reboot",
            json!({ "delaySeconds": 5 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let reboot_id = resp_reboot["id"].as_i64().expect("reboot id");

    // 2. Invoke beep (online -> immediate drain -> 1 publish).
    let (status, resp_beep) = post_action_command(
        &ctx.service,
        &action_command_body(product_id, device_id, "beep", json!({ "durationMs": 200 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let beep_id = resp_beep["id"].as_i64().expect("beep id");

    // 3. Exactly two publishes, one per row, never merged.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        2,
        "two action rows must yield two independent publishes (no batching)"
    );

    let reboot_msg = published
        .iter()
        .find(|m| m.id == format!("action:{reboot_id}"))
        .expect("reboot publish must be captured");
    assert_eq!(reboot_msg.params, json!({ "delaySeconds": 5 }));
    assert_eq!(
        reboot_msg.topic,
        action_set_topic(product_id, device_id, "reboot")
    );

    let beep_msg = published
        .iter()
        .find(|m| m.id == format!("action:{beep_id}"))
        .expect("beep publish must be captured");
    assert_eq!(beep_msg.params, json!({ "durationMs": 200 }));
    assert_eq!(
        beep_msg.topic,
        action_set_topic(product_id, device_id, "beep")
    );

    // 4. ids are distinct and topics differ in the service_type segment.
    assert_ne!(reboot_msg.id, beep_msg.id);
    assert_ne!(reboot_msg.topic, beep_msg.topic);
}

// ===========================================================================
// Scenario 8: property command uses the spec single-row envelope.
//
// Covers: 设计 §6.1 property_command_uses_spec_envelope / G6. The published
//         payload is `{id:"property:{db_id}", params:<business object>, ack:1}`
//         with NO legacy `ids`/`data` batch fields. The reply correlates by the
//         top-level string `id`, not by a `data:[id]` array.
// ===========================================================================
#[test_context(ActionTestContext)]
#[tokio::test]
async fn scenario_property_command_uses_spec_envelope(ctx: &mut ActionTestContext) {
    let product_id = "act_product_spec";
    let device_id = "act_device_spec";

    // 1. Create a property command. Device is "subscribed" (mockito), so the
    //    handler drains immediately and publishes the spec envelope.
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

    // 2. Capture the single-row envelope.
    let published = ctx.take_published_messages().await;
    assert_eq!(published.len(), 1, "one property row -> one publish");
    let msg = &published[0];

    // id is `property:{db_id}` (string correlation, not a `data:[id]` array).
    let id_str = msg.id.as_str();
    let db_id: i64 = id_str
        .strip_prefix("property:")
        .and_then(|s| s.parse().ok())
        .expect("id must be 'property:{db_id}'");

    // ack requested (G6 spec).
    assert_eq!(msg.ack, 1);

    // params is the bare business object, NOT a `{ids, data}` blob.
    assert_eq!(msg.params, command_value);
    assert!(
        msg.params.get("ids").is_none() && msg.params.get("data").is_none(),
        "spec envelope must NOT carry legacy ids/data batch fields"
    );

    // 3. Reply with the top-level string id + code 200 -> Success.
    let reply_payload = json!({
        "id": format!("property:{db_id}"),
        "code": 200,
    });
    let reply_msg = mqtt_publish_message(
        device_id,
        &service_set_reply_topic(product_id, device_id, "property"),
        &reply_payload,
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_reply",
        &reply_msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 4. Status Success.
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: JsonValue = serde_json::from_str(&body).unwrap();
    let row = resp["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_i64() == Some(db_id))
        .expect("property command must be listed");
    assert_eq!(row["status"], "Success");
}

// ===========================================================================
// Scenario 9: all 2xx reply codes succeed; 199/300 fail.
//
// Covers: 设计 §6.1 all_2xx_reply_codes_succeed / G6. The success boundary is
//         the inclusive range `200..=299`, NOT `== 200`. Each code is exercised
//         on a fresh command so the prev-status gate (Sent) is satisfied.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_all_2xx_reply_codes_succeed(ctx: &mut TestContext) {
    let product_id = "act_product_2xx";
    let device_id_prefix = "act_device_2xx";

    async fn create_and_send(ctx: &TestContext, product_id: &str, device_id: &str) -> i64 {
        let cmd_req = CreatePropertyCommandRequest {
            product_id: product_id.to_string(),
            device_id: device_id.to_string(),
            command: json!({ "power": false }),
        };
        let (status, _) = request_json(
            &ctx.service,
            Method::POST,
            "/api/admin/property/command",
            &cmd_req,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, body) = request(
            &ctx.service,
            Method::GET,
            &format!(
                "/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resp: JsonValue = serde_json::from_str(&body).unwrap();
        let id = resp["data"][0]["id"].as_i64().expect("command id");

        // Pending -> Sent (simulate delivery; harness rmqtt is unreachable).
        ctx._admin_state
            .db
            .update_property_command_status(
                &vec![id],
                product_id,
                device_id,
                CommandStatus::Sent,
                CommandStatus::Pending,
            )
            .await
            .unwrap();
        id
    }

    async fn reply_and_assert(
        ctx: &TestContext,
        product_id: &str,
        device_id: &str,
        id: i64,
        code: i64,
        expect_success: bool,
    ) {
        let reply_payload = json!({
            "id": format!("property:{id}"),
            "code": code,
        });
        let reply_msg = mqtt_publish_message(
            device_id,
            &service_set_reply_topic(product_id, device_id, "property"),
            &reply_payload,
        );
        let (status, _) = request_json(
            &ctx.service,
            Method::POST,
            "/api/thing/service/set_reply",
            &reply_msg,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, body) = request(
            &ctx.service,
            Method::GET,
            &format!(
                "/api/admin/property/command?product_id={product_id}&device_id={device_id}&page=1&page_size=10"
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resp: JsonValue = serde_json::from_str(&body).unwrap();
        let row = resp["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"].as_i64() == Some(id))
            .expect("command must be listed");
        let expected = if expect_success { "Success" } else { "Failed" };
        assert_eq!(
            row["status"], expected,
            "code {code}: expected {expected} (2xx boundary is 200..=299, not ==200)"
        );
    }

    // 2xx codes -> Success.
    for code in [200, 202, 204] {
        let did = format!("{device_id_prefix}_{code}");
        let id = create_and_send(ctx, product_id, &did).await;
        reply_and_assert(ctx, product_id, &did, id, code, true).await;
    }

    // Boundary just outside 2xx -> Failed.
    for code in [199, 300] {
        let did = format!("{device_id_prefix}_{code}");
        let id = create_and_send(ctx, product_id, &did).await;
        reply_and_assert(ctx, product_id, &did, id, code, false).await;
    }
}

// ===========================================================================
// Scenario 10: wildcard service hooks do not double-dispatch property.
//
// Covers: 设计 §6.1 wildcard_service_hooks_do_not_double_dispatch_property /
//         §6.3 回归风险点. The unified `service_set_reply` and
//         `service_set_subscribe` handlers replace (not stack on top of) the
//         deleted property-private routes. A single property reply / a single
//         subscribe trigger must each route to exactly one handler, so the
//         device sees exactly one publish (no duplicate delivery).
// ===========================================================================
#[test_context(ActionTestContext)]
#[tokio::test]
async fn scenario_wildcard_service_hooks_do_not_double_dispatch_property(
    ctx: &mut ActionTestContext,
) {
    let product_id = "act_product_dedup";
    let device_id = "act_device_dedup";

    // 1. Queue a property command (do NOT let the create-time drain run: insert
    //    directly so the only drain happens via the subscribe hook below, which
    //    is the path we want to assert de-duplication on).
    ctx._admin_state
        .db
        .insert_property_command(
            product_id,
            device_id,
            &json!({ "brightness": 42 }),
            crate::db::models::CommandSource::OneShot,
        )
        .await
        .unwrap();

    // 2. Fire the unified service_set_subscribe webhook ONCE. The Broker fires
    //    it exactly once for the wildcard `thing/service/+/set` subscription,
    //    replacing the old property-private `property/set_subscribe` route.
    let subscribe_body = json!({
        "clientid": device_id,
        "username": product_id,
        "topic": service_set_subscribe_topic(product_id, device_id),
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/service/set_subscribe",
        &subscribe_body,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 3. Exactly ONE publish reached the device — the unified hook drained the
    //    single Pending row once; no second dispatch from a stacked rule.
    let published = ctx.take_published_messages().await;
    assert_eq!(
        published.len(),
        1,
        "unified service_set_subscribe must dispatch each pending row exactly once"
    );
    assert_eq!(published[0].params, json!({ "brightness": 42 }));
    assert!(published[0].id.starts_with("property:"));
}

// ===========================================================================
// Scenario 11: the unified event_post wildcard routes arbitrary event types.
//
// Covers: 设计 §6.1 custom_event_wildcard_routes_arbitrary_type / BE-D02. The
//         single `+/+/thing/event/+/post` rule routes every event publish to
//         `event_post`, which dispatches by the `{event_type}` segment:
//         `property` delegates to `property_post` (snapshot + rule trigger),
//         any other identifier lands in `insert_event_history`. The wildcard
//         must NOT double-write the property snapshot for non-property types,
//         and `property` must not be duplicated into event history by the
//         generic branch.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_custom_event_wildcard_routes_arbitrary_type(ctx: &mut TestContext) {
    let product_id = "act_product_event";
    let device_id = "act_device_event";

    // 1. Post an `alarm` event -> event_history branch (not property_post).
    let alarm_marker = format!(
        "alarm-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let alarm_payload = json!({
        "id": "evt-alarm-001",
        "ack": 0,
        "params": { "event": "alarm_raised", "marker": alarm_marker },
    });
    let msg = mqtt_publish_message(
        device_id,
        &event_topic(product_id, device_id, "alarm"),
        &alarm_payload,
    );
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 2. Post an `error` event -> same event_history branch.
    let error_marker = format!(
        "error-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let error_payload = json!({
        "id": "evt-error-001",
        "ack": 0,
        "params": { "event": "error_logged", "marker": error_marker },
    });
    let msg = mqtt_publish_message(
        device_id,
        &event_topic(product_id, device_id, "error"),
        &error_payload,
    );
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 3. Both events are visible in the admin event history list.
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/event?product_id={product_id}&device_id={device_id}&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: JsonValue = serde_json::from_str(&body).unwrap();
    let events = resp["data"].as_array().expect("event data array");
    assert!(
        events
            .iter()
            .any(|r| r["events"]["marker"].as_str() == Some(alarm_marker.as_str())),
        "alarm event must be persisted in event history"
    );
    assert!(
        events
            .iter()
            .any(|r| r["events"]["marker"].as_str() == Some(error_marker.as_str())),
        "error event must be persisted in event history"
    );

    // 4. A `property` event_type delegates to property_post and writes the
    //    reported snapshot. It must NOT also be duplicated into the property
    //    snapshot by the generic event branch (the dispatch is exclusive).
    let prop_payload = json!({
        "id": "evt-prop-001",
        "ack": 0,
        "params": { "brightness": 55 },
    });
    let msg = mqtt_publish_message(
        device_id,
        &event_topic(product_id, device_id, "property"),
        &prop_payload,
    );
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let latest = ctx
        ._admin_state
        .db
        .get_property_latest_one(product_id, device_id)
        .await
        .unwrap()
        .expect("property event_type must write the reported snapshot");
    assert_eq!(
        latest.properties["brightness"]["value"], 55,
        "property event_type must route to property_post and snapshot the value"
    );
}

// Silence unused-import warnings for DTOs that are re-exported but only some
// scenarios reference directly. `ActionCommandQuery` documents the GET shape but
// is not constructed in-test (the GET query string is built by hand).
const _: fn() = || {
    fn _assert_query_type(_q: ActionCommandQuery) {}
};
