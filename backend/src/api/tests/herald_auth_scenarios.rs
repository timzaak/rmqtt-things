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
    pub refresh_token: String,
    pub _temp_dir: TempDir,
}

impl AsyncTestContext for HeraldAuthTestContext {
    async fn setup() -> HeraldAuthTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        // Database
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool, Default::default());

        // Herald configuration
        let herald_url = std::env::var("TEST_HERALD_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:13000".to_string());
        let herald_config = HeraldConfig {
            base_url: herald_url.clone(),
            api_key: "rmqtt-things-test-api-key".to_string(),
            realm_id: "rmqtt".to_string(),
            client_id: "admin-web-console".to_string(),
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
            crate::api::tests::simple_tests::empty_factory_auth_state(),
        );

        // Obtain a valid access + refresh token pair by logging in to Herald.
        // Since Herald 0.3.3, login returns a JSON BrowserTokenResponse body and
        // no longer sets an X-Auth cookie — so we read tokens from the body.
        let (session_token, refresh_token) = login_to_herald(&herald_url).await;

        HeraldAuthTestContext {
            service: router,
            _admin_pool: admin_pool,
            schema_name,
            _app_state: app_state,
            _admin_state: admin_state,
            session_token,
            refresh_token,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

/// Log in to Herald and return `(access_token, refresh_token)` from the JSON
/// `BrowserTokenResponse` body. Herald 0.3.3 dropped the `X-Auth` Set-Cookie in
/// favour of returning the token family as JSON.
async fn login_to_herald(herald_url: &str) -> (String, String) {
    let client = reqwest::Client::new();
    let login_url = format!("{herald_url}/api/auth/rmqtt/login");
    let resp = client
        .post(&login_url)
        .json(&serde_json::json!({
            "email": "admin@rmqtt-things.local",
            "password": "password",
            "clientId": "admin-web-console"
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

    let body: serde_json::Value = resp
        .json()
        .await
        .expect("Herald login response was not valid JSON");
    let access_token = body
        .get("accessToken")
        .and_then(|v| v.as_str())
        .expect("Herald login response missing accessToken")
        .to_string();
    let refresh_token = body
        .get("refreshToken")
        .and_then(|v| v.as_str())
        .expect("Herald login response missing refreshToken")
        .to_string();
    (access_token, refresh_token)
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

/// Like `request_with_cookie` but also returns the Set-Cookie headers so the
/// refresh/logout tests can assert cookie rotation/clearing.
async fn request_returning_cookies(
    service: &Router,
    method: Method,
    uri: &str,
    cookie_value: Option<&str>,
) -> (StatusCode, String, Vec<String>) {
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
    let cookies: Vec<String> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap(), cookies)
}

/// Refreshing a valid token rotates BOTH cookies and reports the access TTL.
///
/// Why: Herald 0.3.3 made access tokens expire every 900s. The web console must
/// call /api/auth/refresh before the access cookie lapses; this is the end-to-end
/// proof that the rotation path works against a live Herald.
#[test_context(HeraldAuthTestContext)]
#[tokio::test]
async fn test_scenario_refresh_rotates_both_cookies(ctx: &mut HeraldAuthTestContext) {
    let cookie = format!(
        "X-Auth={}; X-Auth-Refresh={}",
        ctx.session_token, ctx.refresh_token
    );
    let (status, body, cookies) = request_returning_cookies(
        &ctx.service,
        Method::POST,
        "/api/auth/refresh",
        Some(&cookie),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Expected 200 from /api/auth/refresh, got {status}: {body}"
    );

    // Body carries expiresIn so the client can schedule the next refresh.
    let json: serde_json::Value =
        serde_json::from_str(&body).expect("refresh response body was not JSON");
    let expires_in = json
        .get("expiresIn")
        .and_then(|v| v.as_i64())
        .expect("refresh response missing expiresIn");
    assert!(
        expires_in > 0 && expires_in <= 900,
        "access TTL should be the 15-min Herald value, got {expires_in}"
    );

    // Both cookies rotate (refresh is a token-rotation flow).
    let x_auth = cookies.iter().find(|c| c.starts_with("X-Auth="));
    let x_auth_refresh = cookies.iter().find(|c| c.starts_with("X-Auth-Refresh="));
    assert!(
        x_auth.is_some(),
        "refresh response missing X-Auth Set-Cookie"
    );
    assert!(
        x_auth_refresh.is_some(),
        "refresh response missing X-Auth-Refresh Set-Cookie"
    );
    // The new refresh cookie should outlive the access cookie (family lifetime
    // is 1–90d vs the 900s access token), proving we propagate both TTLs.
    let refresh_max_age = x_auth_refresh
        .and_then(|c| c.split("Max-Age=").nth(1))
        .and_then(|s| s.split(';').next())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    assert!(
        refresh_max_age >= 86_400,
        "refresh cookie Max-Age should be >= 1 day, got {refresh_max_age}"
    );
}

/// A missing or rejected refresh token yields 401, not a new session.
#[test_context(HeraldAuthTestContext)]
#[tokio::test]
async fn test_scenario_refresh_rejects_missing_cookie(ctx: &mut HeraldAuthTestContext) {
    let (status, _body, cookies) =
        request_returning_cookies(&ctx.service, Method::POST, "/api/auth/refresh", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "refresh without X-Auth-Refresh should be 401"
    );
    assert!(
        cookies.is_empty(),
        "a rejected refresh must not set any cookies"
    );
}

/// Logout clears both auth cookies so the client session ends locally.
#[test_context(HeraldAuthTestContext)]
#[tokio::test]
async fn test_scenario_logout_clears_cookies(ctx: &mut HeraldAuthTestContext) {
    let cookie = format!(
        "X-Auth={}; X-Auth-Refresh={}",
        ctx.session_token, ctx.refresh_token
    );
    let (status, body, cookies) = request_returning_cookies(
        &ctx.service,
        Method::POST,
        "/api/auth/logout",
        Some(&cookie),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Expected 200 from /api/auth/logout, got {status}: {body}"
    );

    let cleared = |name: &str| {
        cookies.iter().any(|c| {
            c.starts_with(&format!("{name}="))
                && c.split("Max-Age=")
                    .nth(1)
                    .is_some_and(|s| s.split(';').next().is_some_and(|v| v.trim() == "0"))
        })
    };
    assert!(cleared("X-Auth"), "logout did not clear X-Auth (Max-Age=0)");
    assert!(
        cleared("X-Auth-Refresh"),
        "logout did not clear X-Auth-Refresh (Max-Age=0)"
    );
}
