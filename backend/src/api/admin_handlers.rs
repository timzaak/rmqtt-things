use crate::api::ApiState;
use crate::api::admin_models::*;
use crate::api::error::ApiError;
use crate::api::handlers::is_file_upload_directory_allowed;
use crate::api::shadow::compute_delta;
use crate::api::utils::{
    send_property_command_to_device, validate_identifier, validate_version_format,
};
use crate::api::web_models::{FileUploadRequest, FileUploadResponse};
use crate::cache::SchemaCacheManager;
use crate::db::database::ACTIVE_TEMPLATE_SCHEMA_ERR;
use crate::db::models::{CommandSource, OtaVersion};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::Query;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::{error, info, warn};

// GET /api/admin/file/download-url - Get a presigned S3 download URL for a file
// attachment (design §4.2.2 F / §5.5). The frontend uses this to render
// `FileAttachment.fileKey` values as clickable direct links.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
pub struct FileDownloadUrlQuery {
    /// S3 object key (same value as `FileAttachment.fileKey`). Non-empty, ≤ 1024 chars,
    /// must not start with `/` and must not contain a `..` path segment.
    #[serde(rename = "fileKey")]
    pub file_key: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct FileDownloadUrlResponse {
    pub url: String,
    #[serde(rename = "expiresInSeconds")]
    pub expires_in_seconds: u64,
}

/// Validate an admin file download `file_key` against the path-traversal rules
/// in design §4.5. Extracted as a pure function so the validation matrix
/// (empty / overlong / `..` segment / absolute path / valid) is unit-testable
/// without spinning up a full router.
fn validate_file_key(file_key: &str) -> Result<(), ApiError> {
    if file_key.is_empty()
        || file_key.len() > 1024
        || file_key.starts_with('/')
        || file_key.split('/').any(|seg| seg == "..")
    {
        return Err(ApiError::bad_request("Invalid fileKey"));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/admin/file/download-url",
    tag = "admin",
    params(FileDownloadUrlQuery),
    responses(
        (status = 200, description = "Presigned download URL", body = FileDownloadUrlResponse),
        (status = 400, description = "Invalid fileKey"),
        (status = 403, description = "Directory not allowed"),
        (status = 503, description = "S3 client not configured")
    )
)]
pub async fn admin_file_download_url_handler(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FileDownloadUrlQuery>,
) -> Result<Json<FileDownloadUrlResponse>, ApiError> {
    // §4.5 path-traversal protection. `validate_identifier` cannot be reused
    // because a file key legitimately contains `/` (directory segments).
    validate_file_key(&query.file_key)?;

    // No `/` ⇒ the whole key is the filename and directory is empty (same
    // convention as `admin_file_upload_handler`, which passes empty
    // product_id / device_id in the admin context).
    let directory = query
        .file_key
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("");

    let s3_client = state
        .admin
        .s3_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("S3 client not configured"))?;

    // §4.5 directory whitelist — prevents reading outside the configured S3
    // prefixes. Admin context has no product/device variables, matching
    // `admin_file_upload_handler`.
    if !is_file_upload_directory_allowed(&s3_client.config.directories, "", "", directory) {
        return Err(ApiError::forbidden_with("Directory not allowed"));
    }

    let url = s3_client
        .get_presigned_download_url(&query.file_key)
        .await
        .map_err(|e| {
            error!("Failed to get presigned download url: {}", e);
            ApiError::internal("Failed to get presigned download url")
        })?;

    // `S3Config.expired_seconds` is `u32`; the API contract exposes `u64`
    // (design §4.2.2 F) so widen here.
    Ok(Json(FileDownloadUrlResponse {
        url,
        expires_in_seconds: u64::from(s3_client.config.expired_seconds),
    }))
}

