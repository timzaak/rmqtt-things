//! Scenario tests for the thing-model-extension spec-deviation fixes.
//!
//! Covers (spec 修正):
//! - OTA device-side report now accepts a spec `"major.minor.patch"` semver
//!   **string** (`OtaReport.version: String`), and the OTA upgrade push carries
//!   the S3 **object key** under `file_url` (NOT a presigned URL) plus the
//!   mandatory `ack: 0` (no OTA upgrade reply topic in spec).
//! - Ack gating: property post / event post / file upload / factory-metadata
//!   requests with `ack: 0` MUST NOT trigger a `_reply` publish; only
//!   `ack: 1` does.
//! - Response `data` contract: success-with-data → object, error → `{message}`
//!   object (NOT a bare string), no-data → `data` field omitted entirely
//!   (`#[serde(skip_serializing_if = "Option::is_none")]`, NOT `"data": null`).
//! - Property report must be accepted when the product has no Active thing
//!   schema template (无 Active 模板时放行).
//!
//! Test style mirrors `action_invocation_scenarios.rs` and
//! `factory_metadata_scenarios.rs`:
//! - Default `#[test_context(TestContext)]` scenarios reuse the unreachable
//!   rmqtt URL (publishes fail silently); assertions rest on HTTP status + DB.
//! - Scenarios that must observe the device-side published envelope or count
//!   publishes use `#[test_context(SpecDeviationTestContext)]`, which reroutes
//!   `config.mqtt.url` at a mockito server capturing every `POST /mqtt/publish`
//!   body into an append-only `Arc<Mutex<Vec<JsonValue>>>`.
//!
//! Business rules encoded:
//! - G7 OTA: semver string in, object key + `ack:0` out.
//! - G7 ack gating: `ack:0` → no `_reply`.
//! - G7 object data: error = `{message}`, no-data = field omitted.
//! - G7 no-template: property accepted without an Active schema.

use super::simple_tests::TestContext;
use super::simple_tests::{create_test_database, drop_test_schema, request_json, test_s3_endpoint};
use crate::api::admin_models::CreateOtaVersionRequest;
use crate::api::handlers::{AppState, S3Client};
use crate::api::web_models::RMqttPublishMessage;
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{Config, MqttConfig, MqttPublishConfig, MqttResponseConfig, S3Config};
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

// ===========================================================================
// shared helpers
// ===========================================================================

fn encode_payload(value: &JsonValue) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

fn mqtt_publish_message(client_id: &str, topic: &str, payload: &JsonValue) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

fn ota_version_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/ota/version")
}

fn property_post_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/property/post")
}

fn event_post_topic(product_id: &str, device_id: &str, event_type: &str) -> String {
    format!("/{product_id}/{device_id}/thing/event/{event_type}/post")
}

fn file_upload_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/file/upload")
}

fn factory_metadata_get_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/factory-metadata/get")
}

/// Build a CreateOtaVersionRequest that targets `min_version <= 1.2.3 <
/// max_version`, so a device reporting `"1.2.3"` is eligible for the upgrade
/// push. `file_key` is pinned so the scenario can assert it round-trips into
/// the upgrade payload's `file_url` verbatim (object-key contract).
fn ota_version_request(product_id: &str, key: &str, file_key: &str) -> CreateOtaVersionRequest {
    CreateOtaVersionRequest {
        product_id: product_id.to_string(),
        key: key.to_string(),
        // Admin-side `parse_version_to_int` shares the handler-side packing, so
        // `1.3.0` decodes to a packed int strictly greater than `1.2.3`. The
        // device reports `1.2.3`, which the OTA matcher accepts as eligible.
        version: "1.3.0".to_string(),
        min_version: "0.9.0".to_string(),
        max_version: Some("1.3.0".to_string()),
        file_key: file_key.to_string(),
        log: Some(json!({ "release_notes": "spec-deviation fixture" })),
        device_ids: None,
        bin_length: 12345,
        bin_md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
    }
}

// ===========================================================================
// SpecDeviationTestContext — mockito-backed rmqtt for publish-capture /
// publish-count scenarios.
//
// Mirrors `factory_metadata_scenarios.rs::FactoryWebhookContext` and
// `action_invocation_scenarios.rs::ActionTestContext`, but captures EVERY
// `POST /mqtt/publish` body into an append-only Vec. The ack-gating scenario
// needs the count; the OTA + response-data scenarios need the captured body.
// No `/subscriptions` mock is registered because none of these scenarios drain
// pending property/action commands (the OTA push goes directly through
// `publish_command`, and the ack-gated replies go through `publish_response`).
// ===========================================================================

