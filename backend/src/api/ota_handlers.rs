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

/// Parse a spec `"major.minor.patch"` semver string into the internal packed
/// `i32` storage (design §5.3). Encodes as `major*100_000 + minor*1_000 +
/// patch`, matching `OtaRepo::parse_version_to_int` so device-side reports and
/// admin-side `ota_versions.version`/`min_version`/`max_version` share the same
/// collation space. Bounds match `validate_version_format` (major/minor <= 99,
/// patch <= 999). Returns 400 on any deviation.
fn parse_semver_to_int(version: &str) -> Result<i32, ApiError> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        error!(
            "Invalid OTA version format '{}': must be major.minor.patch",
            version
        );
        return Err(ApiError::bad_request(
            "Invalid version format: must be major.minor.patch (e.g. 1.2.3)",
        ));
    }
    let mut parsed = [0u32; 3];
    for (i, part) in parts.iter().enumerate() {
        match part.parse::<u32>() {
            Ok(v) => parsed[i] = v,
            Err(_) => {
                error!(
                    "Invalid OTA version format '{}': segment '{}' is not numeric",
                    version, part
                );
                return Err(ApiError::bad_request(
                    "Invalid version format: each segment must be numeric",
                ));
            }
        }
    }
    let max = [99u32, 99, 999];
    for (i, &v) in parsed.iter().enumerate() {
        if v > max[i] {
            error!(
                "Invalid OTA version format '{}': segment {} value {} exceeds {}",
                version, i, v, max[i]
            );
            return Err(ApiError::bad_request(
                "Invalid version format: a segment exceeds its allowed range",
            ));
        }
    }
    Ok((parsed[0] as i32) * 100_000 + (parsed[1] as i32) * 1_000 + (parsed[2] as i32))
}

/// Inverse of `parse_semver_to_int`: decode the packed `i32` storage back to
/// the spec `"major.minor.patch"` string (design §5.3). The OTA upgrade push
/// publishes versions that were originally created as semver strings on the
/// admin side, so the wire form must round-trip as a string, not the raw int
/// encoding. Same `major*100_000 + minor*1_000 + patch` encoding/bounds.
fn format_semver_from_int(encoded: i32) -> String {
    let major = encoded / 100_000;
    let minor = (encoded % 100_000) / 1_000;
    let patch = encoded % 1_000;
    format!("{major}.{minor}.{patch}")
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
        // Spec contract: `version` arrives as a `"major.minor.patch"` string
        // (design §5.3). Parse to the internal packed int that the DB layer
        // shares with admin-side `ota_versions` rows. Validation failure → 400.
        let version_int = parse_semver_to_int(&report.version)?;

        state
            .db
            .ota()
            .upsert_device_version(&product_id, device_id, &report.key, version_int)
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ApiError::internal("Database operation failed")
            })?;

        if let Some(ota_version) = state
            .db
            .ota()
            .get_ota_update(&product_id, device_id, &report.key, version_int)
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
        // Spec contract (design §5.3): the upgrade payload carries the S3
        // **object key** (`ota_versions.file_key`), NOT a presigned download
        // URL — the device is responsible for fetching the binary through its
        // own authorised channel (e.g. a subsequent file/download request).
        // `ack: 0` because the spec defines no OTA upgrade reply topic, so the
        // device must not ack this push.
        let mut params = Vec::new();
        for ota_version in updates {
            // Spec contract (design §5.3): `version` on the wire is the
            // `"major.minor.patch"` semver string, matching the admin-side
            // creation form. `ota_version.version` is the packed i32 storage,
            // so decode it back before serialising (do NOT emit the raw int).
            params.push(json!({
                "key": ota_version.key,
                "file_url": ota_version.file_key,
                "version": format_semver_from_int(ota_version.version),
                "log": ota_version.log,
            }));
        }

        let upgrade_payload = json!({
            "id": ota_report_params.id,
            "ack": 0,
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
