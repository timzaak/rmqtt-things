//! Scenario tests for device auto-provisioning.
//!
//! US-DV-010: Device first-connect auto-registration via HMAC auth
//! US-PA-037: Cert issuance creates Manual registration; device list returns and filters registration_source

use super::mqtt_test_context::MqttTestContext;
use super::simple_tests::TestContext;
use crate::api::ca_handlers::IssueCertRequest;
use crate::db::models::RegistrationSource;
use hmac::{Hmac, Mac};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde_json::json;
use serial_test::serial;
use sha1::Sha1;
use test_context::test_context;
use time::{Duration, OffsetDateTime};

type HmacSha1 = Hmac<Sha1>;

fn unique_id(prefix: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        % 1_000_000;
    format!("{prefix}{millis}")
}

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

async fn create_product_with_auto_provisioning(
    ctx: &TestContext,
    model_no: &str,
    auto_provisioning: bool,
) -> i32 {
    let create_req = json!({
        "name": format!("Product-{model_no}"),
        "model_no": model_no,
        "description": "Test product"
    });
    let (status, body) = ctx.admin_post_json("/api/admin/product", &create_req).await;
    assert_eq!(status, 201, "Product creation failed: {body}");
    let product: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = product["id"].as_i64().unwrap() as i32;

    let update_req = json!({
        "name": format!("Product-{model_no}"),
        "description": "Test product",
        "auto_provisioning": auto_provisioning
    });
    let (status, _) = ctx
        .admin_patch_json(&format!("/api/admin/product/{id}"), &update_req)
        .await;
    assert_eq!(status, 200, "Product update failed");

    id
}

async fn create_product_mqtt(
    ctx: &MqttTestContext,
    model_no: &str,
    auto_provisioning: bool,
) -> i32 {
    let create_req = json!({
        "name": format!("Product-{model_no}"),
        "model_no": model_no,
        "description": "Test product"
    });
    let (status, body) = ctx.admin_post_json("/api/admin/product", &create_req).await;
    assert_eq!(status, 201, "Product creation failed: {body}");
    let product: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = product["id"].as_i64().unwrap() as i32;

    let update_req = json!({
        "name": format!("Product-{model_no}"),
        "description": "Test product",
        "auto_provisioning": auto_provisioning
    });
    let (status, _) = ctx
        .admin_patch_json(&format!("/api/admin/product/{id}"), &update_req)
        .await;
    assert_eq!(status, 200, "Product update failed");

    id
}

async fn issue_test_cert(ctx: &TestContext, product_id: &str, device_id: &str) {
    let issue_req = IssueCertRequest {
        product_id: product_id.to_string(),
        device_id: device_id.to_string(),
        force: true,
        start_at: OffsetDateTime::now_utc(),
        end_at: OffsetDateTime::now_utc() + Duration::days(365),
    };
    let (status, body) = ctx.admin_post_json("/api/admin/ca/cert", &issue_req).await;
    assert_eq!(status, 200, "Cert issuance failed: {body}");
}

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

async fn send_auth_request_oneshot(
    ctx: &TestContext,
    product_id: &str,
    device_id: &str,
) -> (u16, String) {
    let suffix = &ctx._app_state.config.mqtt.access.auth.suffix;
    let payload = build_auth_payload(product_id, device_id, suffix);
    ctx.admin_post_json_with_headers("/api/access/auth", &payload, &[("x-real-ip", "127.0.0.1")])
        .await
}

// US-DV-010: auto_provisioning ON + unregistered -> auto-register + allow
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_on_unregistered(ctx: &mut MqttTestContext) {
    let product_id = unique_id("ap_on_unreg_");
    let device_id = unique_id("dev_ap_on_");

    let _pid = create_product_mqtt(ctx, &product_id, true).await;
    let device = ctx.connect_device(&product_id, &device_id).await;

    let record = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed")
        .expect("Device should be auto-registered");
    assert_eq!(record.registration_source, RegistrationSource::Auto);

    device.disconnect().await;
}

// US-DV-010: auto_provisioning ON + already registered -> no duplicate + allow
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_on_already_registered(ctx: &mut MqttTestContext) {
    let product_id = unique_id("ap_on_dup_");
    let device_id = unique_id("dev_dup_");

    let _pid = create_product_mqtt(ctx, &product_id, true).await;

    let device1 = ctx.connect_device(&product_id, &device_id).await;
    device1.disconnect().await;

    let first_id = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed")
        .expect("Device should exist")
        .id;

    let device2 = ctx.connect_device(&product_id, &device_id).await;

    let second_id = ctx
        ._app_state
        .db
        .device()
        .find_by_product_and_device(&product_id, &device_id)
        .await
        .expect("DB query failed")
        .expect("Device should exist")
        .id;
    assert_eq!(second_id, first_id, "No duplicate device should be created");

    device2.disconnect().await;
}

