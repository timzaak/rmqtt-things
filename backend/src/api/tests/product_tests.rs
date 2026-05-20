use super::simple_tests::TestContext;
use super::simple_tests::{request, request_json};
use axum::http::{Method, StatusCode};
use serde_json::json;
use test_context::test_context;

#[test_context(TestContext)]
#[tokio::test]
async fn test_product_apis(ctx: &mut TestContext) {
    // 1. Create a new product
    let create_req = json!({
        "name": "Test Product",
        "model_no": "TP-001",
        "description": "This is a test product."
    });

    let (status, body) = request_json(
        &ctx.service,
        Method::POST,
        "/api/admin/product",
        &create_req,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    let product: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(product["name"], "Test Product");
    let product_id: i32 = product["id"].as_i64().unwrap() as i32;

    // 2. List products and verify the new product is there
    let (status, body) = request(&ctx.service, Method::GET, "/api/admin/product").await;

    assert_eq!(status, StatusCode::OK);
    let list_res: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list_res["data"][0]["name"], "Test Product");

    // 3. Update the product
    let update_req = json!({
        "name": "Updated Test Product",
        "description": "This is an updated test product.",
        "auto_provisioning": false
    });

    let (status, body) = request_json(
        &ctx.service,
        Method::PATCH,
        &format!("/api/admin/product/{}", product_id),
        &update_req,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let updated_product: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated_product["name"], "Updated Test Product");

    // 4. List products with search
    let (status, body) = request(
        &ctx.service,
        Method::GET,
        "/api/admin/product?search=Updated",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(search_res["data"][0]["name"], "Updated Test Product");
}
