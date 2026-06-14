use crate::api::ApiState;
use crate::api::admin_models::{PaginatedResponse, PaginationInfo, ProductQuery};
use crate::api::error::ApiError;
use crate::api::utils::validate_identifier;
use crate::db::models::{CreateProductRequest, Product, UpdateProductRequest};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use std::sync::Arc;
use tracing::error;

/// PostgreSQL error code for unique constraint violation.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Max length enforced for the human-readable product `name` field.
/// `model_no` is bounded by `validate_identifier` (128 chars).
const MAX_PRODUCT_NAME_LENGTH: usize = 128;

/// Validate a CreateProductRequest before touching the database.
///
/// `model_no` is the product identifier used across the system (it is the
/// `product_id` in alarm rules, validation templates, MQTT topics, etc.), so
/// it must satisfy `validate_identifier` (non-empty, <=128 chars,
/// `[A-Za-z0-9_-]`). `name` is a free-form display label: non-empty, <=128
/// chars. See PRD docs/prd/core/product-device-management.md §5.1 (model_no
/// globally unique) and P1-10 audit fix.
pub fn validate_create_product_request(req: &CreateProductRequest) -> Result<(), ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("name must not be empty"));
    }
    if name.len() > MAX_PRODUCT_NAME_LENGTH {
        return Err(ApiError::bad_request(format!(
            "name must not exceed {MAX_PRODUCT_NAME_LENGTH} characters"
        )));
    }
    validate_identifier(&req.model_no, "model_no")?;
    Ok(())
}

