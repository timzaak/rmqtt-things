//! Scenario tests for the factory metadata feature (support-multiple-device).
//!
//! Covers the 8 business scenarios from design §6.1 + the ACL regression risk
//! point from §6.3, all built on top of the dev-delivered production code:
//! - BE-D01: `FactoryMetadataRepo` + 4-table migration.
//! - BE-D02: `factory_auth_middleware` + factory write handlers + factory DTOs.
//! - BE-D03: admin read handlers + device pull webhook + ACL allow-list
//!   extension for `thing/factory-metadata`.
//!
//! Test style mirrors `shadow_scenarios.rs` / `mqtt_device_flow_scenarios.rs`:
//! in-process axum `#[test_context(Ctx)]` + `#[tokio::test]`, reusing
//! `super::simple_tests::{request, request_json, request_json_with_headers,
//! TestContext}`. HTTP calls go through `ctx.service`; direct DB assertions go
//! through `ctx._admin_state.db.factory_metadata()` (the public repo factory).
//!
//! Path-parameter naming: the actual routes registered in `api/mod.rs` use
//! axum 0.8 `{device_sn}` / `{component_sn}` (snake_case placeholders). The
//! request BODY field names follow the DTOs and use camelCase
//! (`componentType` / `metadata` / `fileAttachments` / `components[].componentSn`).
//!
//! Test function names carry the `scenario_factory_` prefix so the runner
//! (BE-TR01) can target them with the nextest expression
//! `test(~scenario_factory_)`.

use super::simple_tests::TestContext;
use super::simple_tests::{
    create_test_database, drop_test_schema, request, request_json, request_json_with_headers,
    test_s3_endpoint,
};
use crate::api::auth_handlers::{Access, AclPayload, acl};
use crate::api::factory_middleware::FactoryAuthState;
use crate::api::handlers::{AppState, S3Client};
use crate::api::web_models::RMqttPublishMessage;
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{Config, FactoryConfig, S3Config};
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

// --- shared helpers (mirror mqtt_device_flow_scenarios.rs) ---

fn encode_payload(value: &JsonValue) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

fn factory_metadata_get_topic(product_id: &str, device_id: &str) -> String {
    format!("/{product_id}/{device_id}/thing/factory-metadata/get")
}

fn mqtt_publish_message(client_id: &str, topic: &str, payload: &JsonValue) -> RMqttPublishMessage {
    RMqttPublishMessage {
        client_id: client_id.to_string(),
        topic: topic.to_string(),
        payload: encode_payload(payload),
        ..Default::default()
    }
}

/// Bearer header value for the configured factory API key.
const FACTORY_BEARER_HEADERS: &[(&str, &str)] = &[("Authorization", "Bearer test-key")];

/// Build the factory API-key auth state used by the writer-friendly test
/// contexts (`FactoryAuthTestContext`, `FactoryNoS3Context`). Matches
/// `simple_tests::empty_factory_auth_state` construction.
fn factory_auth_state_with_test_key() -> Arc<FactoryAuthState> {
    let keys: Arc<[Box<str>]> = vec!["test-key".to_string().into_boxed_str()].into();
    Arc::new(FactoryAuthState { api_keys: keys })
}

// ===========================================================================
// Writer-friendly contexts
//
// The default `simple_tests::TestContext` is constructed with
// `Config { s3: Some(..), ..Default::default() }` — `[factory] api_keys` is
// empty (every factory request 401) AND `s3_client` is `Some(..)` (the 503
// path is unreachable there). To cover the writer scenarios we need two
// bespoke contexts that mirror `MergeOrderTestContext` in
// `mqtt_device_flow_scenarios.rs` (custom config + rebuilt router):
// - `FactoryAuthTestContext`: `FactoryConfig { api_keys: ["test-key"] }` +
//   `s3: Some(..)` (mirrors the default S3 wiring). Covers valid-key/204,
//   change_log writes, association full-replace, left-join-with-null.
// - `FactoryNoS3Context`: `FactoryConfig { api_keys: ["test-key"] }` +
//   `s3: None`. Covers the file-upload 503 sub-case (s3_client = None).
// ===========================================================================