// POST /api/admin/file/upload - Upload file
#[utoipa::path(
    post,
    path = "/api/admin/file/upload",
    tag = "admin",
    request_body = FileUploadRequest,
    responses((status = 200, description = "Upload policy", body = FileUploadResponse))
)]
pub async fn admin_file_upload_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<FileUploadRequest>,
) -> Result<Json<FileUploadResponse>, ApiError> {
    let state = &state.admin;
    if let Some(s3_client) = state.s3_client.as_ref() {
        // 复用设备端统一的目录白名单校验，确保两端 `/*` 边界语义一致
        // （路径段边界）。管理端目录规则为静态字符串，无 `${productId}` /
        // `${deviceId}` 变量，因此 product_id / device_id 传空串——变量替换
        // 对无变量的规则无影响。See file-upload.md 与 P0-3 audit fix.
        if !is_file_upload_directory_allowed(&s3_client.config.directories, "", "", &req.directory)
        {
            return Err(ApiError::bad_request("Directory not allowed"));
        }

        let file_name = if req.use_origin_name {
            req.file_name.clone()
        } else {
            format!("{}_{}", uuid::Uuid::new_v4(), req.file_name)
        };
        let file_path = format!("{}/{}", req.directory, file_name);

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
        Ok(Json(response_data))
    } else {
        Err(ApiError::internal("S3 client not configured"))
    }
}
// GET /admin/property/command - 查询属性命令
#[utoipa::path(
    get,
    path = "/api/admin/property/command",
    tag = "admin",
    params(PropertyCommandQuery),
    responses((status = 200, description = "Property commands", body = PropertyCommandListResponse))
)]
pub async fn get_property_commands(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<PropertyCommandQuery>,
) -> Result<Json<PropertyCommandListResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let (commands, total) = state
        .db
        .query_property_commands(
            &query.product_id,
            query.device_id.as_deref(),
            query.status,
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = PropertyCommandListResponse {
        data: commands,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/admin/valid/event",
    tag = "admin",
    params(EventValidTemplateQuery),
    responses((status = 200, description = "Validation templates", body = EventValidTemplateListResponse))
)]
pub async fn get_event_valid_templates(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<EventValidTemplateQuery>,
) -> Result<Json<EventValidTemplateListResponse>, ApiError> {
    let state = &state.admin;
    if let Some(ref pid) = query.product_id {
        validate_identifier(pid, "product_id")?;
    }

    let (templates, total) = state
        .db
        .query_event_valid_templates(
            query.product_id.as_deref(),
            query.event.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = EventValidTemplateListResponse {
        data: templates,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/admin/valid/event/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Template id")),
    responses((status = 200, description = "Validation template", body = crate::db::models::EventValidTemplate))
)]
pub async fn get_event_valid_template(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<crate::db::models::EventValidTemplate>, ApiError> {
    let state = &state.admin;

    let template = state
        .db
        .get_event_valid_template_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    match template {
        Some(template) => Ok(Json(template)),
        None => Err(ApiError::not_found("Template not found")),
    }
}

// Create a new event or property validation template.
// For property schemas, the `event` field should be set to `property`.
#[utoipa::path(
    post,
    path = "/api/admin/valid/event",
    tag = "admin",
    request_body = CreateEventValidTemplateRequest,
    responses((status = 201, description = "Validation template created"))
)]
pub async fn create_event_valid_template(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateEventValidTemplateRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    validate_identifier(&request.product_id, "product_id")?;
    jsonschema::meta::validate(&request.schema)
        .map_err(|e| ApiError::bad_request(format!("Invalid JSON schema: {e}")))?;

    state
        .db
        .insert_event_valid_template(
            &request.product_id,
            &request.event,
            request.description.as_deref(),
            &request.schema,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    Ok(StatusCode::CREATED)
}

#[utoipa::path(
    patch,
    path = "/api/admin/valid/event/{id}/status",
    tag = "admin",
    params(("id" = i64, Path, description = "Template id")),
    request_body = UpdateEventValidTemplateStatusRequest,
    responses((status = 200, description = "Validation template status updated"))
)]
pub async fn update_event_valid_template_status(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateEventValidTemplateStatusRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    let template_to_update = state
        .db
        .get_event_valid_template_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let rows_affected = state
        .db
        .update_event_valid_template_status(id, request.status)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected > 0 {
        if let Some(template) = template_to_update.as_ref()
            && template.event == "property"
        {
            if let Err(e) = state.cache._remove(&template.product_id).await {
                error!("Failed to remove schema from cache: {}", e);
            } else {
                info!(
                    "Property schema for product_id {} removed from cache",
                    template.product_id
                );
            }
        }
        if let Some(template) = template_to_update.as_ref()
            && template.event != "property"
        {
            // Event (non-property) templates are cached under
            // `event:{product_id}:{event}` (see handlers::load_event_validator).
            // A status change can flip Active<->Inactive, so the cached schema
            // must be dropped or event_post keeps validating against the old
            // state (PRD validation-template.md §4.2 "模板状态变更或更新时清除缓存").
            let cache_key = format!("event:{}:{}", template.product_id, template.event);
            if let Err(e) = state.cache._remove(&cache_key).await {
                error!("Failed to remove event schema from cache: {}", e);
            } else {
                info!(
                    "Event schema for {}:{} removed from cache after status change",
                    template.product_id, template.event
                );
            }
        }
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::not_found("Template not found"))
    }
}