struct SpecDeviationTestContext {
    service: Router,
    /// Every captured `POST /mqtt/publish` outer body, in arrival order.
    captured: Arc<Mutex<Vec<JsonValue>>>,
    _admin_pool: PgPool,
    schema_name: String,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _mock_server: mockito::ServerGuard,
    _temp_dir: TempDir,
}

impl SpecDeviationTestContext {
    /// Drain and return all captured publish bodies, clearing the buffer.
    async fn take_published(&self) -> Vec<JsonValue> {
        self.captured.lock().await.drain(..).collect()
    }

    /// Current capture count without draining.
    async fn published_count(&self) -> usize {
        self.captured.lock().await.len()
    }
}

impl AsyncTestContext for SpecDeviationTestContext {
    async fn setup() -> SpecDeviationTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        // mockito standing in for the RMQTT HTTP API. `POST /mqtt/publish`
        // answers 200 and appends the outer request JSON to the capture Vec.
        // Scenarios drain the buffer via `take_published()` or read the count
        // via `published_count()`.
        let mut server = mockito::Server::new_async().await;
        let captured: Arc<Mutex<Vec<JsonValue>>> = Arc::new(Mutex::new(Vec::new()));

        let captured_for_publish = captured.clone();
        server
            .mock("POST", "/mqtt/publish")
            .with_status(200)
            .with_body("")
            .with_body_from_request(move |req| {
                let body = req.body().map(|b| b.as_slice()).unwrap_or(&[]);
                let outer: JsonValue = serde_json::from_slice(body).unwrap_or(JsonValue::Null);
                // try_lock: the mockito callback is sync + Send+Sync+'static,
                // so it cannot await the async Mutex. On contention we drop the
                // capture rather than block the mock thread; scenarios that
                // rely on exact counts use a fresh context per test so
                // contention is absent.
                if let Ok(mut guard) = captured_for_publish.try_lock() {
                    guard.push(outer);
                }
                Vec::new()
            })
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
        let mut config = Config {
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
                ..Config::default().mqtt
            },
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let s3_client = config.s3.as_ref().map(|s| S3Client::new(s).unwrap());

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

        SpecDeviationTestContext {
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
// Scenario 1: OTA accepts a semver string and the upgrade push carries the
// S3 object key (not a presigned URL) plus `ack: 0`.
//
// Covers: spec 修正 — OTA 上报+下发. Asserts the contract
//         end-to-end: device-side `OtaReport.version` arrives as a
//         `"major.minor.patch"` string, is parsed via
//         `parse_semver_to_int`, and the resulting upgrade publish carries
//         `file_url == ota_versions.file_key` (object key) and `ack: 0`
//         (spec defines no OTA upgrade reply topic).
// ===========================================================================
/// Covers: spec 修正 — OTA 上报 semver + 下发 object key.
#[test_context(SpecDeviationTestContext)]
#[tokio::test]
async fn scenario_ota_accepts_semver_and_emits_object_key(ctx: &mut SpecDeviationTestContext) {
    let product_id = "spec_dev_ota_prod";
    let device_id = "spec_dev_ota_dev";
    let ota_key = "firmware";
    let file_key = "firmware/v1.bin";

    // 1. Admin creates an OTA version eligible for devices reporting "1.2.3".
    //    (min 0.9.0 <= 1.2.3 < max 1.3.0; status active by default.)
    let create_req = ota_version_request(product_id, ota_key, file_key);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/ota/version",
        &create_req,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "OTA version create must succeed"
    );

    // 2. Device reports its current version as a spec semver STRING ("1.2.3").
    //    ack: 0 so no `/ota/version_reply` is expected; the OTA matcher inside
    //    the handler still produces the upgrade push because the device's
    //    reported version is below the published 1.3.0.
    let report_payload = json!({
        "id": "ota-report-001",
        "ack": 0,
        "params": [
            { "key": ota_key, "version": "1.2.3" }
        ],
    });
    let msg = mqtt_publish_message(
        device_id,
        &ota_version_topic(product_id, device_id),
        &report_payload,
    );
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/ota/version", &msg).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "OTA report with a valid semver string must be accepted (204)"
    );

