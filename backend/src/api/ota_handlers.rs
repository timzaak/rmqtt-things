use crate::api::ApiState;
use crate::api::ack_response;
use crate::api::error::ApiError;
use crate::api::utils::{extract_and_validate_product_id, validate_identifier};
use crate::api::web_models::{AckStatus, OtaReport, RMqttPublishMessage};
use crate::rmqtt_client::PublishRequest;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::json;
use std::sync::Arc;
use tracing::error;
use utoipa::ToSchema;

#[derive(serde::Deserialize, ToSchema)]
struct OtaReportParams {
    params: Vec<OtaReport>,
    id: String,
    ack: AckStatus,
}

#[utoipa::path(
    post,
    path = "/api/ota/version",
    tag = "thing",
    request_body = RMqttPublishMessage,
    responses(
        (status = 204, description = "OTA report accepted"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
pub async fn ota_version_post(
    State(state): State<Arc<ApiState>>,
    Json(mqtt_msg): Json<RMqttPublishMessage>,
) -> Result<StatusCode, ApiError> {
    let state = &state.app;
    let product_id = extract_and_validate_product_id(&mqtt_msg.topic)?;
    let device_id = &mqtt_msg.client_id;
    validate_identifier(device_id, "device_id")?;
    let bytes = mqtt_msg.decode_payload().map_err(|e| {
        error!("Failed to decode payload: {}", e);
        ApiError::bad_request("Invalid payload format")
    })?;
    let ota_report_params: OtaReportParams = serde_json::from_slice(&bytes).map_err(|e| {
        error!("Failed to parse OtaReportParams: {}", e);
        ApiError::bad_request("Invalid params for OtaReportParams")
    })?;

    let mut updates = Vec::new();
    for report in ota_report_params.params {
        state
            .db
            .ota()
            .upsert_device_version(&product_id, device_id, &report.key, report.version)
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ApiError::internal("Database operation failed")
            })?;

        if let Some(ota_version) = state
            .db
            .ota()
            .get_ota_update(&product_id, device_id, &report.key, report.version)
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ApiError::internal("Database operation failed")
            })?
        {
            updates.push(ota_version);
        }
    }

    if ota_report_params.ack == AckStatus::Yes {
        let _ = ack_response(
            ota_report_params.id.clone(),
            &state.rmqtt_client,
            &mqtt_msg.topic,
        )
        .await;
    }

    if !updates.is_empty() {
        let s3_client = state.s3_client.as_ref().ok_or_else(|| {
            error!("S3 client not configured");
            ApiError::internal("S3 client not configured")
        })?;

        let mut params = Vec::new();
        for ota_version in updates {
            let file_url = s3_client
                .get_presigned_download_url(&ota_version.file_key)
                .await
                .map_err(|e| {
                    error!("Failed to get presigned download url: {}", e);
                    ApiError::internal("Failed to get presigned download url")
                })?;
            params.push(json!({
            "key": ota_version.key,
            "file_url": file_url,
                "version": ota_version.version,
                "log": ota_version.log,
            }));
        }

        let upgrade_payload = json!({
            "id": ota_report_params.id,
            "params": params,
        });

        let topic = format!("/{}/{}/ota/upgrade", product_id, device_id);
        let publish_request = PublishRequest {
            topic,
            clientid: state.config.mqtt.publish.response.clientid.clone(),
            payload: upgrade_payload.to_string(),
            encoding: None,
            qos: Some(1),
            retain: Some(false),
        };

        state
            .rmqtt_client
            .publish_command(publish_request)
            .await
            .map_err(|e| {
                error!("Failed to publish OTA upgrade message: {}", e);
                ApiError::internal("Failed to publish OTA upgrade message")
            })?;
    }

    Ok(StatusCode::NO_CONTENT)
}