#[utoipa::path(
    patch,
    path = "/api/admin/valid/event/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Template id")),
    request_body = UpdateEventValidTemplateRequest,
    responses((status = 200, description = "Validation template updated"))
)]
pub async fn update_event_valid_template(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateEventValidTemplateRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    let template_to_update = state
        .db
        .get_event_valid_template_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if let Some(schema) = &request.schema {
        jsonschema::meta::validate(schema)
            .map_err(|e| ApiError::bad_request(format!("Invalid JSON schema: {e}")))?;
    }

    let rows_affected = state
        .db
        .update_event_valid_template(id, request.schema.as_ref(), request.description.as_deref())
        .await
        .map_err(|e| {
            if e.to_string().contains(ACTIVE_TEMPLATE_SCHEMA_ERR) {
                return ApiError::bad_request(e.to_string());
            }
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected > 0 {
        if let Some(template) = template_to_update.as_ref()
            && template.event == "property"
        {
            if let Err(e) = state.cache._remove(&template.product_id).await {
                error!("Failed to remove schema from cache: {}", e);
            } else {
                info!(
                    "Property schema for product_id {} removed from cache",
                    template.product_id
                );
            }
        }
        if let Some(template) = template_to_update.as_ref()
            && template.event != "property"
        {
            // Mirrors the property branch: an Active event schema is cached
            // under `event:{product_id}:{event}` and must be invalidated on
            // update so event_post re-reads the new schema from the DB.
            let cache_key = format!("event:{}:{}", template.product_id, template.event);
            if let Err(e) = state.cache._remove(&cache_key).await {
                error!("Failed to remove event schema from cache: {}", e);
            } else {
                info!(
                    "Event schema for {}:{} removed from cache after update",
                    template.product_id, template.event
                );
            }
        }
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::not_found(
            "Template not found or cannot be updated",
        ))
    }
}

// DELETE /api/admin/valid/event/{id} - Delete a validation template.
//
// Design decision (P1-2): templates of any status (Draft / Active / Inactive)
// may be deleted. Active-state uniqueness is enforced by the DB layer on
// promotion, so deleting an Active template simply leaves the
// (product_id, event) pair without a template — equivalent to "no validation"
// for that pair, matching the documented behavior for absent templates
// (validation-template.md §3.2). When the deleted template was an Active
// property schema, the in-memory schema cache entry for that product is
// invalidated so the next property_post re-queries the DB.
#[utoipa::path(
    delete,
    path = "/api/admin/valid/event/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Template id")),
    responses(
        (status = 200, description = "Validation template deleted"),
        (status = 404, description = "Template not found")
    )
)]
pub async fn delete_event_valid_template(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;

    // Fetch first so we can invalidate the schema cache when deleting an
    // Active property template (mirrors update_*_template_status).
    let template = state
        .db
        .get_event_valid_template_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let rows_affected = state
        .db
        .delete_event_valid_template(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected == 0 {
        return Err(ApiError::not_found("Template not found"));
    }

    // Borrow once so both the property and event cache-invalidation branches
    // can read template fields without moving the Option.
    if let Some(template) = template.as_ref()
        && template.event == "property"
    {
        if let Err(e) = state.cache._remove(&template.product_id).await {
            error!("Failed to remove schema from cache: {}", e);
        } else {
            info!(
                "Property schema for product_id {} removed from cache after delete",
                template.product_id
            );
        }
    }
    if let Some(template) = template.as_ref()
        && template.event != "property"
    {
        // An Active event schema may have been cached under
        // `event:{product_id}:{event}`; drop it so event_post re-queries the
        // DB and correctly treats the (product, event) pair as unvalidated
        // after the template is removed.
        let cache_key = format!("event:{}:{}", template.product_id, template.event);
        if let Err(e) = state.cache._remove(&cache_key).await {
            error!("Failed to remove event schema from cache: {}", e);
        } else {
            info!(
                "Event schema for {}:{} removed from cache after delete",
                template.product_id, template.event
            );
        }
    }

    Ok(StatusCode::OK)
}

// POST /admin/property/command - 创建属性命令
#[utoipa::path(
    post,
    path = "/api/admin/property/command",
    tag = "admin",
    request_body = CreatePropertyCommandRequest,
    responses((status = 201, description = "Property command created"))
)]
pub async fn create_property_command(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreatePropertyCommandRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    validate_identifier(&request.product_id, "product_id")?;
    validate_identifier(&request.device_id, "device_id")?;

    if request.command.is_null() {
        return Err(ApiError::bad_request("command cannot be null"));
    }

    // 插入命令到数据库
    state
        .db
        .insert_property_command(
            &request.product_id,
            &request.device_id,
            &request.command,
            CommandSource::OneShot,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    // 检查设备是否订阅了属性设置主题
    match state
        .rmqtt_client
        .is_subscribed_to_properties(&request.product_id, &request.device_id)
        .await
    {
        Ok(is_subscribed) => {
            if is_subscribed {
                info!(
                    "Device {} is subscribed to properties, sending command immediately",
                    request.device_id
                );

                // 设备在线，立即发送命令
                if let Err(e) = send_property_command_to_device(
                    &state.db,
                    &state.rmqtt_client,
                    &request.product_id,
                    &request.device_id,
                )
                .await
                {
                    error!(
                        "Failed to send command to device {}: {}",
                        request.device_id, e
                    );
                    // 即使发送失败，命令已经保存到数据库，设备下次上线时会收到
                }
            } else {
                info!(
                    "Device {} is not subscribed to properties, command will be sent when device comes online",
                    request.device_id
                );
            }
        }
        Err(e) => {
            warn!(
                "Failed to check device {} subscription status: {}, command will be sent when device comes online",
                request.device_id, e
            );
        }
    }

    Ok(StatusCode::CREATED)
}

// PUT /admin/property/shadow/desired - Set-Desired：upsert desired，计算 delta，
// 非空则借命令通道投递（设计 shadow-device-support.md §5.2）。
#[utoipa::path(
    put,
    path = "/api/admin/property/shadow/desired",
    tag = "admin",
    request_body = SetDesiredRequest,
    responses((status = 200, description = "Desired state updated", body = SetDesiredResponse))
)]
pub async fn set_property_desired(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SetDesiredRequest>,
) -> Result<Json<SetDesiredResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&request.product_id, "product_id")?;
    validate_identifier(&request.device_id, "device_id")?;

    // 空对象 patch 无任何操作，返回 400（US-PA-042 场景 4）。
    // 注：patch 只含 null（如 `{"key": null}`）合法——它有删除操作，
    // 合并后文档即使变空也不返回 400。
    if request.desired.is_empty() {
        return Err(ApiError::bad_request(
            "desired must be a non-empty JSON object",
        ));
    }

    // RFC 7396 子集合并 upsert，返回合并后的完整 desired 文档。
    let merged = state
        .db
        .upsert_property_desired(&request.product_id, &request.device_id, &request.desired)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let reported_row = state
        .db
        .get_property_latest_one(&request.product_id, &request.device_id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let reported_map = reported_row
        .as_ref()
        .and_then(|r| r.properties.as_object())
        .cloned()
        .unwrap_or_default();

    let desired_map = merged.desired.as_object().cloned().unwrap_or_default();
    let delta_map = compute_delta(&desired_map, &reported_map);

    // delta 非空则入 Pending + 在线投递；离线/查询失败留队。pushed = delta 非空。
    let pushed = !delta_map.is_empty();
    let delta = JsonValue::Object(delta_map);
    if pushed {
        state
            .db
            .insert_property_command(
                &request.product_id,
                &request.device_id,
                &delta,
                CommandSource::DesiredDelta,
            )
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ApiError::internal("Database operation failed")
            })?;

        match state
            .rmqtt_client
            .is_subscribed_to_properties(&request.product_id, &request.device_id)
            .await
        {
            Ok(is_subscribed) => {
                if is_subscribed {
                    info!(
                        "Device {} is subscribed to properties, sending desired delta immediately",
                        request.device_id
                    );
                    // 在线则排空 Pending 下发；发送失败仅日志，命令留 DB
                    // （参照 create_property_command，不返回错误）。
                    if let Err(e) = send_property_command_to_device(
                        &state.db,
                        &state.rmqtt_client,
                        &request.product_id,
                        &request.device_id,
                    )
                    .await
                    {
                        error!(
                            "Failed to send desired delta to device {}: {}",
                            request.device_id, e
                        );
                    }
                } else {
                    info!(
                        "Device {} is not subscribed to properties, desired delta will be sent when device comes online",
                        request.device_id
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Failed to check device {} subscription status: {}, desired delta will be sent when device comes online",
                    request.device_id, e
                );
            }
        }
    }

    Ok(Json(SetDesiredResponse {
        desired: merged.desired,
        delta,
        pushed,
    }))
}

