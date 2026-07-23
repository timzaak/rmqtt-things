use crate::api::ApiState;
use crate::api::error::ApiError;
use crate::db::models::RegistrationSource;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use rand::distr::{Alphanumeric, SampleString};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{info, warn};
use utoipa::ToSchema;

type HmacSha1 = Hmac<Sha1>;

#[derive(Serialize, ToSchema)]
pub struct AuthConfigResponse {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub herald_login_url: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/auth/config",
    tag = "system",
    responses((status = 200, description = "Auth configuration", body = AuthConfigResponse))
)]
pub async fn get_auth_config(State(state): State<Arc<ApiState>>) -> Json<AuthConfigResponse> {
    let herald = &state.admin.config.herald;
    Json(AuthConfigResponse {
        enabled: herald.is_some(),
        login_url: herald.as_ref().map(|_| "/api/auth/oauth/start".to_string()),
        herald_login_url: herald.as_ref().map(|h| {
            format!(
                "{}/{}/auth/login",
                h.base_url.trim_end_matches('/'),
                h.realm_id
            )
        }),
    })
}

#[derive(Deserialize)]
pub struct OAuthStartQuery {
    redirect: Option<String>,
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    code: String,
    state: String,
}

/// Token-family response from Herald. Used for both the OAuth token exchange
/// (`POST /api/oauth/{realm}/token`, snake_case per RFC 6749) and the browser
/// refresh (`POST /api/auth/browser-token/refresh`, camelCase since `3d0d6ff7`)
/// — the two endpoints return the same five fields under different JSON casing,
/// so each field aliases its camelCase spelling to accept either form.
///
/// Herald 0.3.3 returns a short-lived access token (default 900s) plus a
/// long-lived refresh token (default 30d, config 1–90d).
/// `expires_in`/`refresh_expires_in` fit in i64 (max 7_776_000 < i64::MAX).
#[derive(Deserialize)]
struct TokenResponse {
    #[serde(alias = "accessToken")]
    access_token: String,
    #[serde(alias = "refreshToken")]
    refresh_token: String,
    #[allow(dead_code)]
    #[serde(alias = "tokenType")]
    token_type: String,
    #[serde(alias = "expiresIn")]
    expires_in: i64,
    #[serde(alias = "refreshExpiresIn")]
    refresh_expires_in: i64,
}

/// Response body of our `/api/auth/refresh` endpoint. The web console schedules
/// its next proactive refresh from `expires_in` (well before the 15-minute
/// access-token boundary so we never race the token expiry).
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResponse {
    pub expires_in: i64,
}

#[derive(Serialize, Deserialize)]
struct OAuthCookie {
    state: String,
    code_verifier: String,
    return_to: String,
    redirect_uri: String,
}

#[utoipa::path(
    get,
    path = "/api/auth/oauth/start",
    tag = "system",
    params(("redirect" = Option<String>, Query, description = "Original app URL")),
    responses((status = 302, description = "Redirect to Herald OAuth authorize endpoint"))
)]
pub async fn oauth_start(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<OAuthStartQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(herald) = state.admin.config.herald.as_ref() else {
        return Err(ApiError::not_found("Herald auth is not configured"));
    };

    let (app_origin, return_to) = resolve_app_origin_and_return_to(&headers, query.redirect)?;
    let redirect_uri = format!("{app_origin}/api/auth/oauth/callback");
    let oauth_state = random_token(32);
    let code_verifier = random_token(64);
    let code_challenge = pkce_challenge(&code_verifier);

    let authorize_url = build_authorize_url(
        herald.base_url.trim_end_matches('/'),
        &herald.realm_id,
        &herald.client_id,
        &redirect_uri,
        &oauth_state,
        &code_challenge,
    )?;

    let oauth_cookie = encode_oauth_cookie(&OAuthCookie {
        state: oauth_state,
        code_verifier,
        return_to,
        redirect_uri,
    })?;

    Ok((
        StatusCode::FOUND,
        [
            (header::LOCATION, authorize_url),
            (
                header::SET_COOKIE,
                build_cookie("RMQTT_OAUTH", &oauth_cookie, 300),
            ),
        ],
    ))
}

