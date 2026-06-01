use crate::api::ApiState;
use crate::api::admin_models::{CertificatesListResponse, CommonQuery2, SimplePaginationInfo};
use crate::api::error::{ApiError, map_db_err};
use crate::api::utils::validate_identifier;
use crate::ca;
use crate::db::models::{CertIssue, CertStatus, RegistrationSource};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::Path as StdPath;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::fs;
use tracing::error;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IssueCertRequest {
    pub product_id: String,
    pub device_id: String,
    pub force: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub start_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub end_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IssueCertResponse {
    pub cert_pem: String,
    pub key_pem: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateCertStatusRequest {
    pub id: Option<i64>,
    pub product_id: String,
    pub device_id: String,
    pub status: i16,
}

#[utoipa::path(
    post,
    path = "/api/admin/ca/cert",
    tag = "admin",
    request_body = IssueCertRequest,
    responses(
        (status = 200, description = "Certificate issued", body = IssueCertResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn issue_cert_handler(
    State(state): State<Arc<ApiState>>,
    Json(issue_req): Json<IssueCertRequest>,
) -> Result<Json<IssueCertResponse>, ApiError> {
    let app_state = &state.admin;
    validate_identifier(&issue_req.product_id, "product_id")?;
    validate_identifier(&issue_req.device_id, "device_id")?;
    if !issue_req.force
        && let Some(cert) = app_state
            .db
            .cert_issue()
            .find_by_device_id(&issue_req.product_id, &issue_req.device_id)
            .await
            .map_err(map_db_err)?
        && cert.status == CertStatus::Normal
        && cert.end_at > OffsetDateTime::now_utc()
    {
        return Err(ApiError::bad_request(
            "Certificate already exists and is valid",
        ));
    }

    let ca_dir = StdPath::new(&app_state.config.ca.ca_dir);
    let ca_cert_path = ca_dir.join("ca.pem");
    let ca_key_path = ca_dir.join("ca.key");

    let ca_cert_pem = fs::read_to_string(ca_cert_path)
        .await
        .map_err(|_e| ApiError::internal("could not get CA file"))?;
    let ca_key_pem = fs::read_to_string(ca_key_path)
        .await
        .map_err(|_e| ApiError::internal("could not get CA file"))?;

    let (cert_pem, key_pem) = ca::generator::issue_cert(
        &ca_cert_pem,
        &ca_key_pem,
        &issue_req.product_id,
        &issue_req.device_id,
        issue_req.start_at,
        issue_req.end_at,
    )
    .map_err(|e| {
        error!("Database error: {e}");
        ApiError::internal("Certificate generation failed")
    })?;

    let new_cert = CertIssue {
        id: 0,
        product_id: issue_req.product_id.clone(),
        device_id: issue_req.device_id.clone(),
        pub_cert: cert_pem.clone(),
        start_at: issue_req.start_at,
        end_at: issue_req.end_at,
        status: CertStatus::Normal,
        created_at: OffsetDateTime::now_utc(),
    };

    app_state
        .db
        .cert_issue()
        .create(&new_cert)
        .await
        .map_err(map_db_err)?;

    if let Err(e) = app_state
        .db
        .device()
        .upsert(
            &issue_req.product_id,
            &issue_req.device_id,
            RegistrationSource::Manual,
        )
        .await
    {
        tracing::warn!(
            product_id = %issue_req.product_id,
            device_id = %issue_req.device_id,
            error = %e,
            "Failed to create device record during cert issuance"
        );
    }

    Ok(Json(IssueCertResponse { cert_pem, key_pem }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/ca/cert/status",
    tag = "admin",
    request_body = UpdateCertStatusRequest,
    responses(
        (status = 204, description = "Certificate status updated"),
        (status = 500, description = "Server error")
    )
)]
pub async fn update_cert_status_handler(
    State(state): State<Arc<ApiState>>,
    Json(update_req): Json<UpdateCertStatusRequest>,
) -> Result<StatusCode, ApiError> {
    let app_state = &state.admin;
    let cert_repo = app_state.db.cert_issue();

    // Validate the target status is a valid transition target
    let target_status = CertStatus::try_from(update_req.status)
        .map_err(|_| ApiError::bad_request("无效的证书状态"))?;
    if target_status == CertStatus::Normal {
        return Err(ApiError::bad_request("不允许将证书状态设置为 Normal"));
    }

    if let Some(cert_id) = update_req.id {
        // ID-based: fetch current cert, enforce state machine, update by ID
        let cert = cert_repo
            .find_by_id(cert_id)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| ApiError::not_found("证书不存在"))?;

        if cert.status != CertStatus::Normal {
            return Err(ApiError::bad_request("只能操作 Normal 状态的证书"));
        }

        cert_repo
            .update_status_by_id(cert_id, update_req.status)
            .await
            .map_err(map_db_err)?;
    } else {
        // Legacy batch mode: validate identifiers, enforce state machine on the found cert
        validate_identifier(&update_req.product_id, "product_id")?;
        validate_identifier(&update_req.device_id, "device_id")?;

        let cert = cert_repo
            .find_by_device_id(&update_req.product_id, &update_req.device_id)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| ApiError::not_found("证书不存在"))?;

        if cert.status != CertStatus::Normal {
            return Err(ApiError::bad_request("只能操作 Normal 状态的证书"));
        }

        cert_repo
            .update_status(
                &update_req.product_id,
                &update_req.device_id,
                update_req.status,
            )
            .await
            .map_err(map_db_err)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/admin/ca/cert",
    tag = "admin",
    params(CommonQuery2),
    responses(
        (status = 200, description = "Certificate list", body = CertificatesListResponse),
        (status = 500, description = "Server error")
    )
)]
pub async fn list_certs_handler(
    State(state): State<Arc<ApiState>>,
    Query(req): Query<CommonQuery2>,
) -> Result<Json<CertificatesListResponse>, ApiError> {
    let app_state = &state.admin;
    if let Some(ref pid) = req.product_id {
        validate_identifier(pid, "product_id")?;
    }
    if let Some(ref did) = req.device_id {
        validate_identifier(did, "device_id")?;
    }
    let certs = app_state
        .db
        .cert_issue()
        .list(
            req.product_id.clone(),
            req.device_id.clone(),
            req.page,
            req.page_size,
        )
        .await
        .map_err(map_db_err)?;
    Ok(Json(CertificatesListResponse {
        data: certs,
        pagination: SimplePaginationInfo {
            page: req.page,
            page_size: req.page_size,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/ca/cert/{id}",
    tag = "admin",
    responses(
        (status = 200, description = "Certificate detail", body = CertIssue),
        (status = 404, description = "Certificate not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn get_cert_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<CertIssue>, ApiError> {
    let app_state = &state.admin;
    let cert = app_state
        .db
        .cert_issue()
        .find_by_id(id)
        .await
        .map_err(map_db_err)?;
    match cert {
        Some(c) => Ok(Json(c)),
        None => Err(ApiError::not_found("Certificate not found")),
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CaCertResponse {
    pub ca_pem: String,
}

#[utoipa::path(
    get,
    path = "/api/admin/ca/pem",
    tag = "admin",
    responses(
        (status = 200, description = "CA certificate PEM", body = CaCertResponse),
        (status = 500, description = "Server error")
    )
)]
pub async fn get_ca_cert_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CaCertResponse>, ApiError> {
    let app_state = &state.admin;
    let ca_dir = StdPath::new(&app_state.config.ca.ca_dir);
    let ca_cert_path = ca_dir.join("ca.pem");
    let ca_cert_pem = fs::read_to_string(ca_cert_path)
        .await
        .map_err(|_e| ApiError::internal("could not get CA file"))?;
    Ok(Json(CaCertResponse {
        ca_pem: ca_cert_pem,
    }))
}
