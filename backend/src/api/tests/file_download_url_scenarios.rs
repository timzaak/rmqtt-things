//! Scenario tests for the admin generic file download presigned-URL endpoint
//! (interface F, design §4.2.2 F / §4.5 / §5.5), built on top of the
//! dev-delivered production code from BE-D04
//! (`admin_file_download_url_handler` + `FileDownloadUrlQuery` /
//! `FileDownloadUrlResponse` DTO + directory whitelist + path-traversal
//! protection + 503 mapping + OpenAPI registration).
//!
//! Covers the 4 business scenarios from design §6.1 (`download-url` row).
//!
//! Test style mirrors `factory_metadata_scenarios.rs` (BE-T01, same slot, same
//! in-process axum `#[test_context(Ctx)]` + `#[tokio::test]` pattern) and
//! reuses `super::simple_tests::{request, test_s3_endpoint,
//! create_test_database, drop_test_schema}`. HTTP calls go through
//! `ctx.service`.
//!
//! Query parameter / JSON field naming notes (all verified against the BE-D04
//! production code, NOT assumed from the design doc):
//! - The route is `GET /api/admin/file/download-url`.
//! - The query parameter name is `fileKey` (camelCase). `FileDownloadUrlQuery`
//!   (`backend/src/api/admin_handlers.rs:24`) carries
//!   `#[serde(rename = "fileKey")]` on the `file_key` field (per-field rename,
//!   matching the same-feature `FileAttachment` convention), so axum-extra's
//!   `Query` extractor binds the camelCase name.
//! - The response body field names are `url` and `expiresInSeconds`
//!   (camelCase). `FileDownloadUrlResponse`
//!   (`backend/src/api/admin_handlers.rs:32`) carries
//!   `#[serde(rename = "expiresInSeconds")]` on `expires_in_seconds`, so the
//!   backend assertions use the camelCase names.
//!
//! Test function names carry the `scenario_admin_file_download_url_` prefix so
//! the runner (BE-TR01) can target them with the nextest expression
//! `test(~scenario_admin_file_download_url_)`.
//!
//! Context decisions (verified against the real whitelist matcher
//! `is_file_upload_directory_allowed`, `backend/src/api/handlers.rs:427`):
//! - The default `simple_tests::TestContext` configures
//!   `directories: vec!["/*".to_string()]`. For rule `"/*"`, the matcher's base
//!   is `""` and the allow condition becomes `directory == "" ||
//!   directory.starts_with("/")`. A non-empty, non-slash-prefixed directory
//!   like `ota` therefore does NOT pass the default whitelist — it would yield
//!   403 instead of the 200 the happy-path scenario must observe. The design's
//!   fallback ("construct a dedicated context") applies: see
//!   `FileDownloadUrlContext` below, which whitelists `ota/*` explicitly.
//! - The 503 scenario mirrors `factory_metadata_scenarios.rs::FactoryNoS3Context`
//!   (BE-T01): `Config { s3: None, .. }` rebuilt router ⇒
//!   `state.admin.s3_client == None` ⇒ handler returns 503.

use super::simple_tests::{create_test_database, drop_test_schema, request, test_s3_endpoint};
use crate::api::factory_middleware::FactoryAuthState;
use crate::api::handlers::{AppState, S3Client};
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{Config, S3Config};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Router;
use axum::http::{Method, StatusCode};
use serde_json::{Value as JsonValue, json};
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::{TempDir, tempdir};
use test_context::{AsyncTestContext, test_context};

// ===========================================================================
// Dedicated contexts.
//
// `FileDownloadUrlContext`: `s3: Some(..)` with `directories: vec!["ota/*"]`,
// so the `ota` directory passes the whitelist matcher
// (`is_file_upload_directory_allowed`: rule `"ota/*"` ⇒ base `"ota"` ⇒
// matches `directory == "ota" || directory.starts_with("ota/")`). Covers the
// 200 happy path, the 400 validation matrix (whitelist is never reached), and
// the 403 directory-not-allowed case (point the fileKey at a directory that is
// NOT in `["ota/*"]`, e.g. `secret`).
//
// `FileDownloadNoS3Context`: `s3: None` (mirrors
// `factory_metadata_scenarios.rs::FactoryNoS3Context`). `admin.s3_client` is
// `None` so the handler returns 503 before the whitelist / presign steps.
//
// Both contexts pass an empty `FactoryAuthState` (same as the default
// `TestContext`): the download-url endpoint is NOT mounted behind
// `factory_auth_middleware`, so the factory key list is irrelevant here.
// ===========================================================================