// US-DV-010: auto_provisioning OFF + unregistered -> deny
// Uses HTTP directly because connect_device panics on denial.
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_off_unregistered(ctx: &mut MqttTestContext) {
    let product_id = unique_id("ap_off_");
    let device_id = unique_id("dev_off_");

    let _pid = create_product_mqtt(ctx, &product_id, false).await;

    let (status, body) = send_auth_request(ctx, &product_id, &device_id).await;
    assert_eq!(status, 200);
    assert_eq!(body, "deny");
}

// US-DV-010: auto_provisioning OFF + already registered -> allow
#[test_context(MqttTestContext)]
#[tokio::test]
#[serial]
async fn scenario_auto_provision_off_already_registered(ctx: &mut MqttTestContext) {
    let product_id = unique_id("ap_off_reg_");
    let device_id = unique_id("dev_off_reg_");

    let _pid = create_product_mqtt(ctx, &product_id, false).await;

    ctx._app_state
        .db
        .device()
        .upsert(&product_id, &device_id, RegistrationSource::Manual)
        .await
        .expect("Failed to pre-register device");

    let device = ctx.connect_device(&product_id, &device_id).await;
    device.disconnect().await;
}

// US-PA-037: Cert issuance creates Manual registration
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_cert_creates_manual_registration(ctx: &mut TestContext) {
    let product_id = "cert_manual_product";
    let device_id = "cert_manual_device";

    issue_test_cert(ctx, product_id, device_id).await;

    let record = ctx
        ._admin_state
        .db
        .device()
        .find_by_product_and_device(product_id, device_id)
        .await
        .expect("DB query failed")
        .expect("Cert should create device record");
    assert_eq!(record.registration_source, RegistrationSource::Manual);
}

// US-PA-037: Device list returns registration_source
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_list_returns_registration_source(ctx: &mut TestContext) {
    let product_id = "reg_source_product";
    let device_auto = "reg_source_auto_dev";
    let device_manual = "reg_source_manual_dev";

    let _pid = create_product_with_auto_provisioning(ctx, product_id, true).await;

    let (status, body) = send_auth_request_oneshot(ctx, product_id, device_auto).await;
    assert_eq!(status, 200, "Auth failed: {body}");
    assert_eq!(body, "allow");

    issue_test_cert(ctx, product_id, device_manual).await;

    let (status, body) = ctx
        .admin_get(&format!(
            "/api/admin/device/status?product_id={product_id}&page=1&page_size=50"
        ))
        .await;
    assert_eq!(status, 200, "Device list failed: {body}");

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let devices = resp["data"].as_array().expect("Expected data array");

    let auto_dev = devices
        .iter()
        .find(|d| d["device_id"].as_str() == Some(device_auto))
        .expect("Auto device missing");
    assert_eq!(auto_dev["registration_source"], "Auto");

    let manual_dev = devices
        .iter()
        .find(|d| d["device_id"].as_str() == Some(device_manual))
        .expect("Manual device missing");
    assert_eq!(manual_dev["registration_source"], "Manual");
}

// US-PA-037: Device list filters by registration_source
#[test_context(TestContext)]
#[tokio::test]
async fn scenario_device_list_filter_registration_source(ctx: &mut TestContext) {
    let product_id = "filter_source_product";
    let device_auto = "filter_source_auto";
    let device_manual = "filter_source_manual";

    let _pid = create_product_with_auto_provisioning(ctx, product_id, true).await;

    let (status, body) = send_auth_request_oneshot(ctx, product_id, device_auto).await;
    assert_eq!(status, 200);
    assert_eq!(body, "allow");

    issue_test_cert(ctx, product_id, device_manual).await;

    let (status, body) = ctx
        .admin_get(&format!("/api/admin/device/status?product_id={product_id}&registration_source=Auto&page=1&page_size=50"))
        .await;
    assert_eq!(status, 200, "Filter Auto failed: {body}");
    let resp = serde_json::from_str::<serde_json::Value>(&body).unwrap();
    let auto_devices = resp["data"].as_array().unwrap();
    assert!(
        auto_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_auto))
    );
    assert!(
        !auto_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_manual))
    );

    let (status, body) = ctx
        .admin_get(&format!("/api/admin/device/status?product_id={product_id}&registration_source=Manual&page=1&page_size=50"))
        .await;
    assert_eq!(status, 200, "Filter Manual failed: {body}");
    let resp = serde_json::from_str::<serde_json::Value>(&body).unwrap();
    let manual_devices = resp["data"].as_array().unwrap();
    assert!(
        manual_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_manual))
    );
    assert!(
        !manual_devices
            .iter()
            .any(|d| d["device_id"].as_str() == Some(device_auto))
    );
}
