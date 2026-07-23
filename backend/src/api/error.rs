use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorResponse {
    pub error: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// Status code carried by this error. Exposed so unit tests can assert the
    /// HTTP contract (e.g. that `validate_file_key` rejects with 400) without
    /// constructing a full `Response`. Kept public for future handler-side
    /// introspection; `#[allow(dead_code)]` silences the prod-only-unused
    /// warning until a non-test caller appears.
    #[allow(dead_code)]
    pub fn status_code(&self) -> StatusCode {
        self.status
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    /// Like `unauthorized()` but with a custom message. Symmetric with
    /// `forbidden_with`; used by `factory_auth_middleware` (design §5.2) to
    /// surface "Invalid factory API key" instead of the generic "unauthorized".
    pub fn unauthorized_with(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    pub fn forbidden_with(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }

    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

/// Helper to map database errors into a consistent 500 response.
/// Use via `.map_err(map_db_err)?` in handler code.
pub fn map_db_err(e: impl std::fmt::Display) -> ApiError {
    tracing::error!("Database error: {e}");
    ApiError::internal("Database operation failed")
}
