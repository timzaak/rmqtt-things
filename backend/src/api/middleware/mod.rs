use crate::api::error::ApiError;
use axum::extract::State;
use axum::http::{Method, Request, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use herald_sdk::{Client, Error, PermissionCheckRequest, Rule};
use std::sync::Arc;

#[derive(Clone)]
pub struct HeraldAuthState {
    pub herald_sdk: Arc<Client>,
    pub client_id: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    #[allow(dead_code)]
    pub user_id: String,
}

pub async fn herald_auth_middleware(
    State(auth_state): State<HeraldAuthState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(token) = extract_auth_token(&request) else {
        return ApiError::unauthorized().into_response();
    };
    let Some(rule) = extract_permission(request.uri().path(), request.method()) else {
        return ApiError::forbidden().into_response();
    };

    let response = auth_state
        .herald_sdk
        .check_permission(PermissionCheckRequest {
            token,
            rules: Some(vec![rule]),
            client_id: auth_state.client_id.to_string(),
        })
        .await;

    match response {
        Ok(permission) if permission.allowed => {
            let Some(user_id) = permission.user_id else {
                return ApiError::unauthorized().into_response();
            };
            request.extensions_mut().insert(CurrentUser { user_id });
            next.run(request).await
        }
        Ok(_) => ApiError::forbidden().into_response(),
        Err(error) => classify_auth_error(&error).into_response(),
    }
}

pub fn extract_permission(path: &str, method: &Method) -> Option<Rule> {
    // Strip /api prefix if present — the middleware runs inside a nest("/api", …)
    // so the path may arrive as either "/admin/…" or "/api/admin/…".
    let path = path.strip_prefix("/api").unwrap_or(path);

    let resource = if path.starts_with("/admin/product")
        || path.starts_with("/admin/valid")
        || path.starts_with("/admin/file")
    {
        "product"
    } else if path.starts_with("/admin/device")
        || path.starts_with("/admin/property")
        || path.starts_with("/admin/event")
    {
        "device"
    } else if path.starts_with("/admin/ca") || path.starts_with("/admin/ota") {
        "cert"
    } else {
        return None;
    };

    let action = match *method {
        Method::GET => "read",
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE => "write",
        _ => return None,
    };

    Some(Rule {
        resource: resource.to_string(),
        action: action.to_string(),
    })
}

fn classify_auth_error(error: &Error) -> ApiError {
    match error {
        Error::Unauthorized(_) => ApiError::unauthorized(),
        Error::Forbidden(_) => ApiError::forbidden(),
        _ => ApiError::service_unavailable("auth service unavailable"),
    }
}

fn extract_auth_token(request: &Request<axum::body::Body>) -> Option<String> {
    let cookies = request.headers().get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == "X-Auth" && !value.is_empty()).then(|| value.to_string())
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::Json;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::routing::{get, post};
    use serde_json::json;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    #[test]
    fn auth_extracts_expected_admin_permissions() {
        let cases = [
            ("/api/admin/product", Method::GET, "product", "read"),
            ("/admin/product", Method::GET, "product", "read"),
            ("/api/admin/product/1", Method::POST, "product", "write"),
            ("/admin/product/1", Method::POST, "product", "write"),
            ("/api/admin/valid/event", Method::PATCH, "product", "write"),
            ("/api/admin/file/upload", Method::DELETE, "product", "write"),
            ("/api/admin/device/status", Method::GET, "device", "read"),
            (
                "/api/admin/property/command",
                Method::POST,
                "device",
                "write",
            ),
            ("/api/admin/event", Method::DELETE, "device", "write"),
            ("/api/admin/ca/cert", Method::GET, "cert", "read"),
            ("/api/admin/ota/version", Method::PUT, "cert", "write"),
        ];

        for (path, method, resource, action) in cases {
            let rule = extract_permission(path, &method).unwrap();
            assert_eq!(rule.resource, resource);
            assert_eq!(rule.action, action);
        }
    }

    #[test]
    fn auth_ignores_unprotected_or_unsupported_paths() {
        assert!(extract_permission("/api/health", &Method::GET).is_none());
        assert!(extract_permission("/api/access/auth", &Method::POST).is_none());
        assert!(extract_permission("/api/thing/property/post", &Method::POST).is_none());
        assert!(extract_permission("/api/device/connect", &Method::POST).is_none());
        assert!(extract_permission("/api/admin/product", &Method::HEAD).is_none());
    }

    #[tokio::test]
    async fn auth_rejects_missing_cookie() {
        let app = protected_admin_router("http://127.0.0.1:9".to_string());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/product")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_request_when_herald_allows_permission() {
        let base_url = spawn_herald_mock(
            StatusCode::OK,
            Json(json!({"allowed": true, "userId": "user-1"})),
        )
        .await;
        let app = protected_admin_router(base_url);
        let response = app.oneshot(admin_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_rejects_request_when_herald_denies_permission() {
        let base_url = spawn_herald_mock(StatusCode::OK, Json(json!({"allowed": false}))).await;
        let app = protected_admin_router(base_url);
        let response = app.oneshot(admin_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn auth_returns_service_unavailable_when_herald_fails() {
        let base_url = spawn_herald_mock(
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "down"})),
        )
        .await;
        let app = protected_admin_router(base_url);
        let response = app.oneshot(admin_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    fn protected_admin_router(base_url: String) -> Router {
        let auth_state = HeraldAuthState {
            herald_sdk: Arc::new(Client::new(
                base_url,
                "test-api-key".to_string(),
                Some(Duration::from_secs(1)),
            )),
            client_id: Arc::from("rmqtt-things-admin"),
        };

        Router::new()
            .route("/api/admin/product", get(|| async { StatusCode::OK }))
            .layer(from_fn_with_state(auth_state, herald_auth_middleware))
    }

    fn admin_request() -> Request<Body> {
        Request::builder()
            .uri("/api/admin/product")
            .header(header::COOKIE, "X-Auth=session-token")
            .body(Body::empty())
            .unwrap()
    }

    async fn spawn_herald_mock(status: StatusCode, body: Json<serde_json::Value>) -> String {
        let app = Router::new().route(
            "/api/ext/permission/check",
            post(move || {
                let body = body.clone();
                async move { (status, body) }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        format!("http://{}", address)
    }
}
