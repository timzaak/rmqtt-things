use crate::api::utils::{
    extract_and_validate_product_id, extract_event_identifier_from_topic,
    send_property_command_to_device, validate_identifier,
};
use crate::api::web_models::*;
use crate::cache::{SchemaCache, SchemaCacheManager, compile_schema};
use crate::config::Config;
use crate::db::database::DatabaseService;
use crate::db::models::CommandStatus;
use crate::rmqtt_client::RmqttHttpClient;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use jsonschema::Validator;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::post_policy::{PostPolicy, PostPolicyField, PostPolicyValue};
use s3::region::Region;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;

use crate::api::ApiState;
use crate::api::error::ApiError;
use crate::rule_engine::{TriggerContext, TriggerType, evaluate_and_trigger};

#[derive(Deserialize, ToSchema)]
pub struct PropertySetReplyPayload {
    data: Vec<i64>,
    #[allow(dead_code)]
    id: String,
    code: u32,
}

pub struct AppState {
    pub db: DatabaseService,
    pub rmqtt_client: RmqttHttpClient,
    pub config: Arc<Config>,
    pub cache: SchemaCache,
    pub s3_client: Option<S3Client>,
}

// 属性上报接口
#[utoipa::path(
    post,
    path = "/api/thing/property/post",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Property accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn property_post(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;

    info!("Received property set from device: {}", mqtt_msg.client_id);

    // 解析 payload
    let payload = mqtt_msg.decode_payload_as_json().map_err(|e| {
        error!("Failed to decode payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;
    let timestamp = OffsetDateTime::now_utc();
    let properties = payload.params.unwrap_or(JsonValue::Null);
    if let JsonValue::Object(map) = &properties {
        // 如果开启了 schema 校验
        if app_state.config.api.property_schema_validator {
            let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;

            // 尝试从缓存中获取 schema
            let schema_value = app_state.cache.get(&product_id).await.map_err(|e| {
                error!("Cache error: {}", e);
                ApiError::internal("Cache error")
            })?;

            let validator = if let Some(schema) = schema_value {
                compile_schema(&schema).map_err(|e| {
                    error!("Failed to compile schema from cache: {}", e);
                    ApiError::internal("Schema compilation failed")
                })?
            } else {
                // 从数据库获取 schema
                let schema_template = app_state
                    .db
                    .get_property_schema(&product_id)
                    .await
                    .map_err(|e| {
                        error!("Database error while getting schema: {}", e);
                        ApiError::internal("Database operation failed")
                    })?
                    .ok_or_else(|| {
                        error!("Schema not found for product_id: {}", product_id);
                        ApiError::bad_request("Schema not found")
                    })?;

                // 编译 schema
                let validator = compile_schema(&schema_template.schema).map_err(|e| {
                    error!("Failed to compile schema: {}", e);
                    ApiError::internal("Schema compilation failed")
                })?;

                // 异步地将 schema 存入缓存
                let cache_clone = app_state.cache.clone();
                let product_id_clone = product_id.clone();
                let schema_to_cache = schema_template.schema.clone();
                tokio::spawn(async move {
                    if let Err(e) = cache_clone.set(product_id_clone, schema_to_cache).await {
                        error!("Failed to cache schema: {}", e);
                    }
                });
                validator
            };

            // 验证属性
            let errors: Vec<_> = validator.iter_errors(&properties).collect();
            if !errors.is_empty() {
                let error_messages: Vec<String> =
                    errors.into_iter().map(|err| err.to_string()).collect();
                error!(
                    "Property validation failed for device {}: {:?}",
                    device_id, error_messages
                );
                return Err(ApiError::bad_request("Property validation failed"));
            }
        }

        let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
        // 数据库操作
        app_state
            .db
            .upsert_property_latest(&product_id, device_id, map.clone(), timestamp)
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ApiError::internal("Database operation failed")
            })?;

        // 异步触发规则评估（不阻塞主流程）
        let admin = Arc::clone(&state.admin);
        let task_set = admin.task_set.clone();
        let trigger_product_id = product_id.clone();
        let trigger_device_id = device_id.clone();
        let trigger_value = properties.clone();
        task_set.lock().await.spawn(async move {
            let alarm_repo = admin.db.alarm();
            let rule_cache = admin.rule_cache.clone();
            let ctx = TriggerContext {
                product_id: trigger_product_id,
                device_id: trigger_device_id,
                trigger_type: TriggerType::Property,
                trigger_value,
            };
            evaluate_and_trigger(ctx, alarm_repo, rule_cache, None).await;
        });
    } else {
        return Err(ApiError::bad_request("Invalid params format"));
    }

    // 如果需要响应，发布到 RMQTT
    if payload.ack == AckStatus::Yes {
        let _ = ack_response(payload.id, &app_state.rmqtt_client, &mqtt_msg.topic).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

// 事件上报接口
#[utoipa::path(
    post,
    path = "/api/thing/event/post",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Event accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn event_post(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;

    let payload = mqtt_msg.decode_payload_as_json().map_err(|e| {
        error!("Failed to decode payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;
    let timestamp = OffsetDateTime::now_utc();
    let events = payload.params.unwrap_or(JsonValue::Null);

    let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;

    // 事件 schema 校验：当 thing schema validator 开启且存在 Active 状态的
    // (product_id, event_identifier) 模板时，校验 params（事件负载）。
    // 无模板则放行，与 property_post 的"无 schema 即放行"语义一致。
    // See validation-template.md §3.2 第 4 条：「其他值用于事件校验」。
    if app_state.config.api.property_schema_validator
        && let Some(event_identifier) = extract_event_identifier_from_topic(&mqtt_msg.topic)
        && let Some(validator) =
            load_event_validator(app_state, &product_id, &event_identifier).await?
    {
        let errors: Vec<_> = validator.iter_errors(&events).collect();
        if !errors.is_empty() {
            let error_messages: Vec<String> =
                errors.into_iter().map(|err| err.to_string()).collect();
            error!(
                "Event validation failed for device {}: event={}, errors={:?}",
                device_id, event_identifier, error_messages
            );
            return Err(ApiError::bad_request("Event validation failed"));
        }
    }

    // 保存事件到数据库
    app_state
        .db
        .insert_event_history(&product_id, device_id, &events, timestamp)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    // 异步触发规则评估（不阻塞主流程）
    let admin = Arc::clone(&state.admin);
    let task_set = admin.task_set.clone();
    let trigger_product_id = product_id.clone();
    let trigger_device_id = device_id.clone();
    let trigger_value = events.clone();
    task_set.lock().await.spawn(async move {
        let alarm_repo = admin.db.alarm();
        let rule_cache = admin.rule_cache.clone();
        let ctx = TriggerContext {
            product_id: trigger_product_id,
            device_id: trigger_device_id,
            trigger_type: TriggerType::Event,
            trigger_value,
        };
        evaluate_and_trigger(ctx, alarm_repo, rule_cache, None).await;
    });

    // 如果需要响应，发布到 RMQTT
    if payload.ack == AckStatus::Yes {
        let _ = ack_response(payload.id, &app_state.rmqtt_client, &mqtt_msg.topic).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

use crate::api::ack_response;
use crate::config::S3Config;
use std::borrow::Cow;

#[derive(Clone)]
pub struct S3Client {
    bucket: Bucket,
    pub config: S3Config,
}

impl S3Client {
    pub fn new(s3_config: &S3Config) -> Result<Self, anyhow::Error> {
        let region = Region::Custom {
            region: s3_config.region.clone(),
            endpoint: s3_config.endpoint.clone(),
        };
        let credentials = Credentials::new(
            Some(&s3_config.access_key),
            Some(&s3_config.secret_key),
            None,
            None,
            None,
        )?;

        // Use path-style addressing so presigned URLs against IPv4 endpoints
        // (e.g. LocalStack http://127.0.0.1:14566) are built as
        // http://<endpoint>/<bucket>/... instead of the virtual-host-style
        // http://<bucket>.<endpoint>/... which Url::parse rejects with
        // "invalid IPv4 address".
        let bucket = Bucket::new(&s3_config.bucket, region, credentials)?.with_path_style();

        Ok(S3Client {
            bucket: *bucket,
            config: s3_config.clone(),
        })
    }

    pub async fn get_presigned_post(
        &self,
        key: &str,
    ) -> Result<s3::post_policy::PresignedPost, s3::error::S3Error> {
        // Pin the S3 object key to the exact value the server chose. The
        // previous StartsWith(key) policy allowed a client to upload to any
        // key sharing the prefix, which is unnecessary here because the server
        // fully controls the key (directory + UUID-prefixed file name) and
        // widens the upload surface to unintended keys. rust-s3 0.37 supports
        // PostPolicyValue::Exact, which emits an `{ "key": "<value>" }` policy
        // condition (P1-7 audit fix).
        let post_policy = PostPolicy::new(self.config.expired_seconds)
            .condition(PostPolicyField::Key, PostPolicyValue::Exact(Cow::from(key)))
            .unwrap();
        self.bucket.presign_post(post_policy).await
    }

    pub async fn get_presigned_download_url(
        &self,
        key: &str,
    ) -> Result<String, s3::error::S3Error> {
        self.bucket
            .presign_get(key, self.config.expired_seconds, None)
            .await
    }
}

// 文件上传接口
#[utoipa::path(
    post,
    path = "/api/thing/file/upload",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Upload command accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn file_upload_handler(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let state = &state.app;
    let payload = mqtt_msg.decode_payload_as_json().map_err(|e| {
        error!("Failed to decode payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    if let Some(s3_client) = &state.s3_client {
        let file_upload_req: FileUploadRequest =
            serde_json::from_value(payload.params.unwrap_or(JsonValue::Null)).map_err(|e| {
                error!("Failed to parse FileUploadRequest: {}", e);
                ApiError::bad_request("Invalid params for FileUploadRequest")
            })?;

        let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
        let device_id = &mqtt_msg.client_id;
        validate_identifier(device_id, "device_id")?;

        if !is_file_upload_directory_allowed(
            &s3_client.config.directories,
            &product_id,
            device_id,
            &file_upload_req.directory,
        ) {
            return Err(ApiError::bad_request("Directory not allowed"));
        }

        let file_name = if file_upload_req.use_origin_name {
            file_upload_req.file_name.clone()
        } else {
            format!("{}_{}", uuid::Uuid::new_v4(), file_upload_req.file_name)
        };
        let file_path = format!("{}/{}", file_upload_req.directory, file_name);

        let presigned_post = s3_client
            .get_presigned_post(&file_path)
            .await
            .map_err(|e| {
                error!("Failed to get presigned post: {}", e);
                ApiError::internal("Failed to get presigned post")
            })?;

        let response_data = FileUploadResponse {
            url: presigned_post.url,
            fields: presigned_post.fields,
        };

        let response = MqttResponse {
            id: payload.id,
            code: 200,
            data: Some(json!(response_data)),
        };

        let response_payload = serde_json::to_string(&response).map_err(|e| {
            error!("Failed to serialize response: {}", e);
            ApiError::internal("Failed to serialize response")
        })?;

        if let Err(e) = state
            .rmqtt_client
            .publish_response(&mqtt_msg.topic, &response_payload)
            .await
        {
            error!("Failed to publish response: {}", e);
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        warn!("does not support file upload");
        let response = MqttResponse {
            id: payload.id,
            code: 503,
            data: Some(json!("do not support file upload")),
        };
        let response_payload = serde_json::to_string(&response).map_err(|e| {
            error!("Failed to serialize response: {}", e);
            ApiError::internal("Failed to serialize response")
        })?;
        if let Err(e) = state
            .rmqtt_client
            .publish_response(&mqtt_msg.topic, &response_payload)
            .await
        {
            error!("Failed to publish response: {}", e);
        }
        Ok(StatusCode::NO_CONTENT)
    }
}

pub fn is_file_upload_directory_allowed(
    rules: &[String],
    product_id: &str,
    device_id: &str,
    directory: &str,
) -> bool {
    rules.iter().any(|rule| {
        let rule = rule
            .replace("${productId}", product_id)
            .replace("${deviceId}", device_id);

        if let Some(base) = rule.strip_suffix("/*") {
            directory == base || directory.starts_with(&format!("{base}/"))
        } else if let Some(prefix) = rule.strip_suffix('*') {
            directory.starts_with(prefix)
        } else {
            directory == rule
        }
    })
}

#[utoipa::path(
    post,
    path = "/api/thing/property/set_subscribe",
    tag = "thing",
    request_body = RMqttSubscribeMessage,
    responses(
        (status = 204, description = "Subscription accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn property_set_subscribe(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttSubscribeMessage>,
) -> Result<StatusCode, ApiError> {
    let state = &state.app;

    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;
    info!("Processing property post for device: {}", device_id);

    let topic = mqtt_msg.topic.as_deref().ok_or_else(|| {
        error!(
            "Topic not found in subscribe message for device: {}",
            device_id
        );
        ApiError::bad_request("Topic not found")
    })?;
    let product_id = extract_and_validate_product_id(topic)?;
    // 使用共享函数发送命令
    if let Err(e) =
        send_property_command_to_device(&state.db, &state.rmqtt_client, &product_id, device_id)
            .await
    {
        error!(
            "Failed to send property command to device {}: {}",
            device_id, e
        );
        return Err(ApiError::internal("Failed to publish command"));
    }
    Ok(StatusCode::NO_CONTENT)
}

// 属性下发结果上报接口
#[utoipa::path(
    post,
    path = "/api/thing/property/set_reply",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Reply accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn property_set_reply(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let state = &state.app;

    let bytes = mqtt_msg.decode_payload().map_err(|e| {
        debug!("Failed to decode base64 payload: {}", e);
        ApiError::bad_request("Invalid payload encoding")
    })?;
    let payload: PropertySetReplyPayload = serde_json::from_slice(&bytes).map_err(|e| {
        debug!("Failed to parse payload JSON: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    let command_ids = payload.data;
    let status = if payload.code == 200 {
        CommandStatus::Success
    } else {
        CommandStatus::Failed
    };

    let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
    validate_identifier(&mqtt_msg.client_id, "device_id")?;
    // 更新命令状态
    state
        .db
        .update_property_command_status(
            &command_ids,
            &product_id,
            &mqtt_msg.client_id,
            status,
            CommandStatus::Sent,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    info!(
        "Updated property command {:?} status to {:?}",
        command_ids, status
    );

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses((status = 200, description = "Service is healthy"))
)]
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "health",
        "timestamp": OffsetDateTime::now_utc()
    }))
}

#[cfg(test)]
mod tests {
    use super::is_file_upload_directory_allowed;

    #[test]
    fn file_upload_directory_wildcard_allows_base_and_children() {
        let rules = vec![
            "${productId}/${deviceId}/*".to_string(),
            "public/*".to_string(),
        ];

        assert!(is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "demo_product/device-a"
        ));
        assert!(is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "demo_product/device-a/logs"
        ));
        assert!(is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "public"
        ));
        assert!(is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "public/logs"
        ));
    }

    #[test]
    fn file_upload_directory_wildcard_denies_prefix_only_matches() {
        let rules = vec!["${productId}/${deviceId}/*".to_string()];

        assert!(!is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "demo_product/device-ab"
        ));
        assert!(!is_file_upload_directory_allowed(
            &rules,
            "demo_product",
            "device-a",
            "demo_product/device-b"
        ));
    }

    // Admin endpoints (no product/device context) reuse the same helper with
    // empty substitution. The rule is static, but `/*` boundary semantics must
    // match the device-side path-segment boundary (e.g. `ota/*` allows base
    // `ota` itself, and `ota/child`, but not `ota-other`). See P0-3 audit fix.
    #[test]
    fn file_upload_directory_admin_static_rule_uses_segment_boundary() {
        let rules = vec!["ota/*".to_string(), "firmware".to_string()];

        // base directory itself is allowed by `/*`
        assert!(is_file_upload_directory_allowed(&rules, "", "", "ota"));
        // child directories are allowed
        assert!(is_file_upload_directory_allowed(&rules, "", "", "ota/v1"));
        assert!(is_file_upload_directory_allowed(
            &rules,
            "",
            "",
            "ota/v1/bin"
        ));
        // exact-match rule (non-wildcard)
        assert!(is_file_upload_directory_allowed(&rules, "", "", "firmware"));
        // prefix-only match must be rejected (this is the bug the inline
        // admin implementation had: `ends_with('*')` -> `starts_with("ota/")`
        // would have accepted `ota-other` because it shares a textual prefix).
        assert!(!is_file_upload_directory_allowed(
            &rules,
            "",
            "",
            "ota-other"
        ));
        assert!(!is_file_upload_directory_allowed(&rules, "", "", "public"));
    }
}

/// Load an event validation schema for (product_id, event_identifier) by
/// reading from the schema cache first and falling back to the database.
/// Returns Ok(None) when no Active template exists for this event, in which
/// case the caller skips validation (matching `property_post`'s semantics for
/// absent schemas).
async fn load_event_validator(
    app_state: &AppState,
    product_id: &str,
    event_identifier: &str,
) -> Result<Option<Validator>, ApiError> {
    let cache_key = format!("event:{product_id}:{event_identifier}");
    let cached = app_state.cache.get(&cache_key).await.map_err(|e| {
        error!("Cache error while loading event schema: {}", e);
        ApiError::internal("Cache error")
    })?;

    if let Some(schema) = cached {
        let validator = compile_schema(&schema).map_err(|e| {
            error!("Failed to compile event schema from cache: {}", e);
            ApiError::internal("Schema compilation failed")
        })?;
        return Ok(Some(validator));
    }

    let template = app_state
        .db
        .get_event_valid_template(product_id, event_identifier)
        .await
        .map_err(|e| {
            error!("Database error while getting event schema: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    match template {
        Some(template) => {
            let validator = compile_schema(&template.schema).map_err(|e| {
                error!("Failed to compile event schema: {}", e);
                ApiError::internal("Schema compilation failed")
            })?;
            // Async cache populate, mirroring property_post's pattern.
            let cache_clone = app_state.cache.clone();
            let key_clone = cache_key;
            let schema_to_cache = template.schema.clone();
            tokio::spawn(async move {
                if let Err(e) = cache_clone.set(key_clone, schema_to_cache).await {
                    error!("Failed to cache event schema: {}", e);
                }
            });
            Ok(Some(validator))
        }
        None => Ok(None),
    }
}

fn resolve_device_identity(
    product_id: &mut String,
    device_id: &mut String,
    username: &str,
    client_id: Option<&str>,
) -> Result<(), ApiError> {
    if product_id.is_empty() {
        *product_id = username.to_string();
    }
    if device_id.is_empty()
        && let Some(cid) = client_id
    {
        *device_id = cid.to_string();
    }
    if product_id.is_empty() || device_id.is_empty() {
        return Err(ApiError::bad_request("Invalid device identity"));
    }
    validate_identifier(product_id, "product_id")?;
    validate_identifier(device_id, "device_id")?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/device/connect",
    tag = "device",
    request_body = DeviceConnectRequest,
    responses(
        (status = 204, description = "Device connection stored"),
        (status = 500, description = "Server error")
    )
)]
pub async fn device_connect(
    State(state): State<Arc<ApiState>>,
    Json(mut req): Json<DeviceConnectRequest>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;
    resolve_device_identity(
        &mut req.product_id,
        &mut req.device_id,
        req.username.as_deref().unwrap_or(""),
        req.client_id.as_deref(),
    )?;
    info!("Device connected: {}", req.device_id);

    app_state
        .db
        .upsert_device_status_connect(&req)
        .await
        .map_err(|e| {
            error!("Database error on device connect: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    // 异步触发规则评估（不阻塞主流程）
    let admin = Arc::clone(&state.admin);
    let task_set = admin.task_set.clone();
    let trigger_product_id = req.product_id.clone();
    let trigger_device_id = req.device_id.clone();
    task_set.lock().await.spawn(async move {
        let alarm_repo = admin.db.alarm();
        let rule_cache = admin.rule_cache.clone();
        let ctx = TriggerContext {
            product_id: trigger_product_id,
            device_id: trigger_device_id,
            trigger_type: TriggerType::DeviceOnline,
            trigger_value: json!({}),
        };
        evaluate_and_trigger(ctx, alarm_repo, rule_cache, None).await;
    });

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/device/disconnect",
    tag = "device",
    request_body = DeviceDisconnectRequest,
    responses(
        (status = 204, description = "Device disconnection stored"),
        (status = 500, description = "Server error")
    )
)]
pub async fn device_disconnect(
    State(state): State<Arc<ApiState>>,
    Json(mut req): Json<DeviceDisconnectRequest>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;
    resolve_device_identity(
        &mut req.product_id,
        &mut req.device_id,
        &req.username,
        req.client_id.as_deref(),
    )?;
    info!("Device disconnected: {}", req.device_id);

    app_state
        .db
        .update_device_status_disconnect(&req)
        .await
        .map_err(|e| {
            error!("Database error on device disconnect: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    // 异步触发规则评估（不阻塞主流程）
    let admin = Arc::clone(&state.admin);
    let task_set = admin.task_set.clone();
    let trigger_product_id = req.product_id.clone();
    let trigger_device_id = req.device_id.clone();
    task_set.lock().await.spawn(async move {
        let alarm_repo = admin.db.alarm();
        let rule_cache = admin.rule_cache.clone();
        let ctx = TriggerContext {
            product_id: trigger_product_id,
            device_id: trigger_device_id,
            trigger_type: TriggerType::DeviceOffline,
            trigger_value: json!({}),
        };
        evaluate_and_trigger(ctx, alarm_repo, rule_cache, None).await;
    });

    Ok(StatusCode::NO_CONTENT)
}
