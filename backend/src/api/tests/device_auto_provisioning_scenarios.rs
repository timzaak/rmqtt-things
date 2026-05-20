//! Scenario tests for device auto-provisioning feature.
//!
//! Covers:
//! - US-DV-010: Device first-connect auto-registration via HMAC auth
//! - US-PA-037: Cert issuance creates Manual registration; device list returns and filters registration_source

use super::mqtt_test_context::MqttTestContext;
use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use crate::api::ca_handlers::IssueCertRequest;
use crate::db::models::RegistrationSource;
use axum::http::{Method, StatusCode};
use hmac::{Hmac, Mac};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde_json::json;
use serial_test::serial;
use sha1::Sha1;
use test_context::test_context;
use time::{Duration, OffsetDateTime};

type HmacSha1 = Hmac<Sha1>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate an HMAC-signed password matching the auth handler's expectation.
fn generate_hmac_password(device_id: &str, suffix: &str) -> String {
    let nonce: String = rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    let timestamp = OffsetDateTime::now_utc().unix_timestamp();
    let to_sign = format!("{device_id}.{nonce}.{timestamp}.{suffix}");
    let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
    mac.update(to_sign.as_bytes());
    let result = mac.finalize();
    let hash = hex::encode(result.into_bytes());
    format!("{nonce}.{timestamp}.{hash}")
}

/// Build the JSON body for an auth request (AuthPayload fields).
/// Takes the HMAC suffix as parameter so it works with both MqttTestContext and TestContext configs.
fn build_auth_payload(product_id: &str, device_id: &str, suffix: &str) -> serde_json::Value {
    let password = generate_hmac_password(device_id, suffix);
    json!({
        "client_id": device_id,
        "username": product_id,
        "password": password,
        "protocol": 4,
        "ipaddress": "127.0.0.1"
    })
}

/// Create a product via admin API with the given `auto_provisioning` setting.
/// Returns the product id (i32 database id).
async fn create_product_with_auto_provisioning(
    ctx: &TestContext,
    model_no: &str,
    auto_provisioning: bool,
) -> i32 {
    let create_req = json!({
        "name": format!("Product-{model_no}"),
        "model_no": model_no,
        "description": "Test product for auto-provisioning"
    });
    let (status, body) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/product",
        &create_req,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Product creation failed: {body}"
    );
    let product: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = product["id"].as_i64().unwrap() as i32;

    // Update to set auto_provisioning
    let update_req = json!({
        "name": format!("Product-{model_no}"),
        "description": "Test product for auto-provisioning",
        "auto_provisioning": auto_provisioning
    });
    let (status, _) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/product/{id}"),
        &update_req,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Product update failed");

    id
}

/// Issue a certificate for the given product/device via admin API.
async fn issue_test_cert(ctx: &TestContext, product_id: &str, device_id: &str) {
    let issue_req = IssueCertRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };
    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/ca/cert", &issue_req).await;
    assert_eq!(status, StatusCode::OK, "Cert issuance failed: {body}");
}

/// Send an auth request directly to the live server (MqttTestContext).
/// This bypasses MQTT and goes straight to the auth webhook handler.
/// Adds X-Real-IP header to simulate RMQTT webhook callback (internal IP required by middleware).
async fn send_auth_request(
    ctx: &MqttTestContext,
    product_id: &str,
    device_id: &str,
) -> (u16, String) {
    let suffix = &ctx._app_state.config.mqtt.access.auth.suffix;
    let payload = build_auth_payload(product_id, device_id, suffix);
    ctx.admin_post_json_with_headers("/api/access/auth", &payload, &[("x-real-ip", "127.0.0.1")])
        .await
}

/// Send an auth request using the oneshot TestContext router.
/// Adds X-Real-IP header to simulate RMQTT webhook callback (internal IP required by middleware).
async fn send_auth_request_oneshot(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
) -> (StatusCode, String) {
    use super::simple_tests::request_json_with_headers;
    let suffix = &ctx._app_state.config.mqtt.access.auth.suffix;
    let payload = build_auth_payload(product_id, device_id, suffix);
    request_json_with_headers(
        &ctx.service,
        Method::POST,
        "/api/access/auth",
        &payload,
        &[("x-real-ip", "127.0.0.1")],
    )
    .await
}

/// Create a product in MqttTestContext using the live server.
async fn create_product_mqtt(
    ctx: &MqttTestContext,
    model_no: &str,
    auto_provisioning: bool,
) -> i32 {
    let create_req = json!({
        "name": format!("Product-{model_no}"),
        "model_no": model_no,
        "description": "Test product for auto-provisioning"
    });
    let (status, body) = ctx.admin_post_json("/api/admin/product", &create_req).await;
    assert_eq!(status, 201, "Product creation failed: {body}");
    let product: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = product["id"].as_i64().unwrap() as i32;

    let update_req = json!({
        "name": format!("Product-{model_no}"),
        "description": "Test product for auto-provisioning",
        "auto_provisioning": auto_provisioning
    });
    let (status, _) = ctx
        .admin_patch_json(&format!("/api/admin/product/{id}"), &update_req)
        .await;
    assert_eq!(status, 200, "Product update failed");

    id
}