    // 3. Exactly one publish reached the mock RMQTT — the OTA upgrade push. No
    //    `_reply` is sent because the report carried ack: 0.
    let published = ctx.take_published().await;
    assert_eq!(
        published.len(),
        1,
        "exactly one OTA upgrade publish (no ack reply for ack:0)"
    );

    let outer = &published[0];
    let topic = outer
        .get("topic")
        .and_then(|v| v.as_str())
        .expect("publish body must carry a topic");
    assert_eq!(
        topic,
        format!("/{product_id}/{device_id}/ota/upgrade"),
        "upgrade push targets the OTA upgrade topic"
    );

    // 4. The `payload` is a STRINGIFIED JSON object. Re-parse and assert:
    //    - `file_url` is the S3 **object key** (NOT a presigned URL).
    //    - `ack` is 0 (spec defines no OTA upgrade reply topic).
    //    - `version` / `key` round-trip from the published OTA version.
    let payload_str = outer
        .get("payload")
        .and_then(|v| v.as_str())
        .expect("publish payload must be a string");
    let payload: JsonValue =
        serde_json::from_str(payload_str).expect("publish payload string must parse as JSON");

    assert_eq!(
        payload["ack"], 0,
        "OTA upgrade payload must carry ack:0 (no reply topic in spec)"
    );
    assert_eq!(payload["id"], "ota-report-001", "id echoes the report id");

    let params = payload["params"]
        .as_array()
        .expect("upgrade payload params must be an array");
    assert_eq!(params.len(), 1, "one OTA key reported -> one upgrade entry");
    let entry = &params[0];
    assert_eq!(entry["key"], ota_key, "key round-trips");
    assert_eq!(
        entry["version"], "1.3.0",
        "version round-trips the admin-side published semver string"
    );
    // Core object-key contract: `file_url` is the S3 object key verbatim, NOT a
    // presigned URL. A presigned URL would contain "?" / signature
    // params / the S3 endpoint host; the object key is the bare path.
    assert_eq!(
        entry["file_url"], file_key,
        "file_url must be the S3 object key (firmware/v1.bin), not a presigned URL"
    );
    let file_url = entry["file_url"]
        .as_str()
        .expect("file_url must be a string");
    assert!(
        !file_url.starts_with("http"),
        "object-key contract: file_url must NOT be a URL"
    );
    assert!(
        !file_url.contains('?'),
        "object-key contract: file_url must NOT carry query/signature params"
    );
}