struct FileDownloadUrlContext {
    service: Router,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _admin_pool: PgPool,
    schema_name: String,
    _temp_dir: TempDir,
}

impl AsyncTestContext for FileDownloadUrlContext {
    async fn setup() -> FileDownloadUrlContext {
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
            // Whitelist the `ota` directory explicitly so the happy-path
            // scenario's `ota/<uuid>.bin` fileKey is allowed by
            // `is_file_upload_directory_allowed` (rule `"ota/*"` ⇒ base `"ota"`).
            directories: vec!["ota/*".to_string()],
            expired_seconds: 60,
        };
        let temp_dir = tempdir().unwrap();
        let mut config = Config {
            s3: Some(s3_config),
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

        // `herald_client = None` and `config.herald = None` ⇒ `admin_routes`
        // is mounted WITHOUT the Herald middleware (`api/mod.rs` match arm
        // `(_, _) => admin_routes`). The download-url endpoint is therefore
        // reachable via a plain GET with no Authorization header.
        let router = create_router(
            config,
            app_state.clone(),
            admin_state.clone(),
            None,
            empty_factory_auth_state(),
        );

        FileDownloadUrlContext {
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

struct FileDownloadNoS3Context {
    service: Router,
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    _admin_pool: PgPool,
    schema_name: String,
    _temp_dir: TempDir,
}

impl AsyncTestContext for FileDownloadNoS3Context {
    async fn setup() -> FileDownloadNoS3Context {
        let _ = tracing_subscriber::fmt().try_init();
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        let temp_dir = tempdir().unwrap();
        let mut config = Config {
            s3: None,
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::generate_ca_files(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        // `s3_client` intentionally `None`: the whole point of this context is
        // the 503 sub-case (design §4.2.2 F, §5.5).
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
            empty_factory_auth_state(),
        );

        FileDownloadNoS3Context {
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

/// Build a `FactoryAuthState` with an empty API-key list (same construction as
/// `simple_tests::empty_factory_auth_state`). Used because `create_router`
/// requires the argument; the download-url endpoint is not mounted behind
/// `factory_auth_middleware`, so the key list is irrelevant.
fn empty_factory_auth_state() -> Arc<FactoryAuthState> {
    let keys: Arc<[Box<str>]> = Vec::<Box<str>>::new().into();
    Arc::new(FactoryAuthState { api_keys: keys })
}

/// Build a presigned-download-URL request URI for the admin endpoint.
///
/// NOTE: the query parameter name is `fileKey` (camelCase), matching
/// `FileDownloadUrlQuery`'s per-field `#[serde(rename = "fileKey")]`.
///
/// The `file_key` value is interpolated literally. All call sites in this
/// module use only the unreserved characters `[A-Za-z0-9./]` (plus the empty
/// string), none of which require percent-encoding in a query value, so axum's
/// `Query` extractor sees the exact intended raw value. If a future call site
/// needs to exercise characters outside that set (e.g. spaces, `+`, `#`, `%`,
/// `&`), add a percent-encoding pass here (the workspace has no `urlencoding`
/// / `percent-encoding` crate dependency today, so it would have to be added
/// or inlined).
fn download_url_uri(file_key: &str) -> String {
    format!("/api/admin/file/download-url?fileKey={file_key}")
}

// ===========================================================================
// Scenario 1 — admin download-url returns a presigned URL (200 happy path).
//
// User Story: US-PA-047 (admin reads file attachments in the factory-metadata
// panel via a presigned S3 direct link).
// Covers: design §4.2.2 F success response, §5.5.
//
// Asserts:
//   - 200 OK on `GET /api/admin/file/download-url?fileKey=ota/<uuid>.bin`
//     (the `ota` directory is whitelisted by `FileDownloadUrlContext`).
//   - Response body parses to `{ url, expiresInSeconds }` with `url` a
//     non-empty string and `expiresInSeconds` equal to `[s3].expired_seconds`
//     (configured to 60 in this context).
//
// The S3 test stub at `TEST_S3_ENDPOINT` (default `http://127.0.0.1:14566`)
// is the same fake/minio endpoint used by `simple_tests::TestContext`,
// `s3_tests`, and `factory_metadata_scenarios`. `get_presigned_download_url`
// builds the URL locally (no network round-trip), so the presigned URL is
// returned even though no real object exists at that key (design §4.2.2 F:
// "presign does not issue a GET"; 404 handling is delegated to S3 when the
// frontend later fetches the URL).
// ===========================================================================
#[test_context(FileDownloadUrlContext)]
#[tokio::test]
async fn scenario_admin_file_download_url_returns_presigned_url(ctx: &mut FileDownloadUrlContext) {
    let file_key = "ota/calibration-uuid-001.bin";
    let (status, body) = request(&ctx.service, Method::GET, &download_url_uri(file_key)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "valid fileKey with whitelisted directory must return 200"
    );

    let response: JsonValue =
        serde_json::from_str(&body).expect("download-url response must be valid JSON");
    let url = response
        .get("url")
        .and_then(|v| v.as_str())
        .expect("response must carry a `url` string");
    assert!(
        !url.is_empty(),
        "presigned `url` must be a non-empty string"
    );
    assert_eq!(
        response["expiresInSeconds"],
        json!(60u64),
        "expiresInSeconds must equal [s3].expired_seconds (configured to 60)"
    );
}

// ===========================================================================
// Scenario 2 — admin download-url rejects invalid fileKey (400 validation).
//
// User Story: US-PA-047 (admin reads file attachments; malformed fileKey must
// be rejected before any S3 interaction).
// Covers: design §4.2.2 F 400 error response, §4.5 path-traversal protection.
//
// Asserts the four 400 sub-cases implemented in
// `admin_handlers::validate_file_key` (all share the same message
// `"Invalid fileKey"`):
//   1. empty fileKey → 400.
//   2. fileKey length > 1024 → 400.
//   3. fileKey contains a `..` path segment → 400.
//   4. fileKey starts with `/` (absolute path) → 400.
//
// Validation runs before the whitelist check and before the s3_client lookup,
// so all four are 400 even on `FileDownloadUrlContext` (s3 configured,
// `ota/*` whitelisted). The directory whitelist is NOT exercised here.
// ===========================================================================
#[test_context(FileDownloadUrlContext)]
#[tokio::test]
async fn scenario_admin_file_download_url_rejects_invalid_file_key(
    ctx: &mut FileDownloadUrlContext,
) {
    // (1) Empty fileKey → 400 "Invalid fileKey".
    let (status_empty, body_empty) =
        request(&ctx.service, Method::GET, &download_url_uri("")).await;
    assert_eq!(
        status_empty,
        StatusCode::BAD_REQUEST,
        "empty fileKey must be 400"
    );
    assert_error_message(&body_empty, "Invalid fileKey");

    // (2) fileKey longer than 1024 chars → 400 "Invalid fileKey".
    let overlong = "a".repeat(1025);
    let (status_long, body_long) =
        request(&ctx.service, Method::GET, &download_url_uri(&overlong)).await;
    assert_eq!(
        status_long,
        StatusCode::BAD_REQUEST,
        "fileKey longer than 1024 chars must be 400"
    );
    assert_error_message(&body_long, "Invalid fileKey");

    // (3) fileKey containing a `..` path segment → 400 "Invalid fileKey".
    //     `ota/../secret.bin` splits to ["ota", "..", "secret.bin"] which
    //     contains a `..` segment.
    let (status_traversal, body_traversal) = request(
        &ctx.service,
        Method::GET,
        &download_url_uri("ota/../secret.bin"),
    )
    .await;
    assert_eq!(
        status_traversal,
        StatusCode::BAD_REQUEST,
        "fileKey containing a `..` segment must be 400 (path traversal)"
    );
    assert_error_message(&body_traversal, "Invalid fileKey");

    // (4) Absolute path (leading `/`) → 400 "Invalid fileKey".
    let (status_absolute, body_absolute) =
        request(&ctx.service, Method::GET, &download_url_uri("/etc/passwd")).await;
    assert_eq!(
        status_absolute,
        StatusCode::BAD_REQUEST,
        "absolute-path fileKey must be 400 (path traversal)"
    );
    assert_error_message(&body_absolute, "Invalid fileKey");
}

// ===========================================================================
// Scenario 3 — admin download-url rejects fileKey whose directory is not on
// the whitelist (403).
//
// User Story: US-PA-047 (admin must NOT be able to read arbitrary S3 keys).
// Covers: design §4.2.2 F 403 error response, §4.5 directory whitelist
//         (prevents reading outside the configured S3 prefixes).
//
// Asserts:
//   - 403 on `GET /api/admin/file/download-url?fileKey=secret/file.bin`.
//   - Body message is exactly `"Directory not allowed"`.
//
// The `secret` directory is NOT in `FileDownloadUrlContext`'s
// `directories: vec!["ota/*"]` whitelist, so
// `is_file_upload_directory_allowed` returns false and the handler maps it to
// `ApiError::forbidden_with("Directory not allowed")` (503 is NOT reached
// because s3_client is `Some` in this context; the whitelist check happens
// after the s3_client lookup but before presigning).
// ===========================================================================
#[test_context(FileDownloadUrlContext)]
#[tokio::test]
async fn scenario_admin_file_download_url_rejects_directory_not_allowed(
    ctx: &mut FileDownloadUrlContext,
) {
    // `secret/leak.bin` ⇒ directory `secret`, which is NOT in `["ota/*"]`.
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &download_url_uri("secret/leak.bin"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "fileKey whose directory is not on the whitelist must be 403"
    );
    assert_error_message(&body, "Directory not allowed");
}

// ===========================================================================
// Scenario 4 — admin download-url returns 503 when S3 is not configured.
//
// User Story: US-PA-047 (admin reads file attachments; the S3 backend being
// unconfigured must surface as 503, matching the existing file-upload
// convention).
// Covers: design §4.2.2 F 503 error response, §5.5 503 mapping.
//
// Asserts:
//   - 503 on `GET /api/admin/file/download-url?fileKey=ota/<uuid>.bin` when
//     `state.admin.s3_client` is `None`.
//   - Body message is exactly `"S3 client not configured"`.
//
// Uses `FileDownloadNoS3Context` (mirrors BE-T01 `FactoryNoS3Context`:
// `Config { s3: None, .. }` rebuilt router ⇒ `admin.s3_client = None`). The
// path-traversal validation passes (`ota/uuid.bin` is well-formed), so the
// handler reaches the `s3_client.as_ref().ok_or_else(..)` branch and returns
// `ApiError::service_unavailable("S3 client not configured")`.
// ===========================================================================
#[test_context(FileDownloadNoS3Context)]
#[tokio::test]
async fn scenario_admin_file_download_url_returns_503_when_s3_not_configured(
    ctx: &mut FileDownloadNoS3Context,
) {
    let file_key = "ota/calibration-uuid-002.bin";
    let (status, body) = request(&ctx.service, Method::GET, &download_url_uri(file_key)).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "s3_client = None must surface as 503, matching the file-upload convention"
    );
    assert_error_message(&body, "S3 client not configured");
}

// --- shared assertion helper ---

/// Parse a JSON error body and assert the `error` field equals `expected`.
///
/// `ApiError::IntoResponse` (`backend/src/api/error.rs:76`) serialises the
/// error as `{"error": "<message>"}`. Backend scenario tests assert against
/// this raw shape (no camelCase rename on the `error` key).
fn assert_error_message(body: &str, expected: &str) {
    let json: JsonValue = serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("error body must be valid JSON: {body:?} (parse error: {e})"));
    assert_eq!(
        json["error"], expected,
        "error body `error` field must be exactly {expected:?}; got body: {body:?}"
    );
}
