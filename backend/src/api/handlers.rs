use crate::api::utils::{
    extract_and_validate_product_id, extract_event_identifier_from_topic,
    extract_service_type_from_topic, send_action_invocations_to_device,
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
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{debug, error, info, warn};

use crate::api::ApiState;
use crate::api::error::ApiError;
use crate::rule_engine::{TriggerContext, TriggerType, evaluate_and_trigger};

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
                Some(compile_schema(&schema).map_err(|e| {
                    error!("Failed to compile schema from cache: {}", e);
                    ApiError::internal("Schema compilation failed")
                })?)
            } else {
                // 从数据库获取 schema
                match app_state
                    .db
                    .get_property_schema(&product_id)
                    .await
                    .map_err(|e| {
                        error!("Database error while getting schema: {}", e);
                        ApiError::internal("Database operation failed")
                    })? {
                    Some(schema_template) => {
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
                            if let Err(e) = cache_clone.set(product_id_clone, schema_to_cache).await
                            {
                                error!("Failed to cache schema: {}", e);
                            }
                        });
                        Some(validator)
                    }
                    None => {
                        // 无 Active 模板时放行（thing-model-extension 设计 §8 /
                        // §414）：总开关只控制是否启用校验流程，无模板时不拒绝，
                        // 与 event_post 的"无 schema 即放行"语义一致。
                        debug!(
                            "No property schema template for product_id={}, accepting",
                            product_id
                        );
                        None
                    }
                }
            };

            // 验证属性（仅当存在模板时）
            if let Some(validator) = validator {
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
    // Unified event dispatch (thing-model-extension design §5.2): the single
    // wildcard rule `+/+/thing/event/+/post` routes every event publish here.
    // When the `event_type` topic segment is `property`, delegate to
    // `property_post` so its full side-effects are preserved (thing schema
    // validation, `upsert_property_latest` snapshot, and `TriggerType::Property`
    // rule evaluation). This MUST NOT be reduced to "only snapshot the value" —
    // doing so would drop validation and the property rule trigger.
    if extract_event_identifier_from_topic(&mqtt_msg.topic).as_deref() == Some("property") {
        return property_post(State(state.clone()), Json(mqtt_msg)).await;
    }

    let app_state = &state.app;

    let payload = mqtt_msg.decode_payload_as_json().map_err(|e| {
        error!("Failed to decode payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;
    let timestamp = OffsetDateTime::now_utc();
    let events = payload.params.unwrap_or(JsonValue::Null);
    // Spec §消息格式:事件上报的 params 必须是 object。
    if !events.is_object() {
        return Err(ApiError::bad_request("Invalid params format"));
    }

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
            id: payload.id.clone(),
            code: 200,
            data: Some(json!(response_data)),
        };

        let response_payload = serde_json::to_string(&response).map_err(|e| {
            error!("Failed to serialize response: {}", e);
            ApiError::internal("Failed to serialize response")
        })?;

        // Ack gating (design §5.3 / §1.5): only publish the `_reply` when the
        // device asked for one (`ack == AckStatus::Yes`). Matches the pattern
        // in ota_handlers.rs and event_post.
        if payload.ack == AckStatus::Yes
            && let Err(e) = state
                .rmqtt_client
                .publish_response(&mqtt_msg.topic, &response_payload)
                .await
        {
            error!("Failed to publish response: {}", e);
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        warn!("does not support file upload");
        // Object-data contract (design §5.3 / §1.5): error responses carry a
        // `{message}` object, NOT a bare string. The previous `json!("…")`
        // emitted a JSON string and violated spec.
        let response = MqttResponse {
            id: payload.id.clone(),
            code: 503,
            data: Some(json!({"message": "do not support file upload"})),
        };
        let response_payload = serde_json::to_string(&response).map_err(|e| {
            error!("Failed to serialize response: {}", e);
            ApiError::internal("Failed to serialize response")
        })?;
        // Same ack gating as the success branch.
        if payload.ack == AckStatus::Yes
            && let Err(e) = state
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

// 统一服务下发结果上报接口（thing-model-extension 设计 §5.2）
//
// Replaces the deleted `property_set_reply` private-batch handler. The Broker
// forwards every `thing/service/{service_type}/set_reply` publish here via a
// single wildcard rule. The handler dispatches by the `service_type` topic
// segment and by the `{prefix}:{db_id}` correlation id encoded in the spec
// response payload's top-level `id` field:
//   - `property:{db_id}` -> `property_command` (prev_status = Sent)
//   - `action:{db_id}`   -> `action_invocation` (prev_status = Sent)
// Any 2xx `code` is treated as Success; everything else is Failed (HTTP
// semantics, design §5.2). Duplicate / unknown / non-Sent rows return 204 +
// warn so the Broker does not retry.
#[utoipa::path(
    post,
    path = "/api/thing/service/set_reply",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Reply accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn service_set_reply(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;

    validate_identifier(&mqtt_msg.client_id, "device_id")?;
    let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
    let service_type = extract_service_type_from_topic(&mqtt_msg.topic).ok_or_else(|| {
        warn!(
            "service_set_reply: could not extract service_type from topic '{}'",
            mqtt_msg.topic
        );
        ApiError::bad_request("Invalid service topic")
    })?;

    let bytes = mqtt_msg.decode_payload().map_err(|e| {
        debug!("Failed to decode base64 payload: {}", e);
        ApiError::bad_request("Invalid payload encoding")
    })?;
    let payload: MqttResponse = serde_json::from_slice(&bytes).map_err(|e| {
        debug!("Failed to parse payload JSON as MqttResponse: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    // HTTP-style 2xx success boundary (design §5.2). Not `== 200`.
    let status = if (200..=299).contains(&payload.code) {
        CommandStatus::Success
    } else {
        CommandStatus::Failed
    };

    // Parse the platform-generated correlation id `{prefix}:{db_id}`.
    let (prefix, db_id_str) = payload.id.split_once(':').ok_or_else(|| {
        warn!(
            "service_set_reply: id '{}' is not a '{{prefix}}:{{db_id}}' correlation id",
            payload.id
        );
        ApiError::bad_request("Invalid id format")
    })?;
    let db_id: i64 = db_id_str.parse().map_err(|_| {
        warn!(
            "service_set_reply: id '{}' has non-numeric db_id part '{}'",
            payload.id, db_id_str
        );
        ApiError::bad_request("Invalid id format")
    })?;

    match prefix {
        "property" => {
            // Note: the existing `update_property_command_status` takes
            // `&Vec<i64>` (database.rs). The single-id form for action is the
            // BE-D01 asymmetry; we wrap the single id rather than touch the
            // property signature (BE-D02 scope).
            app_state
                .db
                .update_property_command_status(
                    &vec![db_id],
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
                "Updated property command id={} status to {:?}",
                db_id, status
            );
        }
        "action" => {
            let affected = app_state
                .db
                .update_action_invocation_status(
                    db_id,
                    &product_id,
                    &mqtt_msg.client_id,
                    &service_type,
                    status,
                    CommandStatus::Sent,
                )
                .await
                .map_err(|e| {
                    error!("Database error: {}", e);
                    ApiError::internal("Database operation failed")
                })?;
            if affected == 0 {
                // Duplicate reply, unknown id, or row no longer Sent. Return
                // 204 + warn so the Broker does not retry the webhook.
                warn!(
                    "service_set_reply: action id={} not updated (duplicate/unknown/non-Sent)",
                    db_id
                );
            } else {
                info!(
                    "Updated action invocation id={} service_type={} status to {:?}",
                    db_id, service_type, status
                );
            }
        }
        other => {
            // Unknown correlation prefix. Return 204 + warn so the Broker does
            // not retry; this is not a client-format error worth a 4xx.
            warn!(
                "service_set_reply: unknown id prefix '{}' (id='{}')",
                other, payload.id
            );
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// 统一服务订阅触发投递接口（thing-model-extension 设计 §5.2 / §4.2.2）
//
// Replaces the deleted `property_set_subscribe` handler. The Broker fires this
// once when a device subscribes to the wildcard `thing/service/+/set` filter.
// The subscribe filter's first segment is `+`, so productId cannot be read from
// the topic; it is read from the WebHook `username`. The handler then drains
// both property and action pending commands for the device.
#[utoipa::path(
    post,
    path = "/api/thing/service/set_subscribe",
    tag = "thing",
    request_body = RMqttSubscribeMessage,
    responses(
        (status = 204, description = "Subscription accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn service_set_subscribe(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttSubscribeMessage>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.app;

    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;

    // productId comes from the WebHook username, not the topic: the device
    // subscribes to the wildcard `+/{deviceId}/thing/service/+/set`, so the
    // first topic segment is `+` and cannot serve as productId.
    let product_id = mqtt_msg.username.as_deref().unwrap_or("");
    validate_identifier(product_id, "product_id")?;

    // Validate that the topic's second segment equals the reported clientId,
    // guarding against mismatched subscribe/client identity.
    if let Some(topic) = mqtt_msg.topic.as_deref() {
        let mut parts = topic.trim_start_matches('/').split('/');
        let _first = parts.next();
        let second = parts.next();
        if second.is_none_or(|seg| seg != device_id) {
            warn!(
                "service_set_subscribe: topic '{}' second segment does not match clientId '{}'",
                topic, device_id
            );
            return Err(ApiError::bad_request("Topic does not match clientId"));
        }
    }

    info!(
        "Processing service set_subscribe for product={} device={}",
        product_id, device_id
    );

    // Subscription-readiness gate (design offline-queued-delivery-drain-timing.md
    // §4 方案 A): the `client_subscribe` webhook is dispatched by the broker
    // during CONNECT (auto-subscription), which may happen BEFORE the device's
    // own SUBACK is registered in the broker's subscription table. Publishing
    // at that moment loses the message (no matching subscriber). Mirror the
    // already-working online path (admin_handlers.rs `is_subscribed` gate):
    // query the broker's live `/subscriptions` and only drain when the device is
    // actually subscribed. If not ready, leave rows Pending — they will be
    // drained by a later trigger (device_connect fallback below, the next
    // webhook, or the online immediate-delivery path).
    //
    // The auto-subscription filter is the single wildcard
    // `+/{clientid}/thing/service/+/set` (rmqtt-auto-subscription.toml), which
    // covers every service_type including `property`. So checking the concrete
    // `property/set` topic via `is_subscribed_to_properties` is sufficient to
    // know the whole service-set filter is registered — no separate per-
    // service_type gate is needed.
    let subscribed = app_state
        .rmqtt_client
        .is_subscribed_to_properties(product_id, device_id)
        .await
        .unwrap_or(false);
    if !subscribed {
        info!(
            "service_set_subscribe: device {} not yet registered as subscribed at broker, skip drain (will retry on next trigger)",
            device_id
        );
        return Ok(StatusCode::NO_CONTENT);
    }

    // Drain property pending first, then action pending. Each helper publishes
    // its own per-row spec envelope (BE-D02 property single-row + BE-D01 action).
    if let Err(e) = send_property_command_to_device(
        &app_state.db,
        &app_state.rmqtt_client,
        product_id,
        device_id,
    )
    .await
    {
        error!(
            "Failed to send property command to device {}: {}",
            device_id, e
        );
        return Err(ApiError::internal("Failed to publish command"));
    }

    if let Err(e) = send_action_invocations_to_device(
        &app_state.db,
        &app_state.rmqtt_client,
        product_id,
        device_id,
    )
    .await
    {
        error!(
            "Failed to send action invocations to device {}: {}",
            device_id, e
        );
        return Err(ApiError::internal("Failed to publish command"));
    }

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

    // Offline-queue fallback drain (design
    // offline-queued-delivery-drain-timing.md §4 方案 A.1): the primary drain
    // trigger is `service_set_subscribe` (client_subscribe webhook), but that
    // webhook fires during CONNECT, when the device subscription may not yet be
    // registered at the broker — the gate there skips the drain in that case.
    // `client_connected` fires after CONNACK, by which time the auto-subscription
    // is usually registered, so this is a reliable fallback to drain anything
    // the primary trigger skipped. Same subscription-readiness gate as the
    // primary path; drain helpers are idempotent (atomic Pending->Sent flip via
    // FOR UPDATE SKIP LOCKED), so concurrent/ repeated calls are safe.
    let product_id = req.product_id.clone();
    let device_id = req.device_id.clone();
    let subscribed = app_state
        .rmqtt_client
        .is_subscribed_to_properties(&product_id, &device_id)
        .await
        .unwrap_or(false);
    if subscribed {
        if let Err(e) = send_property_command_to_device(
            &app_state.db,
            &app_state.rmqtt_client,
            &product_id,
            &device_id,
        )
        .await
        {
            warn!(
                "Failed to drain property commands for device {} on connect: {}",
                device_id, e
            );
        }
        if let Err(e) = send_action_invocations_to_device(
            &app_state.db,
            &app_state.rmqtt_client,
            &product_id,
            &device_id,
        )
        .await
        {
            warn!(
                "Failed to drain action invocations for device {} on connect: {}",
                device_id, e
            );
        }
    } else {
        info!(
            "device_connect: device {} not yet subscribed at broker, skip fallback drain",
            device_id
        );
    }

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