// GET /admin/property/shadow - Get-Delta：返回 desired / reported / delta
// （设计 shadow-device-support.md §5.2）。
#[utoipa::path(
    get,
    path = "/api/admin/property/shadow",
    tag = "admin",
    params(ShadowQuery),
    responses((status = 200, description = "Shadow view", body = ShadowView))
)]
pub async fn get_property_shadow(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ShadowQuery>,
) -> Result<Json<ShadowView>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    validate_identifier(&query.device_id, "device_id")?;

    let desired_row = state
        .db
        .get_property_desired(&query.product_id, &query.device_id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let desired = desired_row
        .as_ref()
        .map(|d| d.desired.clone())
        .unwrap_or(JsonValue::Object(serde_json::Map::new()));
    let desired_updated_time = desired_row.as_ref().map(|d| d.updated_time);

    let reported_row = state
        .db
        .get_property_latest_one(&query.product_id, &query.device_id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let reported = reported_row
        .as_ref()
        .map(|r| r.properties.clone())
        .unwrap_or(JsonValue::Object(serde_json::Map::new()));
    let reported_updated_time = reported_row.as_ref().map(|r| r.updated_time);

    let desired_map = desired.as_object().cloned().unwrap_or_default();
    let reported_map = reported.as_object().cloned().unwrap_or_default();
    let delta = JsonValue::Object(compute_delta(&desired_map, &reported_map));

    Ok(Json(ShadowView {
        desired,
        reported,
        delta,
        desired_updated_time,
        reported_updated_time,
    }))
}

// DELETE /admin/property/command - 删除属性命令
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
pub struct DeletePropertyCommandsQuery {
    pub ids: Vec<i64>,
}

#[utoipa::path(
    delete,
    path = "/api/admin/property/command",
    tag = "admin",
    params(DeletePropertyCommandsQuery),
    responses((status = 200, description = "Property commands deleted"))
)]
pub async fn delete_property_commands(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<DeletePropertyCommandsQuery>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;

    info!("Deleting property commands with ids: {:?}", query.ids);

    state
        .db
        .delete_property_commands(&query.ids)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    Ok(StatusCode::OK)
}