struct FactoryAuthTestContext {
    service: Router,
    _app_state: Arc<AppState>,
    admin_state: Arc<AdminAppState>,
    _admin_pool: PgPool,
    schema_name: String,
    _temp_dir: TempDir,
}

impl AsyncTestContext for FactoryAuthTestContext {
    async fn setup() -> FactoryAuthTestContext {
        let _ = tracing_subscriber::fmt().try_init();
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

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
            factory: FactoryConfig {
                api_keys: vec!["test-key".to_string()],
            },
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let s3_client = config
            .s3
            .as_ref()
            .map(|s3_config| S3Client::new(s3_config).unwrap());

        let app_state = Arc::new(AppState {
            db: db_service.clone(),
            rmqtt_client: rmqtt_client.clone(),
            config: config.clone(),
            cache: schema_cache.clone(),
            s3_client,
        });
        let admin_state = Arc::new(AdminAppState {
            db: db_service,
            rmqtt_client,
            config: config.clone(),
            cache: schema_cache,
            s3_client: app_state.s3_client.clone(),
            rule_cache: crate::rule_engine::RuleCache::new_in_memory(),
            task_set: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
        });

        let router = create_router(
            config,
            app_state.clone(),
            admin_state.clone(),
            None,
            factory_auth_state_with_test_key(),
        );

        FactoryAuthTestContext {
            service: router,
            _app_state: app_state,
            admin_state,
            _admin_pool: admin_pool,
            schema_name,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

struct FactoryNoS3Context {
    service: Router,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _admin_pool: PgPool,
    schema_name: String,
    _temp_dir: TempDir,
}

impl AsyncTestContext for FactoryNoS3Context {
    async fn setup() -> FactoryNoS3Context {
        let _ = tracing_subscriber::fmt().try_init();
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        let temp_dir = tempdir().unwrap();
        let mut config = Config {
            s3: None,
            factory: FactoryConfig {
                api_keys: vec!["test-key".to_string()],
            },
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        // s3_client is intentionally None: that's the whole point of this
        // context (file-upload 503 sub-case, design §4.2.2 A0).
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
            app_state.clone(),
            admin_state.clone(),
            None,
            factory_auth_state_with_test_key(),
        );

        FactoryNoS3Context {
            service: router,
            _app_state: app_state,
            _admin_state: admin_state,
            _admin_pool: admin_pool,
            schema_name,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

// ===========================================================================
// Mockito-backed context for the webhook (device-pull) scenario.
//
// Mirrors `mqtt_device_flow_scenarios.rs::MergeOrderTestContext`:
// - Reroutes `config.mqtt.url` at a mockito server.
// - Mocks `POST /mqtt/publish` to capture the request body into an
//   `Arc<Mutex<Option<JsonValue>>>`. Provides `take_published_body()` to drain
//   the latest captured publish.
// - Keeps the factory key configured so a writer sub-flow can pre-seed data
//   before the webhook is invoked (covers the left-join merged-view response).
// ===========================================================================

struct FactoryWebhookContext {
    service: Router,
    _admin_state: Arc<AdminAppState>,
    captured_publish_body: Arc<Mutex<Option<JsonValue>>>,
    _admin_pool: PgPool,
    schema_name: String,
    _app_state: Arc<AppState>,
    _mock_server: mockito::ServerGuard,
    _temp_dir: TempDir,
}

impl FactoryWebhookContext {
    /// Drain and return the most recently captured `POST /mqtt/publish` body
    /// (the raw outer request JSON). `None` until a publish lands.
    async fn take_published_body(&self) -> Option<JsonValue> {
        self.captured_publish_body.lock().await.take()
    }
}

impl AsyncTestContext for FactoryWebhookContext {
    async fn setup() -> FactoryWebhookContext {
        let _ = tracing_subscriber::fmt().try_init();

        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        let mut server = mockito::Server::new_async().await;
        let captured: Arc<Mutex<Option<JsonValue>>> = Arc::new(Mutex::new(None));

        let captured_for_publish = captured.clone();
        server
            .mock("POST", "/mqtt/publish")
            .with_status(200)
            .with_body("")
            .with_body_from_request(move |req| {
                // Capture the raw outer publish body. The handler serialises
                // the `MqttResponse` to a STRING and places it inside the
                // `payload` field of the outer PublishRequest. We keep the
                // outer JSON so the scenario can parse both `topic` and
                // `payload` (then re-parse payload-as-string) in one place.
                let body = req.body().map(|b| b.as_slice()).unwrap_or(&[]);
                let outer: JsonValue = serde_json::from_slice(body).unwrap_or(JsonValue::Null);
                // try_lock: the mockito capture callback is sync + Send+Sync
                // +'static, so it cannot await the async Mutex. Store only
                // the latest publish (single-publish scenario).
                if let Ok(mut guard) = captured_for_publish.try_lock() {
                    *guard = Some(outer);
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
        let mut config = Config {
            s3: Some(s3_config),
            factory: FactoryConfig {
                api_keys: vec!["test-key".to_string()],
            },
            mqtt: crate::config::MqttConfig {
                url: server.url(),
                ..Config::default().mqtt
            },
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let s3_client = config
            .s3
            .as_ref()
            .map(|s3_config| S3Client::new(s3_config).unwrap());

        let app_state = Arc::new(AppState {
            db: db_service.clone(),
            rmqtt_client: rmqtt_client.clone(),
            config: config.clone(),
            cache: schema_cache.clone(),
            s3_client,
        });
        let admin_state = Arc::new(AdminAppState {
            db: db_service,
            rmqtt_client,
            config: config.clone(),
            cache: schema_cache,
            s3_client: app_state.s3_client.clone(),
            rule_cache: crate::rule_engine::RuleCache::new_in_memory(),
            task_set: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
        });

        let router = create_router(
            config,
            app_state.clone(),
            admin_state.clone(),
            None,
            factory_auth_state_with_test_key(),
        );

        FactoryWebhookContext {
            service: router,
            _admin_state: admin_state,
            captured_publish_body: captured,
            _admin_pool: admin_pool,
            schema_name,
            _app_state: app_state,
            _mock_server: server,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

// ===========================================================================
// Scenario 1 — factory API key authentication (401 tri-state).
//
// User Story: US-PA-045 (sub-component metadata report).
// Covers: design §4.5 (factory API key auth), §6.1 scenario
//         `factory_auth_rejects_missing_or_invalid_api_key`, §4.2.2 A.
//
// Asserts the three 401 states on `PUT /api/factory/components/{component_sn}`
// against the default `TestContext` (empty `api_keys`):
//   1. Missing `Authorization` header → 401.
//   2. Wrong bearer (not a configured key) → 401.
//   3. Empty `api_keys` config (the default TestContext) → 401 even with a
//      syntactically valid bearer.
//
// The valid-key/204 happy path is covered by `scenario_factory_upsert_*`
// (scenario 3) and `scenario_factory_replace_associations_*` (scenario 4)
// which both run against `FactoryAuthTestContext` and assert 204 on the
// writer path.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_factory_auth_rejects_missing_or_invalid_api_key(ctx: &mut TestContext) {
    let component_sn = "sn_auth_default_ctx";
    let path = format!("/api/factory/components/{component_sn}");
    let body = json!({ "componentType": "camera", "metadata": { "v": 1 } });

    // (1) Missing `Authorization` header → 401.
    let (status_missing, _) = request_json(&ctx.service, Method::PUT, &path, &body).await;
    assert_eq!(
        status_missing,
        StatusCode::UNAUTHORIZED,
        "missing Authorization header must be 401"
    );

    // (2) Wrong bearer (not in the empty configured list) → 401.
    let (status_wrong, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &body,
        &[("Authorization", "Bearer not-a-configured-key")],
    )
    .await;
    assert_eq!(
        status_wrong,
        StatusCode::UNAUTHORIZED,
        "wrong bearer must be 401"
    );

    // (3) Empty `api_keys` configured (the default TestContext) — even a
    //     syntactically well-formed bearer is rejected because no key matches.
    //     (Same call shape as (2); documented separately for clarity.)
    let (status_empty_cfg, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &body,
        &[("Authorization", "Bearer test-key")],
    )
    .await;
    assert_eq!(
        status_empty_cfg,
        StatusCode::UNAUTHORIZED,
        "empty api_keys config must reject every bearer with 401"
    );
}

// ===========================================================================
// Scenario 2 — factory file upload: 401 (no/invalid key) + 503 (s3 unconfigured).
//
// User Story: US-PA-045 (file-attachment upload prerequisite).
// Covers: design §4.2.2 A0, §4.5.
//
// `POST /api/factory/file/upload` is gated by `factory_auth_middleware`
// (NOT Herald and NOT internal_ip). On `FactoryNoS3Context`:
//   - 401 when no `Authorization` header is sent (auth layer rejects before
//     the handler can reach the S3 branch). This proves the file-upload route
//     is mounted behind `factory_auth_middleware` exactly like the other
//     `/api/factory/*` routes — a Herald 403 or internal_ip rejection would
//     be a routing bug.
//   - 503 when a valid bearer IS sent but `s3_client` is None (auth passes,
//     handler surfaces the existing s3-not-configured semantics shared with
//     admin/thing upload paths).
//
// We intentionally do NOT cover the 200/presigned-POST path: the test S3
// endpoint is a fake that returns presigned fields without a real bucket, so
// observing a 200 would not validate any meaningful behaviour; the 401/503
// pair is the load-bearing contract.
// ===========================================================================
#[test_context(FactoryNoS3Context)]
#[tokio::test]
async fn scenario_factory_file_upload_requires_factory_api_key(ctx: &mut FactoryNoS3Context) {
    let body = json!({
        "fileName": "calib.bin",
        "directory": "factory-attachments",
        "useOriginName": true,
    });

    // 401 when no Authorization header is sent — factory_auth_middleware
    // rejects before the handler can run (NOT Herald 403 / internal_ip).
    let (status_no_header, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/factory/file/upload",
        &body,
    )
    .await;
    assert_eq!(
        status_no_header,
        StatusCode::UNAUTHORIZED,
        "file upload without Authorization must be 401, not Herald 403 / internal_ip reject"
    );

    // 401 when the bearer is wrong (not in the configured list) — auth layer.
    let (status_wrong_bearer, _) = request_json_with_headers(
        &ctx.service,
        Method::POST,
        "/api/factory/file/upload",
        &body,
        &[("Authorization", "Bearer not-a-configured-key")],
    )
    .await;
    assert_eq!(
        status_wrong_bearer,
        StatusCode::UNAUTHORIZED,
        "file upload with wrong bearer must be 401"
    );

    // 503 when the bearer is valid but s3_client is None — auth passes, the
    // handler then returns 503 to mirror admin/thing file-upload behaviour.
    let (status_no_s3, _) = request_json_with_headers(
        &ctx.service,
        Method::POST,
        "/api/factory/file/upload",
        &body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(
        status_no_s3,
        StatusCode::SERVICE_UNAVAILABLE,
        "file upload with valid key + s3_client=None must be 503, not 401"
    );
}

// ===========================================================================
// Scenario 3 — upsert then overwrite writes change_log (before=null on first,
// before=old-snapshot on overwrite, actor="factory").
//
// User Story: US-PA-045 (same-SN re-report is idempotent overwrite + log, R5).
// Covers: design §4.2.2 A, §5.1 (change_log before/after snapshots), §6.1
//         scenario `upsert_component_then_overwrite_writes_change_log`.
//
// Uses `FactoryAuthTestContext` because the default TestContext's empty
// api_keys would 401 the PUT and never reach the change_log path.
// ===========================================================================
#[test_context(FactoryAuthTestContext)]
#[tokio::test]
async fn scenario_factory_upsert_component_then_overwrite_writes_change_log(
    ctx: &mut FactoryAuthTestContext,
) {
    let component_sn = "sn_changelog_actor";
    let path = format!("/api/factory/components/{component_sn}");

    // First report: metadata.v = 1 → 204, no change_log row yet (Created).
    let first_body = json!({ "componentType": "camera", "metadata": { "v": 1 } });
    let (status_first, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &first_body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_first, StatusCode::NO_CONTENT);

    // Just-before-overwrite assertion: still zero change_log rows.
    let (_, total_before) = ctx
        .admin_state
        .db
        .factory_metadata()
        .query_change_log(component_sn, 1, 10)
        .await
        .unwrap();
    assert_eq!(
        total_before, 0,
        "first report is a Created; no change_log row yet (R5)"
    );

    // Second report (overwrite): metadata.v = 2 → 204, change_log gets a row.
    let second_body = json!({ "componentType": "camera", "metadata": { "v": 2 } });
    let (status_second, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &second_body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_second, StatusCode::NO_CONTENT);

    // Query change_log (time-descending): exactly one row, before = the v=1
    // snapshot, after = the v=2 snapshot, actor = "factory".
    let (rows, total_after) = ctx
        .admin_state
        .db
        .factory_metadata()
        .query_change_log(component_sn, 1, 10)
        .await
        .unwrap();
    assert_eq!(
        total_after, 1,
        "overwrite must write exactly one change_log row"
    );
    assert_eq!(rows.len(), 1);
    let log = &rows[0];
    assert_eq!(log.component_sn, component_sn);
    assert_eq!(log.actor, "factory", "R5: actor must be 'factory'");
    // before snapshot carries the first-report metadata (v=1).
    let before = log
        .before
        .as_ref()
        .expect("overwrite before-snapshot must be present");
    assert_eq!(
        before["metadata"]["v"], 1,
        "before snapshot must be the v=1 state"
    );
    // after snapshot carries the new metadata (v=2).
    assert_eq!(
        log.after["metadata"]["v"], 2,
        "after snapshot must be the v=2 state"
    );
}

// ===========================================================================
// Scenario 4 — association full-replace is idempotent; trimming deletes old.
//
// User Story: US-PA-046 (device↔component association upsert).
// Covers: design §4.2.2 B (full-replace semantics), §6.1 scenario
//         `replace_associations_full_replace_is_idempotent`.
//
// Uses `FactoryAuthTestContext`. Asserts:
//   - Same list PUT twice → second PUT introduces no DB diff (associations
//     remain exactly the listed set; no change_log — association writes never
//     log, R5 scopes the log to component-metadata overwrites only).
//   - Trimming the list (drop one component) → that association is deleted
//     (verified via the merged-view query: trimmed component no longer present).
// ===========================================================================
#[test_context(FactoryAuthTestContext)]
#[tokio::test]
async fn scenario_factory_replace_associations_full_replace_is_idempotent(
    ctx: &mut FactoryAuthTestContext,
) {
    let device_sn = "dev_assoc_idempotent";
    let path = format!("/api/factory/devices/{device_sn}/components");
    let comp_a = "comp_idem_a";
    let comp_b = "comp_idem_b";

    let full_list = json!({
        "components": [
            { "componentSn": comp_a, "componentType": "camera" },
            { "componentSn": comp_b, "componentType": "camera" },
        ]
    });

    // First PUT: both associations created.
    let (status_first, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &full_list,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_first, StatusCode::NO_CONTENT);

    let assert_associations = |sns: &[&str]| {
        let sns_vec: Vec<String> = sns.iter().map(|s| s.to_string()).collect();
        sns_vec
    };

    let view_after_first = ctx
        .admin_state
        .db
        .factory_metadata()
        .get_device_view(device_sn)
        .await
        .unwrap()
        .expect("after first PUT the device must have associations");
    let mut sns_after_first: Vec<String> = view_after_first
        .iter()
        .map(|r| r.component_sn.clone())
        .collect();
    sns_after_first.sort();
    assert_eq!(
        sns_after_first,
        assert_associations(&[comp_a, comp_b]),
        "first PUT must register both components"
    );

    // Second PUT: identical list → idempotent (no diff; same set, same order
    // after sort).
    let (status_second, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &full_list,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_second, StatusCode::NO_CONTENT);

    let view_after_second = ctx
        .admin_state
        .db
        .factory_metadata()
        .get_device_view(device_sn)
        .await
        .unwrap()
        .expect("second PUT must not delete all associations");
    let mut sns_after_second: Vec<String> = view_after_second
        .iter()
        .map(|r| r.component_sn.clone())
        .collect();
    sns_after_second.sort();
    assert_eq!(
        sns_after_second,
        assert_associations(&[comp_a, comp_b]),
        "second identical PUT must be idempotent"
    );

    // Trimmed list (drop comp_b) → full-replace must DELETE the absent one.
    let trimmed_list = json!({
        "components": [
            { "componentSn": comp_a, "componentType": "camera" },
        ]
    });
    let (status_trim, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &path,
        &trimmed_list,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_trim, StatusCode::NO_CONTENT);

    let view_after_trim = ctx
        .admin_state
        .db
        .factory_metadata()
        .get_device_view(device_sn)
        .await
        .unwrap()
        .expect("trimmed list still has comp_a, so device is not 404");
    let sns_after_trim: Vec<String> = view_after_trim
        .iter()
        .map(|r| r.component_sn.clone())
        .collect();
    assert_eq!(
        sns_after_trim,
        vec![comp_a.to_string()],
        "trimmed-out association must be deleted by full-replace"
    );

    // No change_log ever written for association writes (R5 scopes the log to
    // component-metadata overwrites only).
    for sn in [comp_a, comp_b] {
        let (_, total) = ctx
            .admin_state
            .db
            .factory_metadata()
            .query_change_log(sn, 1, 10)
            .await
            .unwrap();
        assert_eq!(
            total, 0,
            "association writes must NOT produce change_log rows (R5)"
        );
    }
}

// ===========================================================================
// Scenario 5 — association arrives before metadata; left join returns null
// for the not-yet-arrived metadata fields (R3 partial-data semantics).
//
// User Story: US-PA-046 scenario 1 (associations + metadata async, independent).
// Covers: design §4.2.2 C, §4.3.2 (left join), §6.1 scenario
//         `association_arrives_before_metadata_left_join_returns_null`.
//
// Uses `FactoryAuthTestContext`. Asserts that during the gap (associations
// reported, metadata not yet), `GET /api/admin/factory/devices/{deviceSn}`
// returns 200 (NOT 404) with the association listed and `metadata=null`,
// `fileAttachments=[]`, `updatedAt=null` for that component.
// ===========================================================================
#[test_context(FactoryAuthTestContext)]
#[tokio::test]
async fn scenario_factory_association_arrives_before_metadata_left_join_returns_null(
    ctx: &mut FactoryAuthTestContext,
) {
    let device_sn = "dev_leftjoin_null";
    let component_sn = "comp_leftjoin_null";

    // Step 1: report the association only (metadata not yet arrived).
    let assoc_body = json!({
        "components": [
            { "componentSn": component_sn, "componentType": "camera" },
        ]
    });
    let (status_assoc, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &format!("/api/factory/devices/{device_sn}/components"),
        &assoc_body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_assoc, StatusCode::NO_CONTENT);

    // Step 2: admin GET during the gap — must be 200 + null metadata fields,
    // NOT 404 (design §4.2.2 C: partial data still returns 200 + null fields).
    let (status_get, body_get) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/factory/devices/{device_sn}"),
    )
    .await;
    assert_eq!(
        status_get,
        StatusCode::OK,
        "partial data (associations without metadata) must return 200, not 404"
    );

    let view: JsonValue = serde_json::from_str(&body_get).expect("view must be valid JSON");
    assert_eq!(view["deviceSn"], device_sn);
    assert!(
        view["deviceMetadata"].is_null(),
        "deviceMetadata is reserved null this round"
    );
    let components = view["components"]
        .as_array()
        .expect("components must be an array");
    assert_eq!(components.len(), 1, "exactly the one associated component");
    let comp = &components[0];
    assert_eq!(comp["componentSn"], component_sn);
    // assoc_type carries "camera"; meta_type is null → componentType resolves
    // to the association hint (camera) per map_row_to_component_view.
    assert_eq!(
        comp["componentType"], "camera",
        "assoc_type hint must surface when metadata has not arrived"
    );
    assert!(
        comp["metadata"].is_null(),
        "metadata must be null when not yet reported (R3 left-join)"
    );
    assert!(
        comp["fileAttachments"].is_array()
            && comp["fileAttachments"].as_array().unwrap().is_empty(),
        "fileAttachments must be [] when not yet reported"
    );
    assert!(
        comp["updatedAt"].is_null(),
        "updatedAt must be null when not yet reported"
    );
}

// ===========================================================================
// Scenario 6 — admin query returns strict 404 when device has NO data at all.
//
// User Story: US-PA-047 (admin read; distinguish "not reported" from "device
// does not exist").
// Covers: design §4.2.2 C error response, §6.1 scenario
//         `admin_query_returns_404_when_no_data_at_all`.
//
// Uses the default `TestContext`: admin GET does NOT require a factory API
// key, so the empty api_keys config is fine. Asserts a strict 404 (mutually
// exclusive with the 200+null behaviour of scenario 5) for a device that has
// no associations and no device-level metadata.
// ===========================================================================
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_factory_admin_query_returns_404_when_no_data_at_all(ctx: &mut TestContext) {
    let device_sn = "dev_never_reported";

    let (status, _) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/factory/devices/{device_sn}"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "device with no associations AND no device-level metadata must be strict 404"
    );
}

// ===========================================================================
// Scenario 7 — device pull webhook publishes reply with merged view.
//
// User Story: US-DV-011 (device reads own factory metadata via new topic).
// Covers: design §4.2.2 E, §5.3 (device pull webhook + publish_response to
//         {topic}_reply), §6.1 scenario
//         `device_get_publishes_reply_with_merged_view`.
//
// Uses the mockito-backed `FactoryWebhookContext`:
//   - Pre-seed an association + metadata via the factory writer path (valid
//     bearer on the same context).
//   - Simulate RMQTT forwarding a device publish by POSTing
//     `/api/thing/factory-metadata/get` with a base64-encoded
//     `{ "id": <req_id>, "ack": 0, "params": {} }` payload and a topic of the
//     form `/{product}/{device}/thing/factory-metadata/get`.
//   - Assert HTTP 204.
//   - Capture the `POST /mqtt/publish` body and assert:
//     * the publish `topic` is `{original_topic}_reply`;
//     * the inner `payload` (stringified MqttResponse) parses to an object
//       whose `id` equals the request id, `code` = 200, and `data` is the
//       left-join merged view (`deviceSn`, `deviceMetadata` null, `components`
//       array with the seeded component's metadata).
// ===========================================================================
#[test_context(FactoryWebhookContext)]
#[tokio::test]
async fn scenario_factory_device_get_publishes_reply_with_merged_view(
    ctx: &mut FactoryWebhookContext,
) {
    let product_id = "prod_factory_pull";
    let device_sn = "dev_factory_pull";
    let component_sn = "comp_factory_pull";

    // --- Pre-seed: association + metadata via the factory writer path. ---
    let assoc_body = json!({
        "components": [
            { "componentSn": component_sn, "componentType": "camera" },
        ]
    });
    let (status_assoc, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &format!("/api/factory/devices/{device_sn}/components"),
        &assoc_body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_assoc, StatusCode::NO_CONTENT);

    let meta_body = json!({
        "componentType": "camera",
        "metadata": { "calibration": "v1" },
        "fileAttachments": [
            { "fileKey": "factory-attachments/c1/calib.bin", "fileName": "calib.bin" }
        ]
    });
    let (status_meta, _) = request_json_with_headers(
        &ctx.service,
        Method::PUT,
        &format!("/api/factory/components/{component_sn}"),
        &meta_body,
        FACTORY_BEARER_HEADERS,
    )
    .await;
    assert_eq!(status_meta, StatusCode::NO_CONTENT);

    // --- Trigger the device-pull webhook. ---
    let topic = factory_metadata_get_topic(product_id, device_sn);
    let request_id = "factory-pull-req-001";
    let payload = json!({ "id": request_id, "ack": 0, "params": {} });
    let msg = mqtt_publish_message(device_sn, &topic, &payload);

    let (status_hook, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/factory-metadata/get",
        &msg,
    )
    .await;
    assert_eq!(
        status_hook,
        StatusCode::NO_CONTENT,
        "device-pull webhook must return 204"
    );

    // --- Inspect the captured publish body. ---
    let published = ctx
        .take_published_body()
        .await
        .expect("webhook must publish a reply via /mqtt/publish");

    // The outer publish body is `{ topic, payload (string), ... }`. Topic must
    // end with `_reply`.
    let published_topic = published
        .get("topic")
        .and_then(|v| v.as_str())
        .expect("publish body must carry a topic");
    assert_eq!(
        published_topic,
        format!("{topic}_reply"),
        "publish_response must target {{topic}}_reply"
    );

    // The `payload` is a STRINGIFIED MqttResponse; re-parse it.
    let payload_str = published
        .get("payload")
        .and_then(|v| v.as_str())
        .expect("publish payload must be a string");
    let response: JsonValue = serde_json::from_str(payload_str)
        .expect("publish payload string must parse as MqttResponse JSON");
    assert_eq!(
        response["id"], request_id,
        "reply must echo the original request id"
    );
    assert_eq!(response["code"], 200, "reply code must be 200");

    let data = &response["data"];
    assert!(
        data.is_object(),
        "data must be the merged-view object (not null when device has data)"
    );
    assert_eq!(data["deviceSn"], device_sn);
    assert!(
        data["deviceMetadata"].is_null(),
        "deviceMetadata is reserved null this round"
    );
    let components = data["components"]
        .as_array()
        .expect("components must be an array");
    assert_eq!(components.len(), 1, "exactly the seeded component");
    let comp = &components[0];
    assert_eq!(comp["componentSn"], component_sn);
    assert_eq!(comp["componentType"], "camera");
    assert_eq!(
        comp["metadata"]["calibration"], "v1",
        "metadata must come from the left-joined component row"
    );
    let file_attachments = comp["fileAttachments"]
        .as_array()
        .expect("fileAttachments must be an array");
    assert_eq!(file_attachments.len(), 1);
    assert_eq!(
        file_attachments[0]["fileKey"],
        "factory-attachments/c1/calib.bin"
    );
    // updatedAt is non-null once metadata has arrived.
    assert!(
        comp["updatedAt"].is_string(),
        "updatedAt must be present once metadata has arrived"
    );
}

// ===========================================================================
// Scenario 8 — ACL allows factory-metadata topic only for the device itself.
//
// User Story: §6.3 ACL regression risk point.
// Covers: design §4.2.2 ACL extension (allow-list grew by
//         `thing/factory-metadata`), §4.5, §6.1 scenario
//         `device_acl_allows_factory_metadata_topic_only_for_self`.
//
// Calls `acl(...)` directly (mirrors `auth_handlers.rs::tests` style) so the
// scenario is independent of the routing layers. Asserts:
//   - self topic `{product}/{client_id}/thing/factory-metadata/get` → allow.
//   - cross-device topic `{product}/{other_client}/thing/factory-metadata/get`
//     → deny (p1 != client_id is the load-bearing guard; design §6.3).
// ===========================================================================
#[tokio::test]
async fn scenario_factory_device_acl_allows_factory_metadata_topic_only_for_self() {
    use axum::Json;
    let product_id = "prod_acl_factory";
    let client_id = "dev_acl_self";
    let other_client_id = "dev_acl_other";

    // self topic — allowed (the §4.2.2 ACL extension to the allow-list).
    let self_payload = AclPayload {
        access: Access::Publish,
        username: Some(product_id.to_string()),
        client_id: client_id.to_string(),
        ip: "127.0.0.1".to_string(),
        topic: format!("/{product_id}/{client_id}/thing/factory-metadata/get"),
        protocol: json!(5),
    };
    assert_eq!(
        acl(Json(self_payload)).await,
        "allow",
        "self factory-metadata topic must be allowed"
    );

    // cross-device topic — denied by `p1 != client_id` (§6.3 regression guard).
    let cross_payload = AclPayload {
        access: Access::Publish,
        username: Some(product_id.to_string()),
        client_id: client_id.to_string(),
        ip: "127.0.0.1".to_string(),
        topic: format!("/{product_id}/{other_client_id}/thing/factory-metadata/get"),
        protocol: json!(5),
    };
    assert_eq!(
        acl(Json(cross_payload)).await,
        "deny",
        "cross-device factory-metadata topic must be denied"
    );
}