#[utoipa::path(
    get,
    path = "/api/auth/oauth/callback",
    tag = "system",
    params(
        ("code" = String, Query, description = "OAuth authorization code"),
        ("state" = String, Query, description = "OAuth state")
    ),
    responses((status = 302, description = "Set app auth cookie and redirect to original app path"))
)]
pub async fn oauth_callback(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(herald) = state.admin.config.herald.as_ref() else {
        return Err(ApiError::not_found("Herald auth is not configured"));
    };

    let oauth_cookie_value =
        get_cookie(&headers, "RMQTT_OAUTH").ok_or_else(ApiError::unauthorized)?;
    let oauth_cookie = decode_oauth_cookie(&oauth_cookie_value)?;
    if oauth_cookie.state != query.state {
        return Err(ApiError::unauthorized());
    }

    let token = exchange_oauth_code(
        herald.base_url.trim_end_matches('/'),
        &herald.realm_id,
        &herald.client_id,
        &query.code,
        &oauth_cookie.redirect_uri,
        &oauth_cookie.code_verifier,
    )
    .await?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::LOCATION,
        HeaderValue::from_str(&oauth_cookie.return_to)
            .map_err(|_| ApiError::internal("invalid OAuth return path"))?,
    );
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&build_cookie(
            "X-Auth",
            &token.access_token,
            token.expires_in,
        ))
        .map_err(|_| ApiError::internal("invalid auth cookie"))?,
    );
    // Refresh token rides in its own HttpOnly cookie; its Max-Age is the
    // browser token family's absolute lifetime (default 30d). The access token
    // in X-Auth expires much sooner (900s), so the web console must call
    // /api/auth/refresh before X-Auth lapses to avoid a 15-min forced re-login.
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&build_cookie(
            "X-Auth-Refresh",
            &token.refresh_token,
            token.refresh_expires_in,
        ))
        .map_err(|_| ApiError::internal("invalid refresh cookie"))?,
    );
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_cookie("RMQTT_OAUTH"))
            .map_err(|_| ApiError::internal("invalid OAuth cookie"))?,
    );

    Ok((StatusCode::FOUND, response_headers))
}

/// Refresh the browser access token using the `X-Auth-Refresh` cookie.
///
/// Herald 0.3.3 issues short-lived (900s) access tokens alongside a long-lived
/// refresh token. Both tokens rotate on every refresh, and **reusing a rotated
/// refresh token revokes the whole token family** — so the web console must
/// proactively call this endpoint before `X-Auth` lapses (single in-flight
/// refresh), never retry concurrently after a 401.
#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    tag = "system",
    responses(
        (status = 200, description = "Rotated access token; cookies refreshed", body = RefreshResponse),
        (status = 401, description = "Refresh token missing or rejected by Herald")
    )
)]
pub async fn refresh_token(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let Some(herald) = state.admin.config.herald.as_ref() else {
        return Err(ApiError::not_found("Herald auth is not configured"));
    };
    let refresh_cookie =
        get_cookie(&headers, "X-Auth-Refresh").ok_or_else(ApiError::unauthorized)?;

    let tokens =
        refresh_oauth_token(herald.base_url.trim_end_matches('/'), &refresh_cookie).await?;

    let mut response_headers = HeaderMap::new();
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&build_cookie(
            "X-Auth",
            &tokens.access_token,
            tokens.expires_in,
        ))
        .map_err(|_| ApiError::internal("invalid auth cookie"))?,
    );
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&build_cookie(
            "X-Auth-Refresh",
            &tokens.refresh_token,
            tokens.refresh_expires_in,
        ))
        .map_err(|_| ApiError::internal("invalid refresh cookie"))?,
    );

    Ok((
        StatusCode::OK,
        response_headers,
        Json(RefreshResponse {
            expires_in: tokens.expires_in,
        }),
    ))
}

