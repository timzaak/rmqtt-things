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
