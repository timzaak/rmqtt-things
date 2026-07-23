//! Factory (production-line) write handlers (support-multiple-device feature,
//! design §4.2.2 A0/A/B + §4.2.3 DTO).
//!
//! These handlers run behind `factory_auth_middleware` (see
//! `factory_middleware.rs`) and share `Arc<ApiState>` with every other route —
//! they access dependencies via `state.app` (the device/webhook side), exactly
//! like the existing webhook/file-upload handlers do, since factory writes are
//! the production-line counterpart to the device-side write path.
//!
//! Three handlers are defined here:
//! - `factory_file_upload_handler` (A0): presigned POST for file attachments,
//!   reusing `S3Client::get_presigned_post` + `is_file_upload_directory_allowed`.
//! - `upsert_component_handler` (A): upsert a component's structured metadata
//!   + file references; the repo layer writes the change log on overwrite (R5).
//! - `replace_associations_handler` (B): full-replace a device's component
//!   associations; does NOT write a change log (R5 scopes the log to component
//!   metadata overwrites only).

use crate::api::ApiState;
use crate::api::admin_models::{FactoryChangeLogQuery, PaginatedResponse, PaginationInfo};
use crate::api::error::ApiError;
use crate::api::handlers::is_file_upload_directory_allowed;
use crate::api::utils::{extract_and_validate_product_id, validate_identifier};
use crate::api::web_models::{
    FileUploadRequest, FileUploadResponse, MqttResponse, RMqttPublishMessage,
};
use crate::db::factory_metadata::{
    ComponentAssociationInput, FactoryDeviceMetadataRow, FactoryDeviceViewRow,
};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue, json};
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::error;
use utoipa::ToSchema;

/// Upper bound for a `fileKey` length. S3 keys allow up to 1024 UTF-8 bytes;
/// this is a deliberately conservative cap to surface bad input early without
/// inventing an arbitrary small ceiling.
const MAX_FILE_KEY_LEN: usize = 1024;

/// Request body for `PUT /api/factory/components/{componentSn}` (design §4.2.2 A).
///
/// All fields are optional: `componentType` defaults to `"camera"` (the DB
/// column default is `"camera"`; the handler substitutes `None` explicitly to
/// keep the behaviour visible at the API surface rather than relying on the
/// caller noticing the DB default). `metadata` and `fileAttachments` default to
/// `{}` and `[]` respectively. A request with every field omitted creates an
/// empty placeholder row (intentional — callers may upsert associations first).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertComponentRequest {
    /// Free-text component type (defaults to `"camera"` when omitted). The
    /// schema deliberately does not enumerate values so radar/sensor/etc. can
    /// be added without a migration (design §1.4 D1).
    #[serde(rename = "componentType", default)]
    pub component_type: Option<String>,
    /// Structured metadata (calibration values, etc.). Defaults to `{}`.
    #[serde(default)]
    pub metadata: Option<Map<String, JsonValue>>,
    /// File-attachment references. Defaults to `[]`. Each `fileKey` must be
    /// obtained from `POST /api/factory/file/upload` first (factory API Key
    /// authentication, NOT the admin/thing upload paths — see design §4.5).
    #[serde(rename = "fileAttachments", default)]
    pub file_attachments: Option<Vec<FileAttachment>>,
}

/// Request body for `PUT /api/factory/devices/{deviceSn}` (design §4.2.2 —
/// device-level write, symmetric to `UpsertComponentRequest` but **without
/// `componentType`**, since devices have no component type).
///
/// `metadata` and `fileAttachments` default to `{}` and `[]` respectively. A
/// request with every field omitted creates an empty placeholder row
/// (intentional — callers may upsert associations first).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertDeviceMetadataRequest {
    /// Structured device-level metadata (serial-number label, QC report
    /// reference, firmware version, etc.). Defaults to `{}`.
    #[serde(default)]
    pub metadata: Option<Map<String, JsonValue>>,
    /// File-attachment references. Defaults to `[]`. Each `fileKey` must be
    /// obtained from `POST /api/factory/file/upload` first (factory API Key
    /// authentication — see design §4.5).
    #[serde(rename = "fileAttachments", default)]
    pub file_attachments: Option<Vec<FileAttachment>>,
}

/// Request body for `PUT /api/factory/devices/{deviceSn}/components`
/// (design §4.2.2 B).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertAssociationsRequest {
    /// Component list — **full replace** semantics. Items not in this list are
    /// removed; identical-content re-submissions are idempotent (design §6.1
    /// `replace_associations_full_replace_is_idempotent`).
    pub components: Vec<ComponentAssociationItem>,
}