/// Log out: revoke the Herald token family, then clear both auth cookies.
///
/// Revocation is best-effort — if Herald is unreachable we still clear the
/// cookies so the user's session ends locally; the orphaned family dies on its
/// own when the refresh absolute TTL expires.
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "system",
    responses((status = 200, description = "Logged out; cookies cleared"))
)]
pub async fn logout(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let Some(herald) = state.admin.config.herald.as_ref() else {
        return Err(ApiError::not_found("Herald auth is not configured"));
    };
    if let Some(access_token) = get_cookie(&headers, "X-Auth") {
        // Herald's /api/auth/logout is Bearer-protected; a 4xx/5xx means the
        // token was already gone or the service is down. Either way we still
        // drop the cookies below so the client session ends locally.
        let _ = herald_logout(herald.base_url.trim_end_matches('/'), &access_token).await;
    }

    let mut response_headers = HeaderMap::new();
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_cookie("X-Auth"))
            .map_err(|_| ApiError::internal("invalid auth cookie"))?,
    );
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_cookie("X-Auth-Refresh"))
            .map_err(|_| ApiError::internal("invalid refresh cookie"))?,
    );

    Ok((
        StatusCode::OK,
        response_headers,
        Json(serde_json::json!({"message": "ok"})),
    ))
}

fn random_token(len: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), len)
}

fn pkce_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn encode_oauth_cookie(cookie: &OAuthCookie) -> Result<String, ApiError> {
    let json = serde_json::to_string(cookie)
        .map_err(|_| ApiError::internal("failed to serialize OAuth state"))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn decode_oauth_cookie(value: &str) -> Result<OAuthCookie, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::unauthorized())?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::unauthorized())
}

fn build_authorize_url(
    herald_base_url: &str,
    realm_id: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<String, ApiError> {
    let mut url = Url::parse(&format!("{herald_base_url}/api/oauth/{realm_id}/authorize"))
        .map_err(|_| ApiError::internal("invalid Herald OAuth authorize URL"))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state)
        .append_pair("response_type", "code")
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.to_string())
}

async fn exchange_oauth_code(
    herald_base_url: &str,
    realm_id: &str,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<TokenResponse, ApiError> {
    let url = format!("{herald_base_url}/api/oauth/{realm_id}/token");
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
            "client_id": client_id,
            "code_verifier": code_verifier,
        }))
        .send()
        .await
        .map_err(|_| ApiError::service_unavailable("auth service unavailable"))?;

    if !response.status().is_success() {
        return Err(ApiError::unauthorized());
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|_| ApiError::service_unavailable("auth service unavailable"))
}

async fn refresh_oauth_token(
    herald_base_url: &str,
    refresh_token: &str,
) -> Result<TokenResponse, ApiError> {
    let url = format!("{herald_base_url}/api/auth/browser-token/refresh");
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({ "refreshToken": refresh_token }))
        .send()
        .await
        .map_err(|_| ApiError::service_unavailable("auth service unavailable"))?;

    // Herald maps invalid/reused refresh tokens to 401; both mean the session
    // is gone and the client must re-authenticate.
    if !response.status().is_success() {
        return Err(ApiError::unauthorized());
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|_| ApiError::service_unavailable("auth service unavailable"))
}

