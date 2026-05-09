use crate::api::handlers::S3Client;
use crate::config::S3Config;
use reqwest::StatusCode;
use test_context::{AsyncTestContext, test_context};

struct S3TestContext {
    config: S3Config,
}

impl AsyncTestContext for S3TestContext {
    async fn setup() -> S3TestContext {
        let endpoint = super::simple_tests::test_s3_endpoint();
        let config = S3Config {
            endpoint,
            region: "us-east-1".to_string(),
            access_key: "fake_access".to_string(),
            secret_key: "fake_secret".to_string(),
            bucket: "fake_bucket".to_string(),
            directories: vec!["/*".to_string()],
            expired_seconds: 60,
        };

        S3TestContext { config }
    }

    async fn teardown(self) {}
}

#[test_context(S3TestContext)]
#[tokio::test]
async fn test_s3(ctx: &mut S3TestContext) {
    let client = S3Client::new(&ctx.config).unwrap();
    let presigned_post = client.get_presigned_post("/abc/abc.txt").await.unwrap();

    let mut form = reqwest::multipart::Form::new();
    for (key, value) in presigned_post.fields {
        form = form.text(key, value);
    }

    for (key, value) in presigned_post.dynamic_fields {
        form = form.text(key, value);
    }
    let file_part = reqwest::multipart::Part::text("hello abc")
        .file_name("abc.txt")
        .mime_str("plain/txt")
        .unwrap();
    form = form.part("file".to_string(), file_part);

    let client = reqwest::Client::new();

    let response = client
        .post(&ctx.config.endpoint)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(StatusCode::OK, response.status());
}
