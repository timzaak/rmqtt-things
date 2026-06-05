use crate::api::ApiState;
use crate::api::admin_models::{PaginatedResponse, PaginationInfo};
use crate::api::alarm_models::{
    AlarmQuery, AlarmRecordListResponse, AlarmRecordResponse, AlarmRuleListResponse,
    AlarmRuleQuery, AlarmRuleResponse, ApiAlarmRecord, CreateAlarmRuleRequest,
    UpdateAlarmRuleRequest, UpdateAlarmRuleStatusRequest,
};
use crate::api::error::ApiError;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use std::sync::Arc;
use tracing::error;

#[utoipa::path(
    get,
    path = "/api/admin/alarm-rule",
    tag = "admin",
    params(crate::api::alarm_models::AlarmRuleQuery),
    responses(
        (status = 200, description = "Alarm rule list", body = AlarmRuleListResponse),
        (status = 500, description = "Server error")
    )
)]
pub async fn list_alarm_rules(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AlarmRuleQuery>,
) -> Result<Json<AlarmRuleListResponse>, ApiError> {
    let state = &state.admin;
    let (rules, total) = state
        .db
        .alarm()
        .query_rules(
            query.product_id.as_deref(),
            query.enabled,
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let response = PaginatedResponse {
        data: rules,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/admin/alarm-rule",
    tag = "admin",
    request_body = CreateAlarmRuleRequest,
    responses(
        (status = 201, description = "Alarm rule created", body = AlarmRuleResponse),
        (status = 400, description = "Invalid product_id or request body"),
        (status = 500, description = "Server error")
    )
)]
pub async fn create_alarm_rule(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateAlarmRuleRequest>,
) -> Result<(StatusCode, Json<AlarmRuleResponse>), ApiError> {
    let state = &state.admin;

    // Validate trigger_type is a known value
    if crate::rule_engine::TriggerType::from_str(&req.trigger_type).is_none() {
        return Err(ApiError::bad_request(format!(
            "Invalid trigger_type '{}'. Must be one of: property, event, device_online, device_offline",
            req.trigger_type
        )));
    }

    // Validate product_id exists by checking product.model_no
    let product = state
        .db
        .product()
        .get_product_by_model_no(&req.product_id)
        .await
        .map_err(|e| {
            error!("Database error checking product: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if product.is_none() {
        return Err(ApiError::bad_request(format!(
            "Product '{}' not found",
            req.product_id
        )));
    }

    // Validate duration_minutes >= 0
    if req.duration_minutes < 0 {
        return Err(ApiError::bad_request("duration_minutes must be >= 0"));
    }

    // Only property trigger type supports duration_minutes > 0 and clear_condition
    if req.trigger_type != "property" && (req.duration_minutes > 0 || req.clear_condition.is_some())
    {
        return Err(ApiError::bad_request(
            "Duration and clear conditions are only supported for property trigger type",
        ));
    }

    let rule_id = state.db.alarm().create_rule(&req).await.map_err(|e| {
        error!("Database error creating rule: {}", e);
        ApiError::internal("Database operation failed")
    })?;

    // Fetch the created rule to return full data
    let rule = state
        .db
        .alarm()
        .get_rule_by_id(rule_id)
        .await
        .map_err(|e| {
            error!("Database error fetching created rule: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::internal("Failed to fetch created rule"))?;

    // Invalidate cache for this product
    state.rule_cache.invalidate_product(&req.product_id);

    Ok((StatusCode::CREATED, Json(AlarmRuleResponse { data: rule })))
}

#[utoipa::path(
    get,
    path = "/api/admin/alarm-rule/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm rule id")),
    responses(
        (status = 200, description = "Alarm rule details", body = AlarmRuleResponse),
        (status = 404, description = "Alarm rule not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn get_alarm_rule(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<AlarmRuleResponse>, ApiError> {
    let state = &state.admin;
    let rule = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm rule not found"))?;
    Ok(Json(AlarmRuleResponse { data: rule }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/alarm-rule/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm rule id")),
    request_body = UpdateAlarmRuleRequest,
    responses(
        (status = 200, description = "Alarm rule updated", body = AlarmRuleResponse),
        (status = 404, description = "Alarm rule not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn update_alarm_rule(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateAlarmRuleRequest>,
) -> Result<Json<AlarmRuleResponse>, ApiError> {
    let state = &state.admin;

    // Validate duration_minutes >= 0 if provided
    if let Some(v) = req.duration_minutes
        && v < 0
    {
        return Err(ApiError::bad_request("duration_minutes must be >= 0"));
    }

    // Fetch existing rule to get product_id for cache invalidation
    let existing = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm rule not found"))?;

    // Only property trigger type supports duration_minutes > 0 and clear_condition
    if existing.trigger_type != "property"
        && (req.duration_minutes.is_some_and(|v| v > 0)
            || req
                .clear_condition
                .as_ref()
                .is_some_and(|opt| opt.is_some()))
    {
        return Err(ApiError::bad_request(
            "Duration and clear conditions are only supported for property trigger type",
        ));
    }

    let rows_affected = state
        .db
        .alarm()
        .update_rule(
            id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.trigger_config.as_ref(),
            req.condition.as_ref(),
            req.actions.as_ref(),
            req.throttle_minutes,
            req.duration_minutes,
            req.clear_condition.as_ref().map(|opt| opt.as_ref()),
        )
        .await
        .map_err(|e| {
            error!("Database error updating rule: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected == 0 {
        return Err(ApiError::not_found("Alarm rule not found"));
    }

    // Fetch updated rule
    let updated = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error fetching updated rule: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::internal("Failed to fetch updated rule"))?;

    // Invalidate cache for this product
    state.rule_cache.invalidate_product(&existing.product_id);

    Ok(Json(AlarmRuleResponse { data: updated }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/alarm-rule/{id}/status",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm rule id")),
    request_body = UpdateAlarmRuleStatusRequest,
    responses(
        (status = 200, description = "Alarm rule status updated", body = AlarmRuleResponse),
        (status = 404, description = "Alarm rule not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn update_alarm_rule_status(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateAlarmRuleStatusRequest>,
) -> Result<Json<AlarmRuleResponse>, ApiError> {
    let state = &state.admin;

    // Fetch existing rule to get product_id for cache invalidation
    let existing = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm rule not found"))?;

    let rows_affected = state
        .db
        .alarm()
        .update_rule_status(id, req.enabled)
        .await
        .map_err(|e| {
            error!("Database error updating rule status: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    if rows_affected == 0 {
        return Err(ApiError::not_found("Alarm rule not found"));
    }

    // Fetch updated rule
    let updated = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error fetching updated rule: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::internal("Failed to fetch updated rule"))?;

    // Invalidate cache for this product
    state.rule_cache.invalidate_product(&existing.product_id);

    Ok(Json(AlarmRuleResponse { data: updated }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/alarm-rule/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm rule id")),
    responses(
        (status = 204, description = "Alarm rule deleted"),
        (status = 404, description = "Alarm rule not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn delete_alarm_rule(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let state = &state.admin;

    // Fetch existing rule to get product_id for cache invalidation
    let existing = state
        .db
        .alarm()
        .get_rule_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm rule not found"))?;

    let rows_affected = state.db.alarm().delete_rule(id).await.map_err(|e| {
        error!("Database error deleting rule: {}", e);
        ApiError::internal("Database operation failed")
    })?;

    if rows_affected == 0 {
        return Err(ApiError::not_found("Alarm rule not found"));
    }

    // Invalidate cache for this product
    state.rule_cache.invalidate_product(&existing.product_id);

    Ok(StatusCode::NO_CONTENT)
}

// --- Alarm Record handlers ---

fn parse_level(level_str: &str) -> Option<i16> {
    match level_str {
        "info" => Some(0),
        "warning" => Some(1),
        "critical" => Some(2),
        _ => None,
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/alarm",
    tag = "admin",
    params(crate::api::alarm_models::AlarmQuery),
    responses(
        (status = 200, description = "Alarm record list", body = AlarmRecordListResponse),
        (status = 500, description = "Server error")
    )
)]
pub async fn list_alarms(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AlarmQuery>,
) -> Result<Json<AlarmRecordListResponse>, ApiError> {
    let state = &state.admin;

    // status and acknowledged are mutually exclusive
    if query.status.is_some() && query.acknowledged.is_some() {
        return Err(ApiError::bad_request(
            "Parameters 'status' and 'acknowledged' are mutually exclusive",
        ));
    }

    let level = query.level.as_deref().and_then(parse_level);

    let (alarms, total) = state
        .db
        .alarm()
        .query_alarms(
            query.product_id.as_deref(),
            query.device_id.as_deref(),
            level,
            query.acknowledged,
            query.status.as_deref(),
            query.page,
            query.page_size,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let data: Vec<ApiAlarmRecord> = alarms.into_iter().map(ApiAlarmRecord::from).collect();

    let response = PaginatedResponse {
        data,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/api/admin/alarm/{id}/ack",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm record id")),
    responses(
        (status = 200, description = "Alarm acknowledged", body = AlarmRecordResponse),
        (status = 404, description = "Alarm not found"),
        (status = 409, description = "Alarm already acknowledged"),
        (status = 500, description = "Server error")
    )
)]
pub async fn ack_alarm(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<AlarmRecordResponse>, ApiError> {
    let state = &state.admin;

    // Check existence first
    let existing = state
        .db
        .alarm()
        .get_alarm_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm not found"))?;

    if existing.status != "active" {
        return Err(ApiError::conflict(
            "Only alarms in active status can be acknowledged",
        ));
    }

    let rows_affected = state.db.alarm().ack_alarm(id).await.map_err(|e| {
        error!("Database error acknowledging alarm: {}", e);
        ApiError::internal("Database operation failed")
    })?;

    if rows_affected == 0 {
        return Err(ApiError::conflict(
            "Only alarms in active status can be acknowledged",
        ));
    }

    // Fetch updated record
    let updated = state
        .db
        .alarm()
        .get_alarm_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error fetching updated alarm: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::internal("Failed to fetch updated alarm"))?;

    Ok(Json(AlarmRecordResponse {
        data: ApiAlarmRecord::from(updated),
    }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/alarm/{id}/clear",
    tag = "admin",
    params(("id" = i64, Path, description = "Alarm record id")),
    responses(
        (status = 200, description = "Alarm cleared", body = AlarmRecordResponse),
        (status = 404, description = "Alarm not found"),
        (status = 409, description = "Alarm already cleared"),
        (status = 500, description = "Server error")
    )
)]
pub async fn clear_alarm(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<AlarmRecordResponse>, ApiError> {
    let state = &state.admin;

    // Check existence first
    let existing = state
        .db
        .alarm()
        .get_alarm_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::not_found("Alarm not found"))?;

    if existing.status == "cleared" {
        return Err(ApiError::conflict("Alarm already cleared"));
    }

    let rows_affected = state.db.alarm().clear_alarm(id).await.map_err(|e| {
        error!("Database error clearing alarm: {}", e);
        ApiError::internal("Database operation failed")
    })?;

    if rows_affected == 0 {
        return Err(ApiError::conflict("Alarm already cleared"));
    }

    // Fetch updated record
    let updated = state
        .db
        .alarm()
        .get_alarm_by_id(id)
        .await
        .map_err(|e| {
            error!("Database error fetching cleared alarm: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .ok_or_else(|| ApiError::internal("Failed to fetch cleared alarm"))?;

    Ok(Json(AlarmRecordResponse {
        data: ApiAlarmRecord::from(updated),
    }))
}