/// Best-effort family revocation via Herald's Bearer-protected logout endpoint.
/// Any error (network failure, 4xx for already-revoked tokens) is swallowed by
/// the caller so cookies still get cleared locally.
async fn herald_logout(herald_base_url: &str, access_token: &str) -> Result<(), ApiError> {
    let url = format!("{herald_base_url}/api/auth/logout");
    reqwest::Client::new()
        .post(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .send()
        .await
        .map_err(|_| ApiError::service_unavailable("auth service unavailable"))?;
    Ok(())
}

fn resolve_app_origin_and_return_to(
    headers: &HeaderMap,
    redirect: Option<String>,
) -> Result<(String, String), ApiError> {
    if let Some(redirect) = redirect {
        if let Ok(url) = Url::parse(&redirect) {
            let origin = url_origin(&url)?;
            let mut return_to = url.path().to_string();
            if let Some(query) = url.query() {
                return_to.push('?');
                return_to.push_str(query);
            }
            if let Some(fragment) = url.fragment() {
                return_to.push('#');
                return_to.push_str(fragment);
            }
            return Ok((origin, safe_return_path(&return_to).to_string()));
        }

        if redirect.starts_with('/') && !redirect.starts_with("//") {
            return Ok((
                request_origin(headers)?,
                safe_return_path(&redirect).to_string(),
            ));
        }
    }

    Ok((request_origin(headers)?, "/".to_string()))
}

fn url_origin(url: &Url) -> Result<String, ApiError> {
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ApiError::bad_request("invalid redirect URL scheme"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::bad_request("invalid redirect URL host"))?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    Ok(format!("{scheme}://{host}{port}"))
}

fn request_origin(headers: &HeaderMap) -> Result<String, ApiError> {
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("missing host header"))?;
    Ok(format!("{proto}://{host}"))
}

fn safe_return_path(path: &str) -> &str {
    if path.starts_with('/') && !path.starts_with("//") && !path.chars().any(char::is_control) {
        path
    } else {
        "/"
    }
}

fn build_cookie(name: &str, value: &str, max_age_seconds: i64) -> String {
    format!("{name}={value}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Lax")
}

fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|cookie| {
        let (cookie_name, value) = cookie.trim().split_once('=')?;
        (cookie_name == name && !value.is_empty()).then(|| value.to_string())
    })
}

#[derive(Deserialize, ToSchema, Debug)]
#[allow(dead_code)]
pub struct AuthPayload {
    // #[salvo(schema(max_length = 64))]
    pub client_id: String,
    // #[salvo(schema(max_length = 32))]
    #[serde(default)]
    pub username: Option<String>,
    // #[salvo(schema(max_length = 256))]
    #[serde(default)]
    pub password: String,
    pub protocol: serde_json::Value,
    pub ipaddress: String,
}

//const Subscribe:&str = "1";
// const Publish:&str = "2";

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
pub enum Access {
    #[serde(rename = "1")]
    Subscribe,
    #[serde[rename = "2"]]
    Publish,
}

#[derive(Deserialize, PartialEq, Debug, ToSchema)]
pub enum MqttProtocol {
    #[serde(rename = "3")]
    Mqttv3,
    #[serde(rename = "4")]
    MqttV311,
    #[serde(rename = "5")]
    MqttV5,
}

#[derive(Deserialize, ToSchema, Debug)]
#[allow(dead_code)]
pub struct AclPayload {
    pub access: Access,
    #[serde(default)]
    pub username: Option<String>,
    pub client_id: String,
    pub ip: String,
    pub topic: String,
    pub protocol: serde_json::Value,
}

#[utoipa::path(
    post,
    path = "/api/access/acl",
    tag = "access",
    request_body = AclPayload,
    responses((status = 200, description = "allow or deny", body = String))
)]
pub async fn acl(Json(payload): Json<AclPayload>) -> &'static str {
    // Strip leading '/' if present (backend publishes OTA to /{pid}/{did}/ota/upgrade)
    let topic = payload.topic.strip_prefix('/').unwrap_or(&payload.topic);
    let mut parts = topic.split('/');
    let (p0, p1, p2, p3) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return "deny",
    };

    if p1 != payload.client_id {
        return "deny";
    }

    if let Some(username) = &payload.username
        && p0 != username
    {
        return "deny";
    }

    // Allow thing topics: {product}/{device}/thing/{event|service|file|factory-metadata}/...
    //
    // `factory-metadata` (design §4.2.2 E note + §5.3) is the device pull
    // topic; `p1 == client_id` above already prevents cross-device reads
    // (design §6.3), and `p0 == username(product_id)` prevents cross-product.
    if p2 == "thing"
        && (p3 == "event" || p3 == "service" || p3 == "file" || p3 == "factory-metadata")
    {
        return "allow";
    }

    // Allow OTA topics: {product}/{device}/ota/upgrade or {product}/{device}/ota/version
    if p2 == "ota" && (p3 == "upgrade" || p3 == "version") {
        return "allow";
    }

    "deny"
}