/// Single item of `UpsertAssociationsRequest.components` (design §4.2.2 B).
#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentAssociationItem {
    /// Sub-component SN. Same charset as a device SN (`validate_identifier`).
    #[serde(rename = "componentSn")]
    pub component_sn: String,
    /// Optional type hint. The metadata table's value takes precedence in the
    /// merged view (design §4.2.2 C).
    #[serde(rename = "componentType", default)]
    pub component_type: Option<String>,
}

/// Reference to a file attachment uploaded via
/// `POST /api/factory/file/upload` (design §4.2.2 A).
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct FileAttachment {
    /// S3 object key returned by the presigned POST. Non-empty, ≤ 1024 chars.
    #[serde(rename = "fileKey")]
    pub file_key: String,
    /// Original file name (display/download hint).
    #[serde(rename = "fileName")]
    pub file_name: String,
    /// Optional content type hint.
    #[serde(rename = "contentType", default)]
    pub content_type: Option<String>,
    /// Optional size hint in bytes.
    #[serde(rename = "sizeBytes", default)]
    pub size_bytes: Option<i64>,
}

// POST /api/factory/file/upload — presigned POST for file attachments.
//
// Reuses the device-side `S3Client::get_presigned_post` and
// `is_file_upload_directory_allowed` helpers verbatim. Factory uploads have no
// product/device context (sub-component SNs are NOT product/device identifiers),
// so `product_id`/`device_id` are passed as empty strings: this intentionally
// disables the `${productId}` / `${deviceId}` template placeholders in factory
// `[s3] file_upload_directories` rules. Configure factory directory rules with
// literal paths or static prefixes (e.g. `"factory-attachments/*"`); do NOT
// reuse device-scoped rules verbatim — they will not match for factory uploads.
#[utoipa::path(
    post,
    path = "/api/factory/file/upload",
    tag = "factory",
    request_body = FileUploadRequest,
    responses(
        (status = 200, description = "Upload policy", body = FileUploadResponse),
        (status = 400, description = "Directory not allowed or invalid params"),
        (status = 401, description = "Invalid or missing factory API key"),
        (status = 503, description = "S3 client not configured")
    ),
    security(())
)]
pub async fn factory_file_upload_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<FileUploadRequest>,
) -> Result<Json<FileUploadResponse>, ApiError> {
    let state = &state.app;
    let Some(s3_client) = state.s3_client.as_ref() else {
        // Matches the existing `s3_client`-not-configured semantics on the
        // admin/thing file-upload paths (design §4.2.2 A0).
        return Err(ApiError::service_unavailable("S3 client not configured"));
    };

    // product_id / device_id are intentionally empty — see handler doc comment.
    if !is_file_upload_directory_allowed(&s3_client.config.directories, "", "", &req.directory) {
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

    Ok(Json(FileUploadResponse {
        url: presigned_post.url,
        fields: presigned_post.fields,
    }))
}

// PUT /api/factory/components/{componentSn} — upsert component metadata.
//
// Validates the path SN, validates each file attachment's `fileKey`, normalises
// optional fields to their DB defaults (component_type → "camera", metadata →
// `{}`, file_attachments → `[]`), and delegates to
// `FactoryMetadataRepo::upsert_component` (which writes the change_log inside
// the same tx when an overwrite happens — design §5.1, R5). The handler does
// NOT inspect the `UpsertOutcome`: the change-log row is sufficient side-effect
// for R5; no response body is returned (204).
#[utoipa::path(
    put,
    path = "/api/factory/components/{componentSn}",
    tag = "factory",
    params(
        ("componentSn" = String, Path, description = "Sub-component SN (same charset as a device SN)")
    ),
    request_body = UpsertComponentRequest,
    responses(
        (status = 204, description = "Component metadata upserted"),
        (status = 400, description = "Invalid componentSn or fileAttachments"),
        (status = 401, description = "Invalid or missing factory API key"),
        (status = 500, description = "Server error")
    ),
    security(())
)]
pub async fn upsert_component_handler(
    State(state): State<Arc<ApiState>>,
    Path(component_sn): Path<String>,
    Json(req): Json<UpsertComponentRequest>,
) -> Result<StatusCode, ApiError> {
    validate_identifier(&component_sn, "componentSn")?;
    validate_file_attachments(req.file_attachments.as_deref())?;

    // Normalise optionals. component_type default "camera" matches the DB
    // column default — applied here (not via DB) so the contract is visible at
    // the API surface.
    let component_type = req.component_type.unwrap_or_else(|| "camera".to_string());
    let metadata = match req.metadata {
        Some(map) => JsonValue::Object(map),
        None => JsonValue::Object(Map::new()),
    };
    let file_attachments = match req.file_attachments {
        Some(items) => serde_json::to_value(&items).map_err(|e| {
            error!("Failed to serialise file attachments: {}", e);
            ApiError::internal("Failed to serialise file attachments")
        })?,
        None => json!([]),
    };

    let _ = state
        .app
        .db
        .factory_metadata()
        .upsert_component(&component_sn, &component_type, &metadata, &file_attachments)
        .await
        .map_err(|e| {
            error!("Database error on factory component upsert: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

// PUT /api/factory/devices/{deviceSn} — upsert device-level metadata.
//
// Symmetric to `upsert_component_handler` but without componentType: devices
// have no component type (design §1.4). Validates the path SN, validates each
// file attachment's `fileKey`, normalises optionals to their DB defaults
// (metadata → `{}`, file_attachments → `[]`), and delegates to
// `FactoryMetadataRepo::upsert_device_metadata` (which writes the change_log
// inside the same tx when an overwrite happens — design §5.1, R5). The handler
// does NOT inspect the `UpsertOutcome`; the change-log row is sufficient
// side-effect for R5; no response body is returned (204).
#[utoipa::path(
    put,
    path = "/api/factory/devices/{deviceSn}",
    tag = "factory",
    params(
        ("deviceSn" = String, Path, description = "Device SN (same namespace as MQTT client_id)")
    ),
    request_body = UpsertDeviceMetadataRequest,
    responses(
        (status = 204, description = "Device metadata upserted"),
        (status = 400, description = "Invalid deviceSn or fileAttachments"),
        (status = 401, description = "Invalid or missing factory API key"),
        (status = 500, description = "Server error")
    ),
    security(())
)]
pub async fn upsert_device_metadata_handler(
    State(state): State<Arc<ApiState>>,
    Path(device_sn): Path<String>,
    Json(req): Json<UpsertDeviceMetadataRequest>,
) -> Result<StatusCode, ApiError> {
    validate_identifier(&device_sn, "deviceSn")?;
    validate_file_attachments(req.file_attachments.as_deref())?;

    // Normalise optionals. No component_type normalisation — devices have none.
    let metadata = JsonValue::Object(req.metadata.unwrap_or_default());
    let file_attachments = match req.file_attachments {
        Some(items) => serde_json::to_value(&items).map_err(|e| {
            error!("Failed to serialise file attachments: {}", e);
            ApiError::internal("Failed to serialise file attachments")
        })?,
        None => json!([]),
    };

    let _ = state
        .app
        .db
        .factory_metadata()
        .upsert_device_metadata(&device_sn, &metadata, &file_attachments)
        .await
        .map_err(|e| {
            error!("Database error on factory device metadata upsert: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

// PUT /api/factory/devices/{deviceSn}/components — full-replace associations.
//
// Validates the path device SN and each component SN, then delegates to
// `FactoryMetadataRepo::replace_associations`. Does NOT write a change log
// (design §4.2.2 B note; R5 scopes the log to component metadata overwrites).
#[utoipa::path(
    put,
    path = "/api/factory/devices/{deviceSn}/components",
    tag = "factory",
    params(
        ("deviceSn" = String, Path, description = "Device SN (same namespace as MQTT client_id)")
    ),
    request_body = UpsertAssociationsRequest,
    responses(
        (status = 204, description = "Associations replaced"),
        (status = 400, description = "Invalid deviceSn or componentSns"),
        (status = 401, description = "Invalid or missing factory API key"),
        (status = 500, description = "Server error")
    ),
    security(())
)]
pub async fn replace_associations_handler(
    State(state): State<Arc<ApiState>>,
    Path(device_sn): Path<String>,
    Json(req): Json<UpsertAssociationsRequest>,
) -> Result<StatusCode, ApiError> {
    validate_identifier(&device_sn, "deviceSn")?;
    for item in &req.components {
        validate_identifier(&item.component_sn, "componentSn")?;
    }

    let inputs: Vec<ComponentAssociationInput> = req
        .components
        .into_iter()
        .map(|item| ComponentAssociationInput {
            component_sn: item.component_sn,
            component_type: item.component_type,
        })
        .collect();

    state
        .app
        .db
        .factory_metadata()
        .replace_associations(&device_sn, &inputs)
        .await
        .map_err(|e| {
            error!("Database error on factory associations replace: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Validate each file attachment's `fileKey`: non-empty and within the length
/// limit. `fileName` is left unvalidated beyond what `serde` already enforces
/// (must be present); rejecting empty file_names at this layer adds no real
/// protection since the S3 key is the trust anchor.
fn validate_file_attachments(items: Option<&[FileAttachment]>) -> Result<(), ApiError> {
    let Some(items) = items else {
        return Ok(());
    };
    for item in items {
        if item.file_key.trim().is_empty() {
            return Err(ApiError::bad_request("Invalid fileAttachments"));
        }
        if item.file_key.len() > MAX_FILE_KEY_LEN {
            return Err(ApiError::bad_request("Invalid fileAttachments"));
        }
    }
    Ok(())
}

// ============================================================================
// Admin read handlers + DTOs (design §4.2.2 C/D + §4.2.3 DTO).
//
// These handlers run behind the shared `admin_routes` group (Herald
// `device:read` when Herald is configured; single-tenant passthrough
// otherwise). They read via `state.admin.db.factory_metadata()`.
// ============================================================================

/// Admin merged-view response for a device (design §4.2.2 C).
///
/// `device_metadata` carries the device-level factory metadata row when present
/// (written via `PUT /api/factory/devices/{deviceSn}`), or `null` when no
/// device-level metadata has been reported yet. `components` is the left-join of
/// associations with component metadata; components whose metadata has not
/// arrived yet surface with `metadata: null` and `file_attachments: []` (R3).
#[derive(Debug, Serialize, ToSchema)]
pub struct FactoryDeviceView {
    #[serde(rename = "deviceSn")]
    pub device_sn: String,
    /// Device-level factory metadata (symmetric to `FactoryComponentView` minus
    /// componentType/componentSn). `null` when no device-level metadata has been
    /// reported yet.
    #[serde(rename = "deviceMetadata")]
    pub device_metadata: Option<FactoryDeviceMetadataView>,
    pub components: Vec<FactoryComponentView>,
}

/// Device-level factory metadata view (design §4.2.2, §5.1). Symmetric to
/// `FactoryComponentView` minus `componentType`/`componentSn` — devices have no
/// component type or sub-component SN at this level. `file_attachments` is
/// normalised from the raw JSONB to an array (Array passthrough, Null → empty,
/// scalar → wrapped), matching `map_row_to_component_view`.
#[derive(Debug, Serialize, ToSchema)]
pub struct FactoryDeviceMetadataView {
    pub metadata: Option<JsonValue>,
    #[serde(rename = "fileAttachments")]
    pub file_attachments: Vec<JsonValue>,
    #[serde(rename = "updatedAt", with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Single component in a `FactoryDeviceView` (design §4.2.2 C).
///
/// `component_type` prefers the metadata table's value and falls back to the
/// association table's hint; both absent → `None`. `file_attachments` is an
/// array (defaults to `[]` when metadata has not arrived).
#[derive(Debug, Serialize, ToSchema)]
pub struct FactoryComponentView {
    #[serde(rename = "componentSn")]
    pub component_sn: String,
    #[serde(rename = "componentType")]
    pub component_type: Option<String>,
    pub metadata: Option<JsonValue>,
    #[serde(rename = "fileAttachments")]
    pub file_attachments: Vec<JsonValue>,
    #[serde(rename = "updatedAt", with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

// GET /api/admin/factory/devices/{deviceSn} — admin merged view (design §4.2.2 C).
//
// Returns 404 when the device has neither associations nor device-level
// metadata (strict 404 vs empty-200, so the frontend can distinguish "not
// reported" from "device does not exist"). Partial data still returns 200 with
// null fields per component.
#[utoipa::path(
    get,
    path = "/api/admin/factory/devices/{deviceSn}",
    tag = "admin",
    params(
        ("deviceSn" = String, Path, description = "Device SN (same namespace as MQTT client_id)")
    ),
    responses(
        (status = 200, description = "Merged factory view", body = FactoryDeviceView),
        (status = 400, description = "Invalid deviceSn"),
        (status = 404, description = "Device has no associations and no device-level metadata"),
        (status = 500, description = "Server error")
    )
)]
pub async fn get_factory_device_view_handler(
    State(state): State<Arc<ApiState>>,
    Path(device_sn): Path<String>,
) -> Result<Json<FactoryDeviceView>, ApiError> {
    validate_identifier(&device_sn, "deviceSn")?;

    let rows = state
        .admin
        .db
        .factory_metadata()
        .get_device_view(&device_sn)
        .await
        .map_err(|e| {
            error!("Database error on factory device view: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    let Some(rows) = rows else {
        // Strict 404 — no associations AND no device-level metadata (design §4.2.2 C).
        return Err(ApiError::not_found("Device has no factory metadata"));
    };

    let components: Vec<FactoryComponentView> =
        rows.into_iter().map(map_row_to_component_view).collect();

    // Device-level metadata is read in a second repo call (design §5.1: keep
    // `get_device_view` single-responsibility). The 404 decision above is driven
    // solely by `get_device_view`; this call only reads the content — None here
    // does NOT change the 404 outcome (design §6.3).
    let device_metadata = state
        .admin
        .db
        .factory_metadata()
        .get_device_metadata(&device_sn)
        .await
        .map_err(|e| {
            error!("Database error on factory device metadata read: {}", e);
            ApiError::internal("Database operation failed")
        })?
        .map(map_row_to_device_metadata_view);

    Ok(Json(FactoryDeviceView {
        device_sn,
        device_metadata,
        components,
    }))
}

// GET /api/admin/factory/sn/{sn}/changes — change log
// (design §4.2.2 D, time-descending pagination).
#[utoipa::path(
    get,
    path = "/api/admin/factory/sn/{sn}/changes",
    tag = "admin",
    params(
        ("sn" = String, Path, description = "SN (device SN or sub-component SN)"),
        FactoryChangeLogQuery
    ),
    responses(
        (status = 200, description = "Paginated change log", body = PaginatedResponse<crate::db::models::FactoryMetadataChangeLog>),
        (status = 400, description = "Invalid sn"),
        (status = 500, description = "Server error")
    )
)]
pub async fn query_component_changes_handler(
    State(state): State<Arc<ApiState>>,
    Path(sn): Path<String>,
    Query(query): Query<FactoryChangeLogQuery>,
) -> Result<Json<PaginatedResponse<crate::db::models::FactoryMetadataChangeLog>>, ApiError> {
    validate_identifier(&sn, "sn")?;

    // Clamp page/page_size and reflect the clamped inputs in the pagination
    // echo. i64 -> u32 is safe after clamp (negative/zero become 1).
    let page = query.page.max(1) as u32;
    let page_size = query.page_size.clamp(1, 1000) as u32;

    let (rows, total) = state
        .admin
        .db
        .factory_metadata()
        .query_change_log(&sn, page, page_size)
        .await
        .map_err(|e| {
            error!("Database error on factory change log query: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    Ok(Json(PaginatedResponse {
        data: rows,
        pagination: PaginationInfo {
            page: page as i64,
            page_size: page_size as i64,
            total: total as i64,
        },
    }))
}

/// Coerce a raw JSONB `file_attachments` value into the array the API view
/// exposes: Array passes through, Null → empty, scalar → wrapped in a
/// single-element array (matches the "metadata not arrived" / passthrough
/// surface contract, design §4.2.2 C). Shared by both row→view mappers.
fn normalise_file_attachments(v: Option<JsonValue>) -> Vec<JsonValue> {
    v.and_then(|v| match v {
        JsonValue::Array(arr) => Some(arr),
        JsonValue::Null => None,
        _ => Some(vec![v]),
    })
    .unwrap_or_default()
}

/// Map a left-join row to its API view. `meta_type` (metadata table) wins over
/// `assoc_type` (association table); both absent → `None`. `file_attachments`
/// falls back to `[]` when the JSON value is null or not an array (matches the
/// "metadata not arrived" surface contract in design §4.2.2 C).
fn map_row_to_component_view(row: FactoryDeviceViewRow) -> FactoryComponentView {
    let component_type = row.meta_type.or(row.assoc_type);
    let file_attachments = normalise_file_attachments(row.file_attachments);

    FactoryComponentView {
        component_sn: row.component_sn,
        component_type,
        metadata: row.metadata,
        file_attachments,
        updated_at: row.updated_at,
    }
}

/// Map a device-level metadata row to its API view. Same `file_attachments`
/// normalisation as `map_row_to_component_view` (Array passthrough, Null →
/// empty, scalar → wrapped); device-level has no componentType/componentSn.
fn map_row_to_device_metadata_view(row: FactoryDeviceMetadataRow) -> FactoryDeviceMetadataView {
    let file_attachments = normalise_file_attachments(row.file_attachments);

    FactoryDeviceMetadataView {
        metadata: row.metadata,
        file_attachments,
        updated_at: row.updated_at,
    }
}

// ============================================================================
// Device pull webhook handler (design §5.3).
//
// RMQTT forwards a device's publish on `/{product}/{device}/thing/factory-metadata/get`
// to this handler. The platform assembles the merged view and publishes it back
// to `{topic}_reply` via `publish_response`. Runs behind `internal_ip_middleware`
// (shared with the other webhook routes); HMAC auth is done by the broker.
// ============================================================================

// POST /api/thing/factory-metadata/get — device pull webhook (design §5.3).
#[utoipa::path(
    post,
    path = "/api/thing/factory-metadata/get",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "Reply published to {topic}_reply"),
        (status = 400, description = "Invalid payload or topic"),
        (status = 500, description = "Server error")
    ),
    security(())
)]
pub async fn factory_metadata_get_handler(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let state = &state.app;

    let payload = mqtt_msg.decode_payload_as_json().map_err(|e| {
        error!("Failed to decode factory-metadata payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;

    // `extract_and_validate_product_id` returns Result (utils.rs:168-173); do NOT
    // use `extract_product_id_from_topic(...)?` (returns Option — won't compile
    // in a Result-returning fn).
    let _product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
    let device_id = mqtt_msg.client_id.clone();
    validate_identifier(&device_id, "device_id")?;

    // device_sn == client_id this round (design §5.3 note).
    let view = state
        .db
        .factory_metadata()
        .get_device_view(&device_id)
        .await
        .map_err(|e| {
            error!("Database error on factory-metadata device view: {}", e);
            ApiError::internal("Database operation failed")
        })?;

    // Second repo call for the device-level content (design §5.1: keep
    // `get_device_view` single-responsibility). Fetched outside the
    // `Option::map` closure below because it is async; the closure only assembles.
    let device_metadata_row = state
        .db
        .factory_metadata()
        .get_device_metadata(&device_id)
        .await
        .map_err(|e| {
            error!(
                "Database error on factory-metadata device metadata read: {}",
                e
            );
            ApiError::internal("Database operation failed")
        })?;
    let device_metadata = device_metadata_row.map(map_row_to_device_metadata_view);

    // None → data: null (device treats this as "no factory metadata yet").
    let data: Option<JsonValue> = view.map(|rows| {
        let components: Vec<FactoryComponentView> =
            rows.into_iter().map(map_row_to_component_view).collect();
        let view = FactoryDeviceView {
            device_sn: device_id.clone(),
            device_metadata,
            components,
        };
        // `FactoryDeviceView` is Serialize; convert to a raw JSON value for the
        // generic `MqttResponse.data` field.
        serde_json::to_value(&view).unwrap_or(JsonValue::Null)
    });

    let response = MqttResponse {
        id: payload.id,
        code: 200,
        data,
    };

    // publish_response takes (&str, &str) — serialise first (handlers.rs:391-399).
    let response_payload = serde_json::to_string(&response).map_err(|e| {
        error!("Failed to serialise factory-metadata response: {}", e);
        ApiError::internal("Failed to serialise response")
    })?;

    if let Err(e) = state
        .rmqtt_client
        .publish_response(&mqtt_msg.topic, &response_payload)
        .await
    {
        error!("Failed to publish factory-metadata response: {}", e);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_file_attachments_none_is_ok() {
        assert!(validate_file_attachments(None).is_ok());
    }

    #[test]
    fn validate_file_attachments_empty_file_key_rejected() {
        let items = vec![FileAttachment {
            file_key: "   ".to_string(),
            file_name: "x.bin".to_string(),
            content_type: None,
            size_bytes: None,
        }];
        assert!(validate_file_attachments(Some(&items)).is_err());
    }

    #[test]
    fn validate_file_attachments_overlong_file_key_rejected() {
        let items = vec![FileAttachment {
            file_key: "a".repeat(MAX_FILE_KEY_LEN + 1),
            file_name: "x.bin".to_string(),
            content_type: None,
            size_bytes: None,
        }];
        assert!(validate_file_attachments(Some(&items)).is_err());
    }

    #[test]
    fn validate_file_attachments_accepts_well_formed() {
        let items = vec![FileAttachment {
            file_key: "factory-attachments/abc/calib.bin".to_string(),
            file_name: "calib.bin".to_string(),
            content_type: Some("application/octet-stream".to_string()),
            size_bytes: Some(2048),
        }];
        assert!(validate_file_attachments(Some(&items)).is_ok());
    }
}
