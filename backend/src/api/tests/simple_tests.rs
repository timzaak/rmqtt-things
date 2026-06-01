use crate::api::handlers::{AppState, S3Client};
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{Config, S3Config};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tempfile::{TempDir, tempdir};
use test_context::{AsyncTestContext, test_context};
use tower::ServiceExt;
use uuid::Uuid;

pub struct TestContext {
    pub _app_state: Arc<AppState>,
    pub _admin_state: Arc<AdminAppState>,
    pub service: Router,
    pub _admin_pool: PgPool,
    pub schema_name: String,
    pub _temp_dir: TempDir,
}

impl AsyncTestContext for TestContext {
    async fn setup() -> TestContext {
        let _ = tracing_subscriber::fmt().try_init();
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let db_service = DatabaseService::new(pool);

        let endpoint = test_s3_endpoint();
        let s3_config = S3Config {
            endpoint,
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
            ..Default::default()
        };
        config.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        crate::ca::init_ca(&config.ca).await.unwrap();

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
            rule_cache: crate::rule_engine::RuleCache::new(),
        });

        let router = create_router(config, app_state.clone(), admin_state.clone(), None);

        TestContext {
            _app_state: app_state,
            _admin_state: admin_state,
            service: router,
            _admin_pool: admin_pool,
            schema_name,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

pub async fn create_test_database() -> (PgPool, String, PgPool) {
    let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://rmqtt_user:rmqtt_pass@127.0.0.1:16432/rmqtt_things?sslmode=disable&statement-cache-capacity=0".to_string()
    });
    let admin_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("failed to connect to test PostgreSQL via PgDog; run scripts/test-start.py first");

    let schema_name = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE SCHEMA "{}""#, schema_name))
        .execute(&admin_pool)
        .await
        .expect("failed to create test schema");

    let scoped_url = database_url_with_schema(&database_url, &schema_name);
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&scoped_url)
        .await
        .expect("failed to connect to isolated test schema");

    (admin_pool, schema_name, pool)
}

pub async fn drop_test_schema(pool: &PgPool, schema_name: &str) {
    sqlx::query(&format!(
        r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
        schema_name
    ))
    .execute(pool)
    .await
    .expect("failed to drop test schema");
}

pub fn test_s3_endpoint() -> String {
    std::env::var("TEST_S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:14566".to_string())
}

fn database_url_with_schema(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-c%20search_path%3D{schema_name}")
}

impl TestContext {
    pub async fn admin_get(&self, path: &str) -> (u16, String) {
        let (status, body) = request(&self.service, Method::GET, path).await;
        (status.as_u16(), body)
    }

    pub async fn admin_post_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> (u16, String) {
        let (status, body) = request_json(&self.service, Method::POST, path, body).await;
        (status.as_u16(), body)
    }

    pub async fn admin_patch_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> (u16, String) {
        let (status, body) = request_json(&self.service, Method::PATCH, path, body).await;
        (status.as_u16(), body)
    }

    pub async fn admin_post_json_with_headers<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
        headers: &[(&str, &str)],
    ) -> (u16, String) {
        let (status, body) =
            request_json_with_headers(&self.service, Method::POST, path, body, headers).await;
        (status.as_u16(), body)
    }
}

