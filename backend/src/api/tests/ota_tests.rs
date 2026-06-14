use crate::api::admin_models::{CreateOtaVersionRequest, UpdateOtaVersionRequest};
use crate::api::tests::simple_tests::TestContext;
use crate::api::tests::simple_tests::{request, request_json};
use crate::db::models::OtaVersion;
use axum::http::{Method, StatusCode};
use serde_json::json;
use test_context::test_context;

#[test_context(TestContext)]
#[tokio::test]
async fn test_ota_version_crud_apis(ctx: &mut TestContext) {
    let product_id = "test_product";
    let ota_key = "firmware";

    // 1. Create OTA Version
    let create_req = CreateOtaVersionRequest {
        product_id: product_id.to_string(),
        key: ota_key.to_string(),
        version: "1.0.0".to_string(),
        min_version: "0.9.0".to_string(),
        max_version: Some("1.1.0".to_string()),
        file_key: "path/to/firmware.bin".to_string(),
        log: Some(json!({"release_notes": "Initial release"})),
        device_ids: None,
        bin_length: 12345,
        bin_md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/ota/version",
        &create_req,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Get OTA Versions
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/ota/version?product_id={product_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list_res: serde_json::Value = serde_json::from_str(&body).unwrap();
    let versions = list_res["data"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    let version_id = versions[0]["id"].as_i64().unwrap();

    // 3. Get OTA Version by ID
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/ota/version/{}", version_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let version: OtaVersion = serde_json::from_str(&body).unwrap();
    assert_eq!(version.product_id, product_id);

    // 4. Update OTA Version
    let update_req = UpdateOtaVersionRequest {
        min_version: Some("0.9.5".to_string()),
        max_version: Some("1.2.0".to_string()),
        file_key: None,
        log: Some(json!({"release_notes": "Updated release notes"})),
        device_ids: None,
        bin_length: Some(54321),
        bin_md5: Some("e7f8c9d0a1b2c3d4e5f6a7b8c9d0e1f2".to_string()),
    };
    let (status, _) = request_json(
        &ctx.service,
        Method::PUT,
        &format!("/api/admin/ota/version/{}", version_id),
        &update_req,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 5. Delete OTA Version
    let (status, _) = request(
        &ctx.service,
        Method::DELETE,
        &format!("/api/admin/ota/version/{}", version_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 6. Verify Deletion
    let (status, _) = request(
        &ctx.service,
        Method::GET,
        &format!("/api/admin/ota/version/{}", version_id),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Negative: create_ota_version must reject an invalid `key` (empty / overlong /
// illegal characters) with 400. `key` flows into the OTA object path and device
// lookup, so it must satisfy the same identifier rules as product_id / device_id
// (see admin_handlers.rs::create_ota_version).
#[test_context(TestContext)]
#[tokio::test]
async fn test_create_ota_version_rejects_invalid_key(ctx: &mut TestContext) {
    let base_valid = || CreateOtaVersionRequest {
        product_id: "ota_key_prod".to_string(),
        key: "firmware".to_string(),
        version: "1.0.0".to_string(),
        min_version: "0.9.0".to_string(),
        max_version: None,
        file_key: "path/to/firmware.bin".to_string(),
        log: None,
        device_ids: None,
        bin_length: 1,
        bin_md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
    };

    // Empty key
    let mut req = base_valid();
    req.key = "".to_string();
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/ota/version", &req).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "empty key must be rejected"
    );

    // Illegal character (path separator)
    let mut req = base_valid();
    req.key = "firmware/evil".to_string();
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/ota/version", &req).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "key with '/' must be rejected"
    );

    // Overlong key
    let mut req = base_valid();
    req.key = "a".repeat(129);
    let (status, _) =
        request_json(&ctx.service, Method::POST, "/api/admin/ota/version", &req).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "key longer than 128 chars must be rejected"
    );
}

#[test_context(TestContext)]
#[tokio::test]
async fn test_admin_file_upload_api(ctx: &mut TestContext) {
    let req = json!({
        "fileName": "test.txt",
        "directory": "/",
        "useOriginName": true,
        "fileType": "text/plain"
    });

    let (status, body) =
        request_json(&ctx.service, Method::POST, "/api/admin/file/upload", &req).await;
    assert_eq!(status, StatusCode::OK);
    let upload_res: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(upload_res["url"].is_string());
    assert!(upload_res["fields"].is_object());
}