// GET /admin/property - 查询最新属性
#[utoipa::path(
    get,
    path = "/api/admin/property",
    tag = "admin",
    params(CommonQuery),
    responses((status = 200, description = "Latest properties", body = PropertyLatestListResponse))
)]
pub async fn get_property_latest(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommonQuery>,
) -> Result<Json<PropertyLatestListResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let properties = state
        .db
        .query_property_latest(
            &query.product_id,
            query.device_id.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = PropertyLatestListResponse {
        data: properties,
        pagination: SimplePaginationInfo {
            page: query.page,
            page_size: query.page_size,
        },
    };

    Ok(Json(response))
}

// GET api/admin/device/status - 查询设备状态
#[utoipa::path(
    get,
    path = "/api/admin/device/status",
    tag = "admin",
    params(CommonQuery2),
    responses((status = 200, description = "Device statuses", body = DeviceStatusListResponse))
)]
pub async fn get_device_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommonQuery2>,
) -> Result<Json<DeviceStatusListResponse>, ApiError> {
    let state = &state.admin;
    if let Some(ref pid) = query.product_id {
        validate_identifier(pid, "product_id")?;
    }
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let (devices, total) = state
        .db
        .query_device_status(
            query.product_id.as_deref(),
            query.device_id.as_deref(),
            query.status,
            query.registration_source,
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let response = DeviceStatusListResponse {
        data: devices,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

// GET api/admin/device/status/history - 查询设备历史状态
#[utoipa::path(
    get,
    path = "/api/admin/device/status/history",
    tag = "admin",
    params(CommonQuery),
    responses((status = 200, description = "Device status history", body = DeviceStatusHistoryListResponse))
)]
pub async fn get_device_status_history(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommonQuery>,
) -> Result<Json<DeviceStatusHistoryListResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let history = state
        .db
        .query_device_status_history(
            &query.product_id,
            query.device_id.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = DeviceStatusHistoryListResponse {
        data: history,
        pagination: SimplePaginationInfo {
            page: query.page,
            page_size: query.page_size,
        },
    };

    Ok(Json(response))
}

// GET api/admin/property/history - 查询属性历史
#[utoipa::path(
    get,
    path = "/api/admin/property/history",
    tag = "admin",
    params(CommonQuery),
    responses((status = 200, description = "Property history", body = PropertyHistoryListResponse))
)]
pub async fn get_property_history(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommonQuery>,
) -> Result<Json<PropertyHistoryListResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let properties = state
        .db
        .query_property_history(
            &query.product_id,
            query.device_id.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = PropertyHistoryListResponse {
        data: properties,
        pagination: SimplePaginationInfo {
            page: query.page,
            page_size: query.page_size,
        },
    };

    Ok(Json(response))
}