// ---------------------------------------------------------------------------
// Scenario 1: auto_provisioning ON + unregistered -> auto-register + allow
// ---------------------------------------------------------------------------

/// User Story: US-DV-010 - Device first-connect auto-registration
/// Covers: Product auto_provisioning=ON, device not yet registered.
///   Device connects via HMAC auth -> auth handler creates device record -> returns "allow".
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_on_unregistered(ctx: &mut MqttTestContext) {
    let product_id = format!(
        "ap_on_unreg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let device_id = format!(
        "dev_ap_on_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    // Create product with auto_provisioning ON
    let _pid = create_product_mqtt(ctx, &product_id, true).await;

    // Device should connect successfully (auto-provisioned)
    let device = ctx.connect_device(&product_id, &device_id).await;

    // Verify device was registered in the devices table via direct DB query
    let device_record = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed");
    assert!(
        device_record.is_some(),
        "Device should be auto-registered in devices table"
    );
    let device_record = device_record.unwrap();
    assert_eq!(
        device_record.registration_source,
        RegistrationSource::Auto,
        "Device should be registered with Auto source"
    );

    device.disconnect().await;
}

// ---------------------------------------------------------------------------
// Scenario 2: auto_provisioning ON + already registered -> no duplicate + allow
// ---------------------------------------------------------------------------

/// User Story: US-DV-010 - Device first-connect auto-registration (idempotency)
/// Covers: Product auto_provisioning=ON, device already registered.
///   Second connection of same device -> no duplicate record created -> returns "allow".
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_on_already_registered(ctx: &mut MqttTestContext) {
    let product_id = format!(
        "ap_on_dup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let device_id = format!(
        "dev_dup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    // Create product with auto_provisioning ON
    let _pid = create_product_mqtt(ctx, &product_id, true).await;

    // First connection: auto-register
    let device1 = ctx.connect_device(&product_id, &device_id).await;
    device1.disconnect().await;

    // Verify device was registered
    let record_after_first = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed")
        .expect("Device should exist after first connect");
    let first_id = record_after_first.id;

    // Second connection: should be allowed without duplicate
    let device2 = ctx.connect_device(&product_id, &device_id).await;

    // Verify no duplicate was created
    let record_after_second = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed")
        .expect("Device should still exist");
    assert_eq!(
        record_after_second.id, first_id,
        "No duplicate device record should be created on second connect"
    );

    device2.disconnect().await;
}

// ---------------------------------------------------------------------------
// Scenario 3: auto_provisioning OFF + unregistered -> deny
// ---------------------------------------------------------------------------

/// User Story: US-DV-010 - Device first-connect auto-registration (denied)
/// Covers: Product auto_provisioning=OFF, device not registered.
///   Device attempts HMAC auth -> auth handler denies access -> returns "deny".
///
/// NOTE: This test CANNOT use `MqttTestContext::connect_device` because that helper
/// panics on connection failure. Instead we send an HTTP request directly to the
/// auth endpoint and assert the response is "deny".
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_off_unregistered(ctx: &mut MqttTestContext) {
    let product_id = format!(
        "ap_off_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let device_id = format!(
        "dev_off_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    // Create product with auto_provisioning OFF
    let _pid = create_product_mqtt(ctx, &product_id, false).await;

    // Send auth request directly -- should be denied
    let (status, body) = send_auth_request(ctx, &product_id, &device_id).await;
    assert_eq!(status, 200, "Auth endpoint should return 200");
    assert_eq!(
        body, "deny",
        "Unregistered device should be denied when auto_provisioning OFF"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: auto_provisioning OFF + already registered -> allow
// ---------------------------------------------------------------------------

/// User Story: US-DV-010 - Device first-connect auto-registration (pre-registered)
/// Covers: Product auto_provisioning=OFF, device already registered (e.g. via cert).
///   Device connects via HMAC auth -> auth handler finds existing record -> returns "allow".
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_off_already_registered(ctx: &mut MqttTestContext) {
    let product_id = format!(
        "ap_off_reg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );
    let device_id = format!(
        "dev_off_reg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1_000_000
    );

    // Create product with auto_provisioning OFF
    let _pid = create_product_mqtt(ctx, &product_id, false).await;

    // Pre-register the device manually via DB
    ctx._app_state
        .db
        .device()
        .upsert_manual(&product_id, &device_id)
        .await
        .expect("Failed to pre-register device");

    // Device should connect successfully even though auto_provisioning is OFF
    let device = ctx.connect_device(&product_id, &device_id).await;
    device.disconnect().await;
}

// ---------------------------------------------------------------------------
// Scenario 5: Cert issuance creates Manual registration record
// ---------------------------------------------------------------------------

/// User Story: US-PA-037 - View device registration source
/// Covers: Issuing a certificate via admin API should create a device record
///   with registration_source = Manual.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_cert_creates_manual_registration(ctx: &mut TestContext) {
    let product_id = "cert_manual_product";
    let device_id = "cert_manual_device";

    // Issue a certificate
    issue_test_cert(ctx, product_id, device_id).await;

    // Verify device was registered with Manual source
    let device_record = ctx
        ._admin_state
        .db
        .device()
        .find_by_product_and_device(product_id, device_id)
        .await
        .expect("DB query failed");
    assert!(
        device_record.is_some(),
        "Cert issuance should create a device record"
    );
    let device_record = device_record.unwrap();
    assert_eq!(
        device_record.registration_source,
        RegistrationSource::Manual,
        "Device registered via cert should have Manual source"
    );
}

// ---------------------------------------------------------------------------
// Scenario 6: registration_source field returned in device list
// ---------------------------------------------------------------------------

/// User Story: US-PA-037 - View device registration source
/// Covers: Device list API should return registration_source field for devices
///   that have been registered.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_list_returns_registration_source(ctx: &mut TestContext) {
    let product_id = "reg_source_product";
    let device_auto = "reg_source_auto_dev";
    let device_manual = "reg_source_manual_dev";

    // Create product with auto_provisioning ON
    let _pid = create_product_with_auto_provisioning(ctx, product_id, true).await;

    // Auto-register device via auth endpoint
    let (status, body) = send_auth_request_oneshot(ctx, product_id, device_auto).await;
    assert_eq!(status, StatusCode::OK, "Auth should succeed: {body}");
    assert_eq!(body, "allow");

    // Manual-register device via cert issuance
    issue_test_cert(ctx, product_id, device_manual).await;

    // Query device list
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/device/status?product_id={product_id}&page=1&page_size=50"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Device list query failed: {body}");

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let devices = resp["data"].as_array().expect("Expected data array");

    // Find the auto-registered device
    let auto_dev = devices
        .iter()
        .find(|d| d["device_id"].as_str() == Some(device_auto))
        .expect("Auto-registered device should be in list");
    // RegistrationSource::Auto serializes as 0
    assert_eq!(
        auto_dev["registration_source"], 0,
        "Auto device should have registration_source = 0 (Auto)"
    );

    // Find the manually registered device
    let manual_dev = devices
        .iter()
        .find(|d| d["device_id"].as_str() == Some(device_manual))
        .expect("Manual device should be in list");
    // RegistrationSource::Manual serializes as 1
    assert_eq!(
        manual_dev["registration_source"], 1,
        "Manual device should have registration_source = 1 (Manual)"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7: registration_source filter
// ---------------------------------------------------------------------------

/// User Story: US-PA-037 - View device registration source
/// Covers: Device list API should support filtering by registration_source.
///   When filtering for Auto, only auto-registered devices are returned.
///   When filtering for Manual, only manually-registered devices are returned.
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_list_filter_registration_source(ctx: &mut TestContext) {
    let product_id = "filter_source_product";
    let device_auto = "filter_source_auto";
    let device_manual = "filter_source_manual";

    // Create product with auto_provisioning ON
    let _pid = create_product_with_auto_provisioning(ctx, product_id, true).await;

    // Auto-register one device
    let (status, body) = send_auth_request_oneshot(ctx, product_id, device_auto).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "allow");

    // Manual-register another device via cert
    issue_test_cert(ctx, product_id, device_manual).await;

    // Filter for Auto
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/device/status?product_id={product_id}&registration_source=Auto&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Filter Auto failed: {body}");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let auto_devices = resp["data"].as_array().expect("Expected data array");
    assert!(
        auto_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_auto)),
        "Auto device should appear when filtering registration_source=Auto"
    );
    assert!(
        !auto_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_manual)),
        "Manual device should NOT appear when filtering registration_source=Auto"
    );

    // Filter for Manual
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!(
            "/api/admin/device/status?product_id={product_id}&registration_source=Manual&page=1&page_size=50"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Filter Manual failed: {body}");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let manual_devices = resp["data"].as_array().expect("Expected data array");
    assert!(
        manual_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_manual)),
        "Manual device should appear when filtering registration_source=Manual"
    );
    assert!(
        !manual_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_auto)),
        "Auto device should NOT appear when filtering registration_source=Manual"
    );
}