#[utoipa::path(
    post,
    path = "/api/access/auth",
    tag = "access",
    request_body = AuthPayload,
    responses((status = 200, description = "allow or deny", body = String))
)]
pub async fn auth(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<AuthPayload>,
) -> &'static str {
    // Note: The check to see if a device is subscribed to its properties topic
    // is handled in the `create_property_command` function in `admin_handlers.rs`.
    // This is because we only need to check for the subscription when a command is being sent.
    // return "allow";
    let state = &state.app;
    let suffix = &state.config.mqtt.access.auth.suffix;

    // Validate input lengths
    if payload.client_id.len() > 64 || payload.password.len() > 256 || payload.password.is_empty() {
        return "deny";
    }

    // Deconstruct password
    let parts: Vec<&str> = payload.password.split('.').collect();
    if parts.len() != 3 {
        return "deny";
    }

    let nonce = parts[0];
    if nonce.len() != 6 {
        return "deny";
    }
    let timestamp_str = parts[1];
    let hash = parts[2];

    // Validate timestamp
    let timestamp: i64 = match timestamp_str.parse() {
        Ok(t) => t,
        Err(_) => return "deny",
    };

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let time_diff = (now - timestamp).abs();

    if time_diff > state.config.mqtt.access.auth.timestamp_tolerance_secs {
        warn!(
            clientid = %payload.client_id,
            time_diff = time_diff,
            "Timestamp out of range"
        );
        return "deny";
    }

    // Reconstruct and verify password
    let to_sign = format!(
        "{}.{}.{}.{}",
        payload.client_id, nonce, timestamp_str, suffix
    );

    let mac = HmacSha1::new_from_slice(suffix.as_bytes());
    let mac = match mac {
        Ok(mut mac) => {
            mac.update(to_sign.as_bytes());
            mac
        }
        Err(_) => return "deny",
    };
    let result = mac.finalize();
    let expected_hash = hex::encode(result.into_bytes());

    if expected_hash != hash {
        return "deny";
    }

    // Device admission check
    let product_id = match &payload.username {
        Some(pid) => pid.as_str(),
        None => {
            warn!(client_id = %payload.client_id, "Device admission denied: no product_id (username)");
            return "deny";
        }
    };
    check_device_admission(state, product_id, &payload.client_id).await
}