// ===========================================================================
// Scenario 2: ack: 0 requests never publish a `_reply`.
//
// Covers: spec 修正 — ack=0 不得下发 reply. For each request-type handler that
//         gates its `_reply` on `ack == AckStatus::Yes`
//         (property_post/event_post, file_upload, factory-metadata, ota), an
//         ack:0 request must NOT increase the publish count; an ack:1 request
//         for the same shape MUST increase it by exactly one.
//
// The core assertion is **publish count does not
// increase** after each ack:0 request.
// ===========================================================================
/// Covers: spec 修正 — ack=0 不得下发 reply.
#[test_context(SpecDeviationTestContext)]
#[tokio::test]
async fn scenario_ack_zero_never_publishes_reply(ctx: &mut SpecDeviationTestContext) {
    let product_id = "spec_dev_ack_prod";
    let device_id = "spec_dev_ack_dev";

    // Helper: assert the publish count does not increase after sending an
    // ack:0 request of the given kind.
    async fn assert_no_reply_on_ack_zero(
        ctx: &SpecDeviationTestContext,
        label: &str,
        topic: &str,
        params: JsonValue,
    ) {
        let before = ctx.published_count().await;
        let payload = json!({ "id": format!("ack0-{label}"), "ack": 0, "params": params });
        let msg = mqtt_publish_message("spec_dev_ack_dev", topic, &payload);
        let (status, _) =
            request_json(&ctx.service, Method::POST, route_for_topic(topic), &msg).await;
        // Give the (best-effort, fire-and-forget) publish path a moment so a
        // buggy ack gate would actually be observable. The handlers call
        // `publish_response`/`publish_command` synchronously before returning,
        // so by the time the HTTP response lands the mock has already captured
        // any publish a broken gate would have emitted.
        assert!(
            status.is_success(),
            "{label}: ack:0 request should still succeed at the HTTP layer (got {status})"
        );
        let after = ctx.published_count().await;
        assert_eq!(
            after, before,
            "{label}: ack:0 must NOT publish a _reply (count went {before} -> {after})"
        );
    }

    // Property post (via unified event_post -> property_post delegate).
    assert_no_reply_on_ack_zero(
        ctx,
        "property",
        &property_post_topic(product_id, device_id),
        json!({ "temperature": 21.0 }),
    )
    .await;

    // Custom event post (event_history branch).
    assert_no_reply_on_ack_zero(
        ctx,
        "event",
        &event_post_topic(product_id, device_id, "alarm"),
        json!({ "event": "raised" }),
    )
    .await;

    // File upload (success branch: S3 client is Some in this context, so it
    // produces a presigned post and would publish a `_reply` if the gate were
    // broken).
    assert_no_reply_on_ack_zero(
        ctx,
        "file",
        &file_upload_topic(product_id, device_id),
        json!({
            "fileName": "f.txt",
            "directory": "/",
            "useOriginName": true
        }),
    )
    .await;

    // Factory-metadata get (no device data, but the handler would still publish
    // a `data: omitted` 200 reply if the gate were broken).
    assert_no_reply_on_ack_zero(
        ctx,
        "factory",
        &factory_metadata_get_topic(product_id, device_id),
        json!({}),
    )
    .await;

    // Contrast: an ack:1 factory-metadata request MUST publish exactly one
    // `_reply`. This guards against the symmetric failure (gate always off).
    let before = ctx.published_count().await;
    let payload = json!({ "id": "ack1-factory", "ack": 1, "params": {} });
    let msg = mqtt_publish_message(
        device_id,
        &factory_metadata_get_topic(product_id, device_id),
        &payload,
    );
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/factory-metadata/get",
        &msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let after = ctx.published_count().await;
    assert_eq!(
        after,
        before + 1,
        "ack:1 factory-metadata MUST publish exactly one _reply (count went {before} -> {after})"
    );
}

/// Map a publish topic to the webhook route that handles it. Used by the
/// ack-zero helper so each request kind is dispatched to the correct handler
/// (property/event share the unified `/api/thing/event/post`; file and
/// factory-metadata have dedicated routes).
fn route_for_topic(topic: &str) -> &'static str {
    if topic.contains("/thing/event/property/post") || topic.contains("/thing/event/") {
        "/api/thing/event/post"
    } else if topic.contains("/thing/file/upload") {
        "/api/thing/file/upload"
    } else if topic.contains("/thing/factory-metadata/get") {
        "/api/thing/factory-metadata/get"
    } else {
        "/api/thing/event/post"
    }
}

