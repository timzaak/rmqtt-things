//! Scenario tests for certificate issuance.
//!
//! Covers user story: CA cert issuance returns both cert_pem and key_pem.
//!
//! Acceptance criteria:
//! - Successful issuance returns both cert_pem and key_pem in JSON response
//! - Both fields contain valid PEM content (start with "-----BEGIN")
//! - key_pem is NOT stored in the database (pub_cert field only has the cert)

use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use crate::api::ca_handlers::IssueCertRequest;
use axum::http::{Method, StatusCode};
use test_context::test_context;
use time::{Duration, OffsetDateTime};

/// Verifies that issuing a certificate returns both cert_pem and key_pem
/// in the JSON response, each containing valid PEM content.
///
/// User story path: CA cert issuance
/// Acceptance criteria: AC-1 (response includes cert_pem and key_pem),
///                       AC-2 (both fields contain valid PEM content)
#[test_context(TestContext)]
#[tokio::test]
async fn test_scenario_cert_issue_returns_cert_and_key_pem(ctx: &mut TestContext) {
    let device_id = "scenario_device_001".to_string();
    let product_id = "scenario_product_001".to_string();

    let issue_req = IssueCertRequest {
        product_id: product_id.clone(),
        device_id: device_id.clone(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };

    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/ca/cert", &issue_req).await;

    assert_eq!(status, StatusCode::OK, "Expected 200 OK, got body: {body}");

    let resp: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("Failed to parse response as JSON: {e}\nbody: {body}"));

    // AC-1: response contains both fields
    let cert_pem = resp
        .get("cert_pem")
        .and_then(|v| v.as_str())
        .expect("Response must contain cert_pem string");
    let key_pem = resp
        .get("key_pem")
        .and_then(|v| v.as_str())
        .expect("Response must contain key_pem string");

    // AC-2: both fields contain valid PEM content
    assert!(
        cert_pem.starts_with("-----BEGIN CERTIFICATE-----"),
        "cert_pem must start with PEM header, got: {}",
        &cert_pem[..cert_pem.len().min(40)]
    );
    assert!(
        key_pem.starts_with("-----BEGIN"),
        "key_pem must start with PEM header, got: {}",
        &key_pem[..key_pem.len().min(40)]
    );
    assert!(
        cert_pem.contains("-----END CERTIFICATE-----"),
        "cert_pem must contain PEM footer"
    );
    assert!(
        key_pem.contains("-----END"),
        "key_pem must contain PEM footer"
    );
}

/// Verifies that the private key (key_pem) is NOT stored in the database.
/// Only the public certificate (cert_pem) should be stored in pub_cert.
///
/// User story path: CA cert issuance
/// Acceptance criteria: AC-3 (key_pem not stored in DB)
#[test_context(TestContext)]
#[tokio::test]
async fn test_scenario_cert_issue_key_not_stored_in_db(ctx: &mut TestContext) {
    let device_id = "scenario_device_002".to_string();
    let product_id = "scenario_product_002".to_string();

    let issue_req = IssueCertRequest {
        product_id: product_id.clone(),
        device_id: device_id.clone(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };

    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/ca/cert", &issue_req).await;

    assert_eq!(status, StatusCode::OK, "Expected 200 OK, got body: {body}");

    let resp: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("Failed to parse response as JSON: {e}\nbody: {body}"));

    let cert_pem = resp
        .get("cert_pem")
        .and_then(|v| v.as_str())
        .expect("Response must contain cert_pem");
    let key_pem = resp
        .get("key_pem")
        .and_then(|v| v.as_str())
        .expect("Response must contain key_pem");

    // Fetch the stored cert from DB
    let stored_cert = ctx
        ._admin_state
        .db
        .cert_issue()
        .find_by_device_id(&product_id, &device_id)
        .await
        .expect("DB query should succeed")
        .expect("Cert should exist in DB");

    // AC-3: pub_cert contains only the certificate, not the key
    assert_eq!(
        stored_cert.pub_cert, cert_pem,
        "DB pub_cert must match the issued cert_pem exactly"
    );
    assert!(
        !stored_cert.pub_cert.contains(key_pem),
        "DB pub_cert must NOT contain the private key"
    );
    assert!(
        !stored_cert.pub_cert.contains("PRIVATE KEY"),
        "DB pub_cert must not contain any private key content"
    );
}

/// Verifies the full certificate lifecycle: issue, verify response shape,
/// list via API, and check DB state consistency.
///
/// User story path: CA cert issuance - full lifecycle
#[test_context(TestContext)]
#[tokio::test]
async fn test_scenario_cert_issue_full_lifecycle(ctx: &mut TestContext) {
    let device_id = "scenario_device_003".to_string();
    let product_id = "scenario_product_003".to_string();

    // 1. Issue a certificate
    let issue_req = IssueCertRequest {
        product_id: product_id.clone(),
        device_id: device_id.clone(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };

    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/ca/cert", &issue_req).await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cert_pem = resp["cert_pem"].as_str().unwrap();
    let key_pem = resp["key_pem"].as_str().unwrap();

    // 2. Verify the cert appears in the list endpoint
    let (status, list_body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/ca/cert?product_id={}&device_id={}",
            product_id, device_id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let list_resp: serde_json::Value = serde_json::from_str(&list_body).unwrap();
    let certs = list_resp["data"].as_array().expect("Expected data array");
    assert_eq!(certs.len(), 1, "Expected exactly one cert in list");

    let listed_cert = &certs[0];
    assert_eq!(listed_cert["pub_cert"].as_str().unwrap(), cert_pem);
    assert_eq!(listed_cert["status"].as_str().unwrap(), "Normal");

    // 3. Verify the list response does NOT include key_pem
    assert!(
        listed_cert.get("key_pem").is_none(),
        "List endpoint must not expose key_pem"
    );
    assert!(
        !listed_cert["pub_cert"].as_str().unwrap().contains(key_pem),
        "List response pub_cert must not contain the private key"
    );
}
