use crate::api::error::ApiError;
use axum::extract::State;
use axum::http::{Method, Request, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use herald_sdk::{Client, Error, PermissionCheckRequest, Rule};
use std::net::{IpAddr, Ipv6Addr};
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
        // Session not found/expired returns allowed=false with no user_id
        Ok(permission) if permission.user_id.is_none() => ApiError::unauthorized().into_response(),
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
        || path.starts_with("/admin/alarm-rule")
        || path.starts_with("/admin/alarm")
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

/// Middleware that rejects requests from non-private (public) IP addresses.
/// Checks X-Real-IP and X-Forwarded-For headers for the client IP.
/// Returns 403 Forbidden if the IP is public or cannot be determined.
pub async fn internal_ip_middleware(request: Request<axum::body::Body>, next: Next) -> Response {
    let ip = extract_client_ip(&request);
    match ip {
        Some(ip) if is_private_ip(&ip) => next.run(request).await,
        _ => ApiError::forbidden_with("access denied: internal network only").into_response(),
    }
}

fn extract_client_ip(request: &Request<axum::body::Body>) -> Option<IpAddr> {
    // Prefer X-Real-IP header
    if let Some(real_ip) = request
        .headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return Some(real_ip);
    }

    // Fall back to first entry in X-Forwarded-For
    if let Some(xff) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        && let Some(first) = xff.split(',').next()
        && let Ok(ip) = first.trim().parse::<IpAddr>()
    {
        return Some(ip);
    }

    // Fall back to connection info (direct connection without proxy)
    if let Some(connect_info) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return Some(connect_info.0.ip());
    }

    None
}

/// Returns true if the IP address belongs to a private/reserved range:
/// - 127.0.0.0/8 (loopback)
/// - 10.0.0.0/8 (class A private)
/// - 172.16.0.0/12 (class B private)
/// - 192.168.0.0/16 (class C private)
/// - ::1 (IPv6 loopback)
/// - fc00::/7 (IPv6 unique local)
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 127.0.0.0/8
            octets[0] == 127
            // 10.0.0.0/8
            || octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && (octets[1] & 0xf0) == 16)
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
        }
        IpAddr::V6(v6) => {
            // ::1 loopback
            *v6 == Ipv6Addr::LOCALHOST
            // fc00::/7 (unique local: fc00:: and fd00:: ranges)
            || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Json;
    use axum::Router;
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
            ("/api/admin/alarm-rule", Method::POST, "device", "write"),
            ("/api/admin/alarm-rule/1", Method::GET, "device", "read"),
            (
                "/api/admin/alarm-rule/1/status",
                Method::PATCH,
                "device",
                "write",
            ),
            ("/api/admin/alarm", Method::GET, "device", "read"),
            ("/api/admin/alarm/1/ack", Method::PATCH, "device", "write"),
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
    async fn auth_returns_unauthorized_when_session_not_found() {
        let base_url = spawn_herald_mock(StatusCode::OK, Json(json!({"allowed": false}))).await;
        let app = protected_admin_router(base_url);
        let response = app.oneshot(admin_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_returns_forbidden_when_user_lacks_permission() {
        let base_url = spawn_herald_mock(
            StatusCode::OK,
            Json(json!({"allowed": false, "userId": "user-1"})),
        )
        .await;
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
            client_id: Arc::from("admin-web-console"),
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

    // --- Internal IP middleware tests ---

    fn internal_ip_router() -> Router {
        Router::new()
            .route("/test", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn(internal_ip_middleware))
    }

    async fn send_with_ip(ip: Option<&str>, header_name: &str) -> StatusCode {
        let mut builder = Request::builder().uri("/test");
        if let Some(ip) = ip {
            builder = builder.header(header_name, ip);
        }
        internal_ip_router()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[test]
    fn private_ip_check_loopback_v4() {
        assert!(is_private_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"127.255.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn private_ip_check_class_a() {
        assert!(is_private_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"10.255.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn private_ip_check_class_b() {
        assert!(is_private_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"172.31.255.255".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"172.32.0.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn private_ip_check_class_c() {
        assert!(is_private_ip(&"192.168.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"192.168.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn private_ip_check_loopback_v6() {
        assert!(is_private_ip(&"::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn private_ip_check_unique_local_v6() {
        assert!(is_private_ip(&"fc00::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"fd12:3456::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn public_ips_are_not_private() {
        assert!(!is_private_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"1.2.3.4".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"172.15.0.1".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"2001:db8::1".parse::<IpAddr>().unwrap()));
    }

    #[tokio::test]
    async fn internal_ip_allows_localhost_via_x_real_ip() {
        assert_eq!(
            send_with_ip(Some("127.0.0.1"), "x-real-ip").await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn internal_ip_allows_10_via_x_real_ip() {
        assert_eq!(
            send_with_ip(Some("10.0.0.1"), "x-real-ip").await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn internal_ip_allows_192_168_via_x_forwarded_for() {
        assert_eq!(
            send_with_ip(Some("192.168.1.100"), "x-forwarded-for").await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn internal_ip_allows_172_16_via_x_forwarded_for() {
        assert_eq!(
            send_with_ip(Some("172.16.5.5"), "x-forwarded-for").await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn internal_ip_rejects_public_ip() {
        assert_eq!(
            send_with_ip(Some("8.8.8.8"), "x-real-ip").await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn internal_ip_rejects_missing_ip() {
        assert_eq!(send_with_ip(None, "x-real-ip").await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn internal_ip_allows_connect_info_fallback() {
        use axum::extract::ConnectInfo;
        let mut req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                12345,
            ))));
        let resp = internal_ip_router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn internal_ip_rejects_public_connect_info() {
        use axum::extract::ConnectInfo;
        let mut req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(std::net::SocketAddr::from((
                [8, 8, 8, 8],
                12345,
            ))));
        let resp = internal_ip_router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
