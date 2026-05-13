use crate::api::handlers::{AppState, S3Client};
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{Config, HeraldConfig, S3Config};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::{TempDir, tempdir};
use test_context::{AsyncTestContext, test_context};
use tower::ServiceExt;

use super::simple_tests::{create_test_database, drop_test_schema, test_s3_endpoint};

pub struct HeraldAuthTestContext {
    pub service: Router,
    pub _admin_pool: PgPool,
    pub schema_name: String,
    pub _app_state: Arc<AppState>,
    pub _admin_state: Arc<AdminAppState>,
    pub session_token: String,
    pub _temp_dir: TempDir,
}

impl AsyncTestContext for HeraldAuthTestContext {
    async fn setup() -> HeraldAuthTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        // Database
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool);

        // Herald configuration
        let herald_url = std::env::var("TEST_HERALD_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:13000".to_string());
        let herald_config = HeraldConfig {
            base_url: herald_url.clone(),
            api_key: "rmqtt-things-test-api-key".to_string(),
            realm_id: "default".to_string(),
            client_id: "rmqtt-things-admin".to_string(),
        };

        // Build app config with Herald enabled
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
            herald: Some(herald_config),
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::init_ca(&config.ca).await.unwrap();

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
        });

        // Create Herald SDK client and pass it to the router
        let herald_client = Arc::new(herald_sdk::Client::new(
            herald_url.clone(),
            "rmqtt-things-test-api-key".to_string(),
            None,
        ));
        let router = create_router(
            config,
            app_state.clone(),
            admin_state.clone(),
            Some(herald_client),
        );

        // Obtain a valid session token by logging in to Herald
        let session_token = login_to_herald(&herald_url).await;

        HeraldAuthTestContext {
            service: router,
            _admin_pool: admin_pool,
            schema_name,
            _app_state: app_state,
            _admin_state: admin_state,
            session_token,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

/// Log in to Herald and return the `X-Auth` cookie value.
async fn login_to_herald(herald_url: &str) -> String {
    let client = reqwest::Client::new();
    let login_url = format!("{herald_url}/api/auth/default/login");
    let resp = client
        .post(&login_url)
        .json(&serde_json::json!({
            "email": "admin@rmqtt-things.local",
            "password": "password",
            "clientId": "rmqtt-things-admin"
        }))
        .send()
        .await
        .expect("Failed to call Herald login endpoint; is Herald running?");

    assert!(
        resp.status().is_success(),
        "Herald login failed with status {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    // Extract X-Auth cookie from Set-Cookie header
    let set_cookie = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .find_map(|v| {
            let val = v.to_str().ok()?;
            if val.starts_with("X-Auth=") {
                Some(val.to_string())
            } else {
                None
            }
        })
        .expect("Herald login response did not contain X-Auth cookie");

    // Parse out just the cookie value (before any semicolon)
    set_cookie
        .trim_start_matches("X-Auth=")
        .split(';')
        .next()
        .expect("Failed to parse X-Auth cookie value")
        .to_string()
}

/// Helper to send a request through the oneshot router with an optional Cookie header.
async fn request_with_cookie(
    service: &Router,
    method: Method,
    uri: &str,
    cookie_value: Option<&str>,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie_value {
        builder = builder.header(header::COOKIE, cookie);
    }
    let response = service
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Herald auth middleware protects admin routes:
/// - No cookie → 401, valid token → 200, bad token → 401/403
#[test_context(HeraldAuthTestContext)]
#[tokio::test]
async fn test_scenario_herald_auth_protects_admin_routes(ctx: &mut HeraldAuthTestContext) {
    // (a) No auth cookie -> 401
    let (status, body) =
        request_with_cookie(&ctx.service, Method::GET, "/api/admin/product", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "Expected 401 without auth cookie, got {status}: {body}"
    );

    // (b) Valid Herald token -> 200 (empty product list)
    let cookie = format!("X-Auth={}", ctx.session_token);
    let (status, body) = request_with_cookie(
        &ctx.service,
        Method::GET,
        "/api/admin/product",
        Some(&cookie),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Expected 200 with valid auth token, got {status}: {body}"
    );

    // (c) Invalid token -> 401 or 403
    let bad_cookie = "X-Auth=invalid_token_value";
    let (status, body) = request_with_cookie(
        &ctx.service,
        Method::GET,
        "/api/admin/product",
        Some(bad_cookie),
    )
    .await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "Expected 401 or 403 with invalid token, got {status}: {body}"
    );
}
