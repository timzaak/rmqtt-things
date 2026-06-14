use crate::api::handlers::{AppState, S3Client};
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::Config;
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Router;
use axum::http::{Method, StatusCode};
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::TempDir;
use test_context::{AsyncTestContext, test_context};

struct TestContext {
    _app_state: Arc<AppState>,
    _admin_state: Arc<AdminAppState>,
    service: Router,
    _admin_pool: PgPool,
    schema_name: String,
    _temp_dir: TempDir,
}

impl AsyncTestContext for TestContext {
    async fn setup() -> TestContext {
        let _ = tracing_subscriber::fmt().try_init();
        let (admin_pool, schema_name, pool) = super::simple_tests::create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let db_service = DatabaseService::new(pool, Default::default());

        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.api.property_schema_validator = true;
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
            rule_cache: crate::rule_engine::RuleCache::new_in_memory(),
            task_set: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
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
        super::simple_tests::drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

use super::simple_tests::{request, request_json};
use crate::api::admin_models::{
    CreateEventValidTemplateRequest, UpdateEventValidTemplateStatusRequest,
};
use crate::api::web_models::{AckStatus, RMqttPublishMessage};
use crate::db::models::EventValidTemplateStatus;
use base64::Engine;
use serde_json::json;

#[test_context(TestContext)]
#[tokio::test]
async fn test_schema_validation(ctx: &mut TestContext) {
    let product_id = "test_product_schema_001".to_string();
    let client_id = "test_client_schema_001".to_string();

    // 1. Create a schema for the product
    let schema = json!({
        "type": "object",
        "properties": {
            "temperature": {
                "type": "number"
            },
            "humidity": {
                "type": "number"
            }
        },
        "required": ["temperature", "humidity"]
    });

    let create_schema_req = CreateEventValidTemplateRequest {
        product_id: product_id.clone(),
        event: "property".to_string(),
        description: Some("test schema".to_string()),
        schema: schema.clone(),
    };

    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/valid/event",
        &create_schema_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let resp = UpdateEventValidTemplateStatusRequest {
        status: EventValidTemplateStatus::Active,
    };
    request_json(
        &ctx.service,
        Method::PATCH,
        "/api/admin/valid/event/1/status",
        &resp,
    )
    .await;

    let (_, body) = request(&ctx.service, Method::GET, "/api/admin/valid/event/1").await;
    println!("body:\n{body}");

    // 2. Post a valid property message
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
                "params": property_data,
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 3. Post an invalid property message
    let invalid_property_data = json!({
        "temperature": "hot",
        "humidity": 60.0
    });
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: topic.clone(),
        payload: base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_string(&json!({
                "id": "123",
                "params": invalid_property_data,
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/property/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// Event payload validation: when an Active event validation template exists
// for (product_id, event_identifier), event_post must validate `params`
// against the template schema before persisting. No template => allow.
// See validation-template.md §3.2 line 74 ("other values used for event
// validation") and P0-2 audit fix.
#[test_context(TestContext)]
#[tokio::test]
async fn test_event_schema_validation(ctx: &mut TestContext) {
    let product_id = "test_product_event_001".to_string();
    let client_id = "test_client_event_001".to_string();

    // 1. Create an Active event template for (product_id, "alert").
    let schema = json!({
        "type": "object",
        "properties": {
            "severity": {"type": "string", "enum": ["info", "warn", "error"]},
            "message": {"type": "string"}
        },
        "required": ["severity", "message"]
    });

    let create_req = CreateEventValidTemplateRequest {
        product_id: product_id.clone(),
        event: "alert".to_string(),
        description: Some("event payload schema".to_string()),
        schema: schema.clone(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/valid/event",
        &create_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let resp = UpdateEventValidTemplateStatusRequest {
        status: EventValidTemplateStatus::Active,
    };
    request_json(
        &ctx.service,
        Method::PATCH,
        "/api/admin/valid/event/1/status",
        &resp,
    )
    .await;

    // 2. Post a valid event: severity+message present, severity in enum.
    let topic = format!("/{product_id}/{client_id}/thing/event/alert/post");
    let valid_event = json!({"severity": "warn", "message": "temperature high"});
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: topic.clone(),
        payload: base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_string(&json!({
                "id": "ev1",
                "params": valid_event,
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/event/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 3. Post an invalid event: severity not in enum.
    let invalid_event = json!({"severity": "panic", "message": "x"});
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: topic.clone(),
        payload: base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_string(&json!({
                "id": "ev2",
                "params": invalid_event,
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/event/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 4. No Active template => event allowed through without validation.
    // Use a different event identifier that has no template registered.
    let no_template_topic = format!("/{product_id}/{client_id}/thing/event/heartbeat/post");
    let mqtt_message = RMqttPublishMessage {
        client_id: client_id.clone(),
        topic: no_template_topic,
        payload: base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_string(&json!({
                "id": "ev3",
                "params": json!({"any": "thing"}),
                "ack": AckStatus::No
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/event/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