pub async fn request(service: &Router, method: Method, uri: &str) -> (StatusCode, String) {
    let response = service
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("x-real-ip", "127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

pub async fn request_json<T: serde::Serialize>(
    service: &Router,
    method: Method,
    uri: &str,
    body: &T,
) -> (StatusCode, String) {
    request_json_with_headers(service, method, uri, body, &[]).await
}

pub async fn request_json_with_headers<T: serde::Serialize>(
    service: &Router,
    method: Method,
    uri: &str,
    body: &T,
    headers: &[(&str, &str)],
) -> (StatusCode, String) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-real-ip", "127.0.0.1");
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    let response = service
        .clone()
        .oneshot(
            builder
                .body(Body::from(serde_json::to_vec(body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[test_context(TestContext)]
#[tokio::test]
async fn test_health_check(ctx: &mut TestContext) {
    let (status, _) = request(&ctx.service, Method::GET, "/api/health").await;
    assert_eq!(status, StatusCode::OK);
}

#[test_context(TestContext)]
#[tokio::test]
async fn test_property_post_and_get(ctx: &mut TestContext) {
    use crate::api::web_models::{AckStatus, RMqttPublishMessage};
    use base64::Engine;
    use serde_json::json;

    let client_id = "test_client_001".to_string();
    let product_id = "test_product_001".to_string();

    let topic = format!("/{product_id}/{client_id}/thing/event/property/post");
    let property_data = json!({
        "temperature": 25.5,
        "humidity": 60.0
    });
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: topic.clone(),
        payload: base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_string(&json!({
                "id": "123",
                "version": "1.0",
                "params": property_data,
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };

    // Test property post
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Test get property latest
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/property?device_id={}&product_id={}",
            client_id, product_id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    println!("body:\n{body}");
    let latest_property: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        latest_property["data"][0]["properties"]["humidity"]["value"],
        property_data["humidity"]
    );
    assert_eq!(
        latest_property["data"][0]["properties"]["temperature"]["value"],
        property_data["temperature"]
    );
}

#[test_context(TestContext)]
#[tokio::test]
async fn test_property_set_and_report(ctx: &mut TestContext) {
    use crate::api::admin_models::CreatePropertyCommandRequest;
    use crate::api::web_models::RMqttPublishMessage;
    use crate::db::models::CommandStatus;
    use base64::Engine;
    use serde_json::json;

    let client_id = "test_client_002".to_string();
    let product_id = "test_product_002".to_string();

    // 1. Server sends a property set command
    let command_request = CreatePropertyCommandRequest {
        product_id: product_id.clone(),
        device_id: client_id.clone(),
        command: json!({ "power": "on" }),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/property/command",
        &command_request,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Verify the command status is Pending
    let commands = ctx
        ._admin_state
        .db
        .query_property_commands(&product_id, Some(&client_id), None, 1, 10)
        .await
        .unwrap()
        .0;
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].status, CommandStatus::Pending);
    let id = commands[0].id;

    // 3. Client reports the execution result
    let topic = format!("/{product_id}/{client_id}/thing/event/property/reply");
    let ids = vec![id];
    let reply_payload = json!({
        "id": "123",
        "data": ids,
        "code": 200
    });
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: topic.clone(),
        payload: base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_string(&reply_payload).unwrap()),
        ..Default::default()
    };

    // 4. mock data has been sent to MQTT client
    ctx._app_state
        .db
        .update_property_command_status(
            &ids,
            &product_id,
            &client_id,
            CommandStatus::Sent,
            CommandStatus::Pending,
        )
        .await
        .unwrap();

    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/set_reply",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 5. Verify the command status is Success

    let commands = ctx
        ._admin_state
        .db
        .query_property_commands(&product_id, Some(&client_id), None, 1, 10)
        .await
        .unwrap()
        .0;
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].status, CommandStatus::Success);
}

#[test_context(TestContext)]
#[tokio::test]
async fn test_certificate_issue_and_revoke(ctx: &mut TestContext) {
    use crate::api::ca_handlers::{IssueCertRequest, UpdateCertStatusRequest};
    use crate::db::models::CertStatus;
    use time::{Duration, OffsetDateTime};

    let client_id = "test_client_003".to_string();
    let product_id = "test_product_003".to_string();

    // 1. Issue a certificate
    let issue_req = IssueCertRequest {
        product_id: product_id.clone(),
        device_id: client_id.clone(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };
    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/ca/cert", &issue_req).await;
    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cert_pem = resp["cert_pem"].as_str().unwrap();
    assert!(cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));

    // 2. Revoke the certificate
    let update_req = UpdateCertStatusRequest {
        id: None,
        product_id: product_id.clone(),
        device_id: client_id.clone(),
        status: CertStatus::Revoked as i16,
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        "/api/admin/ca/cert/status",
        &update_req,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 3. Verify the certificate status
    let cert = ctx
        ._admin_state
        .db
        .cert_issue()
        .find_by_device_id(&product_id, &client_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cert.status, CertStatus::Revoked);
}