async fn check_device_admission(
    state: &crate::api::handlers::AppState,
    product_id: &str,
    device_id: &str,
) -> &'static str {
    let (device_exists, auto_provisioning) = match state
        .db
        .device()
        .admission_check(product_id, device_id)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            warn!(client_id = %device_id, product_id = %product_id, error = %e, "Device admission denied: DB error");
            return "deny";
        }
    };

    if device_exists {
        info!(client_id = %device_id, product_id = %product_id, "Device admission: already registered");
        return "allow";
    }

    match auto_provisioning {
        Some(true) => {
            if let Err(e) = state
                .db
                .device()
                .upsert(product_id, device_id, RegistrationSource::Auto)
                .await
            {
                warn!(client_id = %device_id, product_id = %product_id, error = %e, "Auto-provisioning failed");
                return "deny";
            }
            info!(client_id = %device_id, product_id = %product_id, "Auto-provisioned");
            "allow"
        }
        Some(false) => {
            warn!(client_id = %device_id, product_id = %product_id, "Device admission denied: auto-provisioning disabled");
            "deny"
        }
        None => {
            warn!(client_id = %device_id, product_id = %product_id, "Device admission denied: product not found");
            "deny"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use hex;
    use rand::distr::Alphanumeric;
    use rand::{Rng, rng};
    use serde_json::json;

    #[test]
    fn test_generate_password() {
        let client_id = "test_client";
        let suffix = "test_suffix";
        let (password, timestamp) = generate_test_password(client_id, suffix);

        let parts: Vec<&str> = password.split('.').collect();
        assert_eq!(parts.len(), 3);

        let nonce = parts[0];
        let timestamp_str = parts[1];
        let hash = parts[2];

        assert_eq!(nonce.len(), 6);
        assert_eq!(timestamp_str, timestamp.to_string());

        let to_sign = format!("{client_id}.{nonce}.{timestamp}.{suffix}");
        let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let result = mac.finalize();
        let expected_hash = hex::encode(result.into_bytes());

        assert_eq!(hash, expected_hash);
    }

    #[tokio::test]
    async fn test_acl_allows_device_thing_topic_for_own_product() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/demo-device/thing/event/property/post".to_string(),
            protocol: json!(4),
        };

        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_denies_cross_device_topic() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/other-device/thing/event/property/post".to_string(),
            protocol: json!(4),
        };

        assert_eq!(acl(Json(payload)).await, "deny");
    }

    #[tokio::test]
    async fn test_acl_allows_ota_upgrade_topic_with_leading_slash() {
        let payload = AclPayload {
            access: Access::Subscribe,
            username: Some("demo_product".to_string()),
            client_id: "demo-e2e-ota-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "/demo_product/demo-e2e-ota-device/ota/upgrade".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_allows_ota_version_topic() {
        let payload = AclPayload {
            access: Access::Publish,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "demo_product/demo-device/ota/version".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "allow");
    }

    #[tokio::test]
    async fn test_acl_denies_ota_for_wrong_device() {
        let payload = AclPayload {
            access: Access::Subscribe,
            username: Some("demo_product".to_string()),
            client_id: "demo-device".to_string(),
            ip: "127.0.0.1".to_string(),
            topic: "/demo_product/other-device/ota/upgrade".to_string(),
            protocol: json!(4),
        };
        assert_eq!(acl(Json(payload)).await, "deny");
    }

    fn generate_test_password(client_id: &str, suffix: &str) -> (String, i64) {
        let nonce: String = rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();

        let to_sign = format!("{client_id}.{nonce}.{timestamp}.{suffix}");

        let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let result = mac.finalize();
        let hash = hex::encode(result.into_bytes());

        (format!("{nonce}.{timestamp}.{hash}"), timestamp)
    }

    #[test]
    fn oauth_builds_pkce_challenge() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn oauth_resolves_absolute_redirect_to_app_origin_and_path() {
        let headers = HeaderMap::new();
        let (origin, return_to) = resolve_app_origin_and_return_to(
            &headers,
            Some("http://localhost:3000/devices?status=Online#row-1".to_string()),
        )
        .unwrap();

        assert_eq!(origin, "http://localhost:3000");
        assert_eq!(return_to, "/devices?status=Online#row-1");
    }

    #[test]
    fn oauth_resolves_relative_redirect_from_request_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost:8080"));

        let (origin, return_to) =
            resolve_app_origin_and_return_to(&headers, Some("/products".to_string())).unwrap();

        assert_eq!(origin, "http://localhost:8080");
        assert_eq!(return_to, "/products");
    }

    #[test]
    fn oauth_cookie_round_trips() {
        let cookie = OAuthCookie {
            state: "state".to_string(),
            code_verifier: "verifier".to_string(),
            return_to: "/devices?status=Online".to_string(),
            redirect_uri: "http://localhost:3000/api/auth/oauth/callback".to_string(),
        };

        let encoded = encode_oauth_cookie(&cookie).unwrap();
        let decoded = decode_oauth_cookie(&encoded).unwrap();

        assert_eq!(decoded.state, cookie.state);
        assert_eq!(decoded.code_verifier, cookie.code_verifier);
        assert_eq!(decoded.return_to, cookie.return_to);
        assert_eq!(decoded.redirect_uri, cookie.redirect_uri);
    }
}