// ===========================================================================
// Scenario 3: response `data` is always an object or absent — never a string
// and never `null`.
//
// Covers: spec 修正 — response data 为 object 或缺省. Asserts the three
//         branches of the contract:
//         - success-with-data → `data` is an object (factory-metadata with
//           device data).
//         - error → `data` is `{"message": ...}` object (file upload when S3
//           is not configured). Asserted via a bespoke no-S3 context.
//         - no-data → `data` field is OMITTED (factory-metadata with no device
//           data), guaranteed by `#[serde(skip_serializing_if =
//           "Option::is_none")]` on `MqttResponse.data`.
// ===========================================================================
/// Covers: spec 修正 — response data 为 object 或缺省.
#[test_context(SpecDeviationTestContext)]
#[tokio::test]
async fn scenario_response_data_is_object_or_absent(ctx: &mut SpecDeviationTestContext) {
    let product_id = "spec_dev_data_prod";
    let device_id_with_data = "spec_dev_data_with";
    let device_id_no_data = "spec_dev_data_none";

    // --- Branch A: success-with-data → data is an object (not string/null). ---
    //
    // Seed device-level factory metadata so the device-pull view is non-null,
    // then assert the reply's `data` is a JSON object. The factory writer path
    // requires a factory API key; spin up a one-off router with the test key
    // layered in. We reuse the same DB schema by going through `ctx.service`
    // after rebuilding the router — but the simplest path is to insert the row
    // directly via the repo, which the handler then surfaces.

    ctx._app_state
        .db
        .factory_metadata()
        .upsert_device_metadata(
            device_id_with_data,
            &json!({ "qcReport": "PASS" }),
            &json!([]), // file_attachments: empty array (no attachments)
        )
        .await
        .expect("seed device metadata");

    let reply_a = publish_and_parse_reply(
        ctx,
        device_id_with_data,
        &factory_metadata_get_topic(product_id, device_id_with_data),
        "data-obj-req",
    )
    .await
    .expect("factory-metadata reply must be published for ack:1");
    assert_eq!(reply_a["code"], 200, "success branch code");
    let data_a = &reply_a["data"];
    assert!(
        data_a.is_object(),
        "success-with-data: data must be an object (got {data_a})"
    );
    assert!(
        data_a.get("deviceSn").is_some(),
        "merged factory view must carry deviceSn"
    );

    // --- Branch C: no-data → `data` field OMITTED entirely (not null). ----
    //
    // device_id_no_data has no factory metadata at all. The handler constructs
    // `MqttResponse { data: None, .. }`; with the later-added
    // `#[serde(skip_serializing_if = "Option::is_none")]`, the serialised JSON
    // must NOT contain a `data` key at all. The previous code shape would have
    // emitted `"data": null`.
    let reply_c = publish_and_parse_reply(
        ctx,
        device_id_no_data,
        &factory_metadata_get_topic(product_id, device_id_no_data),
        "data-none-req",
    )
    .await
    .expect("factory-metadata reply must be published for ack:1");
    assert_eq!(reply_c["code"], 200, "no-data branch code");
    assert!(
        reply_c.get("data").is_none(),
        "no-data: `data` key must be OMITTED (skip_serializing_if), got: {reply_c}"
    );
    // Sanity: the reply still carries `id` + `code`.
    assert_eq!(reply_c["id"], "data-none-req");

    // --- Branch B: error → `data` is `{"message": ...}` object (not a bare
    // string). ------------------------------------------------------------
    //
    // The error branch in `file_upload_handler` (S3 client = None) emits
    // `data: Some(json!({"message": "do not support file upload"}))`. The
    // default SpecDeviationTestContext wires S3 = Some, so this branch needs a
    // bespoke router with S3 disabled. We build it inline against the same
    // schema-prefixed pool so the OTA/factory seeding above is irrelevant.
    let no_s3 = NoS3PublishCapture::from_parent_pool(ctx).await;
    let file_payload = json!({
        "id": "file-err-req",
        "ack": 1,
        "params": {
            "fileName": "x.txt",
            "directory": "/",
            "useOriginName": true
        }
    });
    let msg = mqtt_publish_message(
        "spec_dev_file_dev",
        &file_upload_topic("spec_dev_file_prod", "spec_dev_file_dev"),
        &file_payload,
    );
    let (status, _) =
        request_json(&no_s3.service, Method::POST, "/api/thing/file/upload", &msg).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "file upload error branch still returns 204 (reply is via MQTT)"
    );

    let captured = no_s3.take_published().await;
    let outer = captured
        .first()
        .expect("file upload error branch must publish a _reply for ack:1");
    let payload_str = outer
        .get("payload")
        .and_then(|v| v.as_str())
        .expect("publish payload must be a string");
    let reply_b: JsonValue = serde_json::from_str(payload_str)
        .expect("publish payload string must parse as MqttResponse JSON");
    assert_eq!(reply_b["code"], 503, "error branch code");
    let data_b = &reply_b["data"];
    assert!(
        data_b.is_object(),
        "error: data must be an object (`{{message}}`), got {data_b}"
    );
    assert!(
        data_b.get("message").is_some(),
        "error: data must carry a `message` field"
    );
    assert!(
        data_b["message"].is_string(),
        "error: data.message must be a string"
    );
    // Negative: data must NOT be a bare JSON string (the old shape was
    // `json!("do not support file upload")`).
    assert!(
        !data_b.is_string(),
        "error: data must NOT be a bare string (object contract)"
    );
}

/// Drive a factory-metadata get with ack:1 and parse the captured `_reply`
/// payload into a `JsonValue`. Returns None when no reply was published (which
/// itself is a test failure at the call site).
async fn publish_and_parse_reply(
    ctx: &SpecDeviationTestContext,
    device_id: &str,
    topic: &str,
    request_id: &str,
) -> Option<JsonValue> {
    let payload = json!({ "id": request_id, "ack": 1, "params": {} });
    let msg = mqtt_publish_message(device_id, topic, &payload);
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/factory-metadata/get",
        &msg,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let captured = ctx.take_published().await;
    let outer = captured.first()?;
    let payload_str = outer.get("payload")?.as_str()?;
    Some(serde_json::from_str(payload_str).expect("payload must parse as MqttResponse JSON"))
}

