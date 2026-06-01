use crate::api::ApiState;
use crate::api::admin_models::{PaginatedResponse, PaginationInfo, ProductQuery};
use crate::api::error::ApiError;
use crate::db::models::{CreateProductRequest, Product, UpdateProductRequest};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use std::sync::Arc;
use tracing::error;

/// PostgreSQL error code for unique constraint violation.
const PG_UNIQUE_VIOLATION: &str = "23505";

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