// GET api/admin/event - 查询事件历史
#[utoipa::path(
    get,
    path = "/api/admin/event",
    tag = "admin",
    params(CommonQuery),
    responses((status = 200, description = "Event history", body = EventHistoryListResponse))
)]
pub async fn get_event_history(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommonQuery>,
) -> Result<Json<EventHistoryListResponse>, ApiError> {
    let state = &state.admin;
    validate_identifier(&query.product_id, "product_id")?;
    if let Some(ref did) = query.device_id {
        validate_identifier(did, "device_id")?;
    }

    let events = state
        .db
        .query_event_history(
            &query.product_id,
            query.device_id.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let response = EventHistoryListResponse {
        data: events,
        pagination: SimplePaginationInfo {
            page: query.page,
            page_size: query.page_size,
        },
    };

    Ok(Json(response))
}

// GET /api/admin/ota/version - Get OTA version list
#[utoipa::path(
    get,
    path = "/api/admin/ota/version",
    tag = "admin",
    params(OtaVersionQuery),
    responses((status = 200, description = "OTA versions", body = OtaVersionListResponse))
)]
pub async fn get_ota_versions(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<OtaVersionQuery>,
) -> Result<Json<OtaVersionListResponse>, ApiError> {
    let state = &state.admin;
    if let Some(ref pid) = query.product_id {
        validate_identifier(pid, "product_id")?;
    }
    let (versions, total) = state
        .db
        .ota()
        .query_ota_versions(query.product_id.as_deref(), query.page, query.page_size)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let response = OtaVersionListResponse {
        data: versions,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

// GET /api/admin/ota/version/{id} - Get OTA version details
#[utoipa::path(
    get,
    path = "/api/admin/ota/version/{id}",
    tag = "admin",
    params(("id" = i32, Path, description = "OTA version id")),
    responses((status = 200, description = "OTA version", body = OtaVersion))
)]
pub async fn get_ota_version(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i32>,
) -> Result<Json<OtaVersion>, ApiError> {
    let state = &state.admin;
    let version = state
        .db
        .ota()
        .get_ota_version_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    match version {
        Some(version) => Ok(Json(version)),
        None => Err(ApiError::not_found("Version not found")),
    }
}