// ---------------------------------------------------------------------------
// NoS3PublishCapture: a second mockito-backed router built against the SAME
// underlying test schema (re-uses `SpecDeviationTestContext`'s DB pool) but
// with `config.s3 = None`. Used to reach the file-upload 503 error branch
// (the parent context wires S3 = Some, so the success branch would fire).
//
// This is a minimal helper struct (not an AsyncTestContext) because it shares
// the parent's schema lifecycle: teardown is the parent's responsibility.
// ---------------------------------------------------------------------------

struct NoS3PublishCapture {
    service: Router,
    captured: Arc<Mutex<Vec<JsonValue>>>,
    // Keep the mockito server guard alive for the helper's lifetime; if it is
    // dropped (as a bare local in `from_parent_pool`) the HTTP server shuts
    // down and `publish_response` then hits a dead/reused port (observed as a
    // 501), so the `_reply` is never captured. Mirrors the parent
    // `SpecDeviationTestContext._mock_server` field.
    _mock_server: mockito::ServerGuard,
}

impl NoS3PublishCapture {
    async fn take_published(&self) -> Vec<JsonValue> {
        self.captured.lock().await.drain(..).collect()
    }

    /// Build a fresh router against a brand-new test schema (the parent's pool
    /// cannot be borrowed from inside the running test to spin a sibling
    /// router with different config without re-running migrations, so we create
    /// an isolated schema). The parent context is only used to inherit the
    /// mockito/S3 endpoint defaults.
    async fn from_parent_pool(_parent: &SpecDeviationTestContext) -> Self {
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        let mut server = mockito::Server::new_async().await;
        let captured: Arc<Mutex<Vec<JsonValue>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_publish = captured.clone();
        server
            .mock("POST", "/mqtt/publish")
            .with_status(200)
            .with_body("")
            .with_body_from_request(move |req| {
                let body = req.body().map(|b| b.as_slice()).unwrap_or(&[]);
                let outer: JsonValue = serde_json::from_slice(body).unwrap_or(JsonValue::Null);
                if let Ok(mut guard) = captured_for_publish.try_lock() {
                    guard.push(outer);
                }
                Vec::new()
            })
            .create_async()
            .await;

        let temp_dir = tempdir().unwrap();
        let mut config = Config {
            s3: None, // KEY: disable S3 so file_upload_handler hits the 503 branch.
            mqtt: MqttConfig {
                url: server.url(),
                publish: MqttPublishConfig {
                    response: MqttResponseConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                    },
                },
                ..Config::default().mqtt
            },
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let app_state = Arc::new(AppState {
            db: db_service.clone(),
            rmqtt_client: rmqtt_client.clone(),
            config: config.clone(),
            cache: schema_cache.clone(),
            s3_client: None,
        });
        let admin_state = Arc::new(AdminAppState {
            db: db_service,
            rmqtt_client,
            config: config.clone(),
            cache: schema_cache,
            s3_client: None,
            rule_cache: crate::rule_engine::RuleCache::new_in_memory(),
            task_set: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
        });
        let router = create_router(
            config,
            app_state,
            admin_state,
            None,
            crate::api::tests::simple_tests::empty_factory_auth_state(),
        );

        // Drop the schema once the helper goes out of scope. Kept simple: spawn
        // the cleanup on a background task so the helper can be dropped without
        // awaiting inside the test body. The admin_pool handle is moved into
        // the task; if the test process exits first the schema is left behind
        // (acceptable for an isolated helper, mirrors the per-test isolation
        // already provided by the parent context).
        let _schema_name = schema_name;
        let pool_for_cleanup = admin_pool;
        tokio::spawn(async move {
            let _ = sqlx::query(&format!(
                r#"DROP SCHEMA IF EXISTS "{_schema_name}" CASCADE"#
            ))
            .execute(&pool_for_cleanup)
            .await;
            // Suppress a misleading warning if the connection errors out
            // before cleanup completes.
            drop(_schema_name);
        });

        Self {
            service: router,
            captured,
            _mock_server: server,
        }
    }
}