/// Validate an UpdateProductRequest before touching the database.
///
/// Mirrors `validate_create_product_request` for the updatable `name` field
/// (non-empty, <=128 chars). `model_no` is immutable on update; `description`
/// is free-form and not length-bounded (consistent with create). See P2-17.
pub fn validate_update_product_request(req: &UpdateProductRequest) -> Result<(), ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("name must not be empty"));
    }
    if name.len() > MAX_PRODUCT_NAME_LENGTH {
        return Err(ApiError::bad_request(format!(
            "name must not exceed {MAX_PRODUCT_NAME_LENGTH} characters"
        )));
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/admin/product",
    tag = "admin",
    request_body = CreateProductRequest,
    responses(
        (status = 201, description = "Product created", body = Product),
        (status = 500, description = "Server error")
    )
)]
pub async fn create_product(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateProductRequest>,
) -> Result<(StatusCode, Json<Product>), ApiError> {
    let state = &state.admin;
    validate_create_product_request(&req)?;
    match state.db.product().create_product(&req).await {
        Ok(product) => Ok((StatusCode::CREATED, Json(product))),
        Err(e) => {
            if let Some(db_err) = e.downcast_ref::<sqlx::Error>()
                && let sqlx::Error::Database(db_err_inner) = db_err
                && db_err_inner.code().as_deref() == Some(PG_UNIQUE_VIOLATION)
            {
                return Err(ApiError::conflict("产品型号编号已存在"));
            }
            error!("Database error: {}", e);
            Err(ApiError::internal("Database operation failed"))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/admin/product/{id}",
    tag = "admin",
    params(("id" = i32, Path, description = "Product id")),
    request_body = UpdateProductRequest,
    responses(
        (status = 200, description = "Product updated", body = Product),
        (status = 404, description = "Product not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn update_product(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateProductRequest>,
) -> Result<Json<Product>, ApiError> {
    let state = &state.admin;
    validate_update_product_request(&req)?;
    match state.db.product().update_product(id, &req).await {
        Ok(Some(product)) => Ok(Json(product)),
        Ok(None) => Err(ApiError::not_found("Product not found")),
        Err(e) => {
            error!("Database error: {}", e);
            Err(ApiError::internal("Database operation failed"))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/product/{id}",
    tag = "admin",
    params(("id" = i64, Path, description = "Product id")),
    responses(
        (status = 200, description = "Product details", body = Product),
        (status = 404, description = "Product not found"),
        (status = 500, description = "Server error")
    )
)]
pub async fn get_product(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> Result<Json<Product>, ApiError> {
    let state = &state.admin;
    match state.db.product().get_product(id).await {
        Ok(Some(product)) => Ok(Json(product)),
        Ok(None) => Err(ApiError::not_found("Product not found")),
        Err(e) => {
            error!("Database error: {}", e);
            Err(ApiError::internal("Database operation failed"))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/product",
    tag = "admin",
    params(ProductQuery),
    responses(
        (status = 200, description = "Product list", body = PaginatedResponse<Product>),
        (status = 500, description = "Server error")
    )
)]
pub async fn list_products(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ProductQuery>,
) -> Result<Json<PaginatedResponse<Product>>, ApiError> {
    let state = &state.admin;
    let (products, total) = state
        .db
        .product()
        .list_products(query.search.as_deref(), query.page, query.page_size)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            ApiError::internal("Database operation failed")
        })?;
    let response = PaginatedResponse {
        data: products,
        pagination: PaginationInfo {
            page: query.page,
            page_size: query.page_size,
            total,
        },
    };
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateProductRequest;

    fn req(name: &str, model_no: &str) -> CreateProductRequest {
        CreateProductRequest {
            name: name.into(),
            model_no: model_no.into(),
            description: None,
        }
    }

    fn err_status(result: Result<(), ApiError>) -> Option<u16> {
        use axum::response::IntoResponse;
        result.err().map(|e| e.into_response().status().as_u16())
    }

    #[test]
    fn valid_request_passes() {
        assert!(validate_create_product_request(&req("Sensor", "sensor-v1")).is_ok());
    }

    #[test]
    fn empty_model_no_rejected() {
        assert_eq!(
            err_status(validate_create_product_request(&req("Sensor", ""))),
            Some(400)
        );
    }

    #[test]
    fn empty_name_rejected() {
        // Whitespace-only name trims to empty -> rejected.
        assert_eq!(
            err_status(validate_create_product_request(&req("   ", "sensor-v1"))),
            Some(400)
        );
    }

    #[test]
    fn overlong_name_rejected() {
        let long = "a".repeat(129);
        assert_eq!(
            err_status(validate_create_product_request(&req(&long, "sensor-v1"))),
            Some(400)
        );
    }

    #[test]
    fn overlong_model_no_rejected() {
        let long = "a".repeat(129);
        assert_eq!(
            err_status(validate_create_product_request(&req("Sensor", &long))),
            Some(400)
        );
    }

    #[test]
    fn invalid_chars_in_model_no_rejected() {
        // Spaces, slashes, dots, etc. are not part of [A-Za-z0-9_-].
        assert_eq!(
            err_status(validate_create_product_request(&req("Sensor", "sensor v1"))),
            Some(400)
        );
        assert_eq!(
            err_status(validate_create_product_request(&req("Sensor", "sensor/v1"))),
            Some(400)
        );
        assert_eq!(
            err_status(validate_create_product_request(&req("Sensor", "sensor.v1"))),
            Some(400)
        );
    }

    #[test]
    fn name_at_max_length_and_model_no_with_dashes_underscore_accepted() {
        let max_name = "a".repeat(128);
        assert!(validate_create_product_request(&req(&max_name, "sensor_v1-rc")).is_ok());
    }

    fn update_req(name: &str) -> UpdateProductRequest {
        UpdateProductRequest {
            name: name.into(),
            description: String::new(),
            auto_provisioning: false,
        }
    }

    #[test]
    fn update_valid_request_passes() {
        assert!(validate_update_product_request(&update_req("Sensor")).is_ok());
    }

    #[test]
    fn update_empty_name_rejected() {
        assert_eq!(
            err_status(validate_update_product_request(&update_req("   "))),
            Some(400)
        );
    }

    #[test]
    fn update_overlong_name_rejected() {
        let long = "a".repeat(129);
        assert_eq!(
            err_status(validate_update_product_request(&update_req(&long))),
            Some(400)
        );
    }
}
