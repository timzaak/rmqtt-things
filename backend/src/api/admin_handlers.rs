use crate::api::ApiState;
use crate::api::admin_models::*;
use crate::api::error::ApiError;
use crate::api::utils::{send_property_command_to_device, validate_identifier};
use crate::api::web_models::{FileUploadRequest, FileUploadResponse};
use crate::cache::SchemaCacheManager;
use crate::db::models::OtaVersion;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::Query;
use std::sync::Arc;
use tracing::{error, info, warn};

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
        let is_directory_allowed = s3_client.config.directories.iter().any(|rule| {
            if rule.ends_with('*') {
                req.directory.starts_with(&rule[..rule.len() - 1])
            } else {
                &req.directory == rule
            }
        });

        if !is_directory_allowed {
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
        if let Some(template) = template_to_update
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
            if e.to_string()
                .contains("Cannot update schema of active template")
            {
                return ApiError::bad_request(e.to_string());
            }
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected > 0 {
        if let Some(template) = template_to_update
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
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::not_found(
            "Template not found or cannot be updated",
        ))
    }
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
        .insert_property_command(&request.product_id, &request.device_id, &request.command)
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