// ===========================================================================
// Scenario 4: property report is accepted when the product has no Active
// schema template.
//
// Covers: spec 修正 — 无 Active 模板的属性上报放行.
//
// **DEGRADATION NOTE**: The design intent is that with the
// `property_schema_validator` master switch ON and the product having NO
// Active schema template, property post is放行 (accepted, 204). The current
// production `property_post` (handlers.rs) still returns 400 "Schema not
// found" in that exact case — the property_post relaxation was not part of
// that fix, so the gap is still open. Asserting 204 with validator-on +
// no-template would
// therefore fail against the current production code.
//
// Per the item's documented degradation, this scenario asserts the half of
// the contract that IS implemented: with the master switch OFF (default
// `TestContext`), a property post for a product with no template is accepted
// (204). The validator-ON + no-template case is left to the accept slot to
// cover once the production gap is closed (tracked in Handoff).
// ===========================================================================
/// Covers: spec 修正 — 无 Active 模板的属性上报放行.
///
/// Degraded: asserts the validator-OFF + no-template half. The validator-ON +
/// no-template half is pending a production-code change in `property_post`
/// (currently returns 400 "Schema not found"); see the scenario doc-comment
/// above.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_property_without_active_schema_is_accepted(ctx: &mut TestContext) {
    let product_id = "spec_dev_noschema_prod";
    let device_id = "spec_dev_noschema_dev";

    // The default TestContext has `api.property_schema_validator = false` and
    // the product has NO Active thing-schema template. The contract says
    // property post must be放行 (accepted) in this state.
    let property_data = json!({
        "temperature": 25.5,
        "humidity": 60.0
    });
    let payload = json!({
        "id": "prop-no-schema-001",
        "ack": 0,
        "params": property_data,
    });
    let msg = mqtt_publish_message(
        device_id,
        &property_post_topic(product_id, device_id),
        &payload,
    );
    let (status, _) = request_json(&ctx.service, Method::POST, "/api/thing/event/post", &msg).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "property post with no Active schema template must be accepted (204), not 400"
    );

    // The reported snapshot is persisted (proves the handler ran the full
    // property_post side-effects, not just a short-circuit 204).
    let latest = ctx
        ._admin_state
        .db
        .get_property_latest_one(product_id, device_id)
        .await
        .expect("db read ok")
        .expect("property snapshot must be persisted");
    assert_eq!(
        latest.properties["temperature"]["value"], 25.5,
        "reported temperature must be snapshotted"
    );
    assert_eq!(
        latest.properties["humidity"]["value"], 60.0,
        "reported humidity must be snapshotted"
    );
}

// ===========================================================================
// Scenario 5: event_post rejects non-object params.
//
// Covers: thing-model-spec.md §消息格式 ("事件上报 ... 必须是 object"). The
// unified event_post webhook must reject any `params` that is not a JSON
// object (array, scalar, null, omitted) with 400 "Invalid params format",
// matching property_post's existing guard. The `property` event_type branch
// delegates to property_post and is already guarded, so this targets a
// custom event_type (`alarm`).
// ===========================================================================
#[test_context(SpecDeviationTestContext)]
#[tokio::test]
async fn scenario_event_post_rejects_non_object_params(ctx: &mut SpecDeviationTestContext) {
    let product_id = "spec_dev_evt_prod";
    let device_id = "spec_dev_evt_dev";
    let topic = event_post_topic(product_id, device_id, "alarm");

    for (label, params) in [
        ("array", json!([{"event": "raised"}])),
        ("scalar", json!(42)),
        ("string", json!("raised")),
        ("null", JsonValue::Null),
    ] {
        // `null` is what `params` deserializes to when omitted, so the omitted
        // case is covered by the `null` variant without a separate request.
        let payload = json!({ "id": format!("evt-{label}"), "ack": 0, "params": params });
        let msg = mqtt_publish_message(device_id, &topic, &payload);
        let (status, _) =
            request_json(&ctx.service, Method::POST, route_for_topic(&topic), &msg).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "event_post with {label} params should be rejected (400), got {status}"
        );
    }

    // Sanity: an object params on the same topic is accepted (204), proving the
    // 400s above come from the object check, not some other failure.
    let payload = json!({ "id": "evt-ok", "ack": 0, "params": { "event": "raised" } });
    let msg = mqtt_publish_message(device_id, &topic, &payload);
    let (status, _) = request_json(&ctx.service, Method::POST, route_for_topic(&topic), &msg).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "event_post with object params should be accepted (204)"
    );
}