// POST /api/admin/ota/version - Create OTA version
#[utoipa::path(
    post,
    path = "/api/admin/ota/version",
    tag = "admin",
    request_body = CreateOtaVersionRequest,
    responses((status = 201, description = "OTA version created"))
)]
pub async fn create_ota_version(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateOtaVersionRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    validate_identifier(&req.product_id, "product_id")?;
    // `key` becomes part of the OTA object path / device lookup; reject empty,
    // overlong, or path-traversal characters up front (mirrors product_id /
    // device_id validation elsewhere in this module).
    validate_identifier(&req.key, "key")?;
    validate_version_format(&req.version, "version")?;
    validate_version_format(&req.min_version, "min_version")?;
    if let Some(ref max_ver) = req.max_version {
        validate_version_format(max_ver, "max_version")?;
    }
    state.db.ota().create_ota_version(&req).await.map_err(|e| {
        error!("Database error: {}", e);
        ApiError::internal("Database operation failed")
    })?;
    Ok(StatusCode::CREATED)
}

// PUT /api/admin/ota/version/{id} - Update OTA version
#[utoipa::path(
    put,
    path = "/api/admin/ota/version/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "OTA version id")),
    request_body = UpdateOtaVersionRequest,
    responses((status = 200, description = "OTA version updated"))
)]
pub async fn update_ota_version(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateOtaVersionRequest>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    if let Some(ref min_ver) = req.min_version {
        validate_version_format(min_ver, "min_version")?;
    }
    if let Some(ref max_ver) = req.max_version {
        validate_version_format(max_ver, "max_version")?;
    }
    let rows_affected = state
        .db
        .ota()
        .update_ota_version(id, &req)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected > 0 {
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::not_found("Version not found or no changes made"))
    }
}

// DELETE /api/admin/ota/version/{id} - Delete OTA version
#[utoipa::path(
    delete,
    path = "/api/admin/ota/version/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "OTA version id")),
    responses((status = 200, description = "OTA version deleted"))
)]
pub async fn delete_ota_version(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;
    let rows_affected = state.db.ota().delete_ota_version(id).await.map_err(|e| {
        error!("Database error: {}", e);
        ApiError::internal("Database operation failed")
    })?;
    if rows_affected > 0 {
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::not_found("Version not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_file_key;
    use crate::api::error::ApiError;
    use axum::http::StatusCode;

    // Asserts the validator rejects with a 400 BAD_REQUEST rather than another
    // status — `admin_file_download_url_handler` relies on this mapping for the
    // §4.2.2 F 400 response contract.
    fn assert_bad_request(result: Result<(), ApiError>) {
        match result {
            Err(e) => assert_eq!(
                e.status_code(),
                StatusCode::BAD_REQUEST,
                "expected 400 for invalid fileKey"
            ),
            Ok(_) => panic!("expected rejection, got Ok"),
        }
    }

    #[test]
    fn validate_file_key_rejects_empty() {
        assert_bad_request(validate_file_key(""));
    }

    #[test]
    fn validate_file_key_rejects_overlong() {
        // Exactly 1025 chars — one past the limit.
        let overlong = "a".repeat(1025);
        assert_bad_request(validate_file_key(&overlong));
    }

    #[test]
    fn validate_file_key_rejects_dotdot_segment() {
        assert_bad_request(validate_file_key("foo/../bar"));
        // Trailing `..` is also a path segment and must be rejected.
        assert_bad_request(validate_file_key("foo/.."));
    }

    #[test]
    fn validate_file_key_rejects_absolute_path() {
        assert_bad_request(validate_file_key("/etc/passwd"));
    }

    #[test]
    fn validate_file_key_accepts_valid_keys() {
        // Plain filename, no directory.
        assert!(validate_file_key("report.pdf").is_ok());
        // Nested directories with a normal filename — a realistic attachment key.
        assert!(validate_file_key("factory/attachments/report.pdf").is_ok());
        // Exactly 1024 chars is the upper bound and must pass.
        let max = "a".repeat(1024);
        assert!(validate_file_key(&max).is_ok());
    }
}
