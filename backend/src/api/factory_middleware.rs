//! Factory (production-line) API key middleware (support-multiple-device
//! feature, design §5.2).
//!
//! Backs the `/api/factory/*` write path with an authentication scheme that is
//! fully isolated from the three existing mechanisms (Herald OAuth, HMAC device
//! certs, internal-IP allow-list). The API key is supplied via a `Bearer`
//! header and compared against `[factory] api_keys` in constant time.
//!
//! Behavioural invariants enforced here:
//! - Empty `api_keys` (the default when `[factory]` is absent) rejects every
//!   request with 401. The routes still exist, so an unconfigured deployment
//!   surfaces a clear authentication failure rather than a 503 (design §5.4).
//! - The comparison uses `constant_time_eq` to avoid timing side-channels on
//!   the static API key list — a deliberate improvement over the existing HMAC
//!   comparison (design §4.5).
//! - On success the middleware inserts a `FactoryCaller` into request
//!   extensions so downstream handlers / future audit hooks can attribute the
//!   write to the factory path.

use crate::api::error::ApiError;
use axum::extract::State;
use axum::http::{HeaderMap, Request, header::AUTHORIZATION};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

/// State held by `factory_auth_middleware`. `api_keys` mirrors
/// `[factory] api_keys` from the config; an empty list means every request is
/// rejected (design §5.4).
#[derive(Clone)]
pub struct FactoryAuthState {
    pub api_keys: Arc<[Box<str>]>,
}

/// Caller identity inserted into request extensions on success. Reserved for
/// future audit / change-log attribution; the current factory write path
/// always runs under the fixed label `"factory"` (no per-key identity yet).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FactoryCaller {
    pub label: Box<str>,
}

/// Authorise a `/api/factory/*` request.
///
/// Extracts `Authorization: Bearer <key>` and constant-time-compares it against
/// each configured key. On success inserts `FactoryCaller` into extensions and
/// forwards the request. On any failure (missing/malformed header, no key
/// match, empty configured list) returns 401 with message
/// `"Invalid factory API key"`.
pub async fn factory_auth_middleware(
    State(state): State<Arc<FactoryAuthState>>,
    headers: HeaderMap,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized_with("Invalid factory API key"))?;

    // Constant-time comparison against each configured key. Iteration order
    // leaks only how many keys are configured (already known from config), not
    // the key bytes themselves.
    let ok = state
        .api_keys
        .iter()
        .any(|k| constant_time_eq::constant_time_eq(bearer.as_bytes(), k.as_bytes()));

    if !ok {
        return Err(ApiError::unauthorized_with("Invalid factory API key"));
    }

    req.extensions_mut().insert(FactoryCaller {
        label: "factory".into(),
    });
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::routing::post;
    use tower::ServiceExt;

    fn factory_router(api_keys: &[&str]) -> Router {
        let keys: Arc<[Box<str>]> = api_keys
            .iter()
            .map(|s| -> Box<str> { (**s).into() })
            .collect::<Vec<Box<str>>>()
            .into();
        let state = Arc::new(FactoryAuthState { api_keys: keys });
        Router::new()
            .route("/factory/echo", post(|| async { StatusCode::OK }))
            .layer(from_fn_with_state(state, factory_auth_middleware))
    }

    fn factory_request(bearer: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method("POST").uri("/factory/echo");
        if let Some(bearer) = bearer {
            builder = builder.header(AUTHORIZATION, format!("Bearer {bearer}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn empty_key_list_rejects_everything() {
        // Mirrors the `[factory]` absent deployment: routes exist but every
        // request is 401 (design §5.4).
        let app = factory_router(&[]);
        assert_eq!(
            app.clone()
                .oneshot(factory_request(Some("any")))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.oneshot(factory_request(None)).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn missing_authorization_header_is_unauthorized() {
        let app = factory_router(&["secret"]);
        let resp = app.oneshot(factory_request(None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_bearer_is_unauthorized() {
        let app = factory_router(&["secret"]);
        let resp = app.oneshot(factory_request(Some("nope"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn non_bearer_scheme_is_unauthorized() {
        let app = factory_router(&["secret"]);
        // build a Basic-style header directly to confirm only Bearer is honoured
        let req = Request::builder()
            .method("POST")
            .uri("/factory/echo")
            .header(AUTHORIZATION, "Basic secret")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn valid_bearer_passes() {
        let app = factory_router(&["secret", "backup"]);
        let resp = app.oneshot(factory_request(Some("backup"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
