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
    CreateEventValidTemplateRequest, UpdateEventValidTemplateRequest,
    UpdateEventValidTemplateStatusRequest,
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

// Event template cache invalidation (P2-9 audit fix).
//
// `load_event_validator` caches an Active event schema under
// `event:{product_id}:{event}` so subsequent event_post requests skip the DB.
// When the template's status flips (Inactive<->Active) or its schema is
// updated, the cache entry MUST be invalidated, otherwise event_post keeps
// validating against the old schema (validation-template.md §4.2 "模板状态
// 变更或更新时清除缓存").
//
// The DB layer forbids updating the schema of an Active template
// (`ACTIVE_TEMPLATE_SCHEMA_ERR`), so the real-world "change an event schema"
// flow is: Active -> Inactive -> update schema -> Active. Each transition
// routes through a different handler; all three must invalidate the cache.
//
// Flow under test:
//   1. Create + Activate a LENIENT event schema (severity: any string).
//   2. POST severity="panic" -> 204 (accepted, cache populated).
//   3. Set Inactive (handler: update_event_valid_template_status).
//   4. Update schema to STRICT (severity ∈ {info,warn,error})
//      (handler: update_event_valid_template).
//   5. Re-Activate (handler: update_event_valid_template_status again).
//   6. POST severity="panic" again -> must be 400. If ANY of the three
//      invalidation points is missing, the stale lenient schema is served
//      from cache and this returns 204 (the P2-9 bug).
#[test_context(TestContext)]
#[tokio::test]
async fn test_event_template_update_invalidates_cache(ctx: &mut TestContext) {
    let product_id = "evt_cache_inval_prod".to_string();
    let client_id = "evt_cache_inval_dev".to_string();
    let event_id = "cache_event";

    // Helper to build an event_post MQTT message with a given severity.
    let post_event_with_severity = |severity: &str| {
        let topic = format!("/{product_id}/{client_id}/thing/event/{event_id}/post");
        let payload = json!({"severity": severity});
        RMqttPublishMessage {
            client_id: client_id.clone(),
            topic,
            payload: base64::engine::general_purpose::STANDARD.encode(
                serde_json::to_string(&json!({
                    "id": "evt",
                    "params": payload,
                    "ack": AckStatus::No
                }))
                .unwrap(),
            ),
            ..Default::default()
        }
    };

    // 1. Create + Activate a LENIENT event template.
    let lenient = json!({
        "type": "object",
        "properties": { "severity": {"type": "string"} },
        "required": ["severity"]
    });
    let create_req = CreateEventValidTemplateRequest {
        product_id: product_id.clone(),
        event: event_id.to_string(),
        description: Some("lenient v1".to_string()),
        schema: lenient,
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/valid/event",
        &create_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Create returns 201 with no body; look the id up via the list endpoint.
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/valid/event?product_id={product_id}&event={event_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    let template_id: i64 = list["data"][0]["id"]
        .as_i64()
        .expect("template id must be present in list response");

    // 1a. Activate the lenient template.
    {
        let req = UpdateEventValidTemplateStatusRequest {
            status: EventValidTemplateStatus::Active,
        };
        let url = format!("/api/admin/valid/event/{template_id}/status");
        let _ = request_json(&ctx.service, Method::PATCH, &url, &req).await;
    }

    // 2. POST severity="panic" -> 204 under lenient schema. This populates
    //    the event:{product}:{event} cache entry.
    let mqtt_message = post_event_with_severity("panic");
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/event/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "lenient schema must accept any severity string (and populates the cache)"
    );

    // 3. Set Inactive. This handler must invalidate the cache so the next
    //    event_post does not serve the (now-inactive) cached schema.
    {
        let req = UpdateEventValidTemplateStatusRequest {
            status: EventValidTemplateStatus::Inactive,
        };
        let url = format!("/api/admin/valid/event/{template_id}/status");
        let _ = request_json(&ctx.service, Method::PATCH, &url, &req).await;
    }

    // 4. Update the schema to STRICT. Allowed because the template is now
    //    Inactive. This handler must ALSO invalidate the cache (defensive:
    //    there may be no cache entry at this point, but the call must be
    //    idempotent and not error).
    let strict = json!({
        "type": "object",
        "properties": { "severity": {"type": "string", "enum": ["info", "warn", "error"]} },
        "required": ["severity"]
    });
    let update_req = UpdateEventValidTemplateRequest {
        schema: Some(strict),
        description: Some("strict v2".to_string()),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/valid/event/{template_id}"),
        &update_req,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "schema update on an Inactive template must succeed"
    );

    // 5. Re-Activate. With the strict schema now in the DB, the cache MUST be
    //    invalidated again so event_post re-reads the strict schema.
    {
        let req = UpdateEventValidTemplateStatusRequest {
            status: EventValidTemplateStatus::Active,
        };
        let url = format!("/api/admin/valid/event/{template_id}/status");
        let _ = request_json(&ctx.service, Method::PATCH, &url, &req).await;
    }

    // 6. POST severity="panic" again. The strict schema rejects it -> 400.
    //    If any of the three invalidation points failed, the stale lenient
    //    schema is served from cache -> 204 (the P2-9 bug).
    let mqtt_message = post_event_with_severity("panic");
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/thing/event/post",
        &mqtt_message,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "after Active->Inactive->update->Active the event cache must be \
         invalidated so the strict v2 schema rejects severity='panic'"
    );
}
