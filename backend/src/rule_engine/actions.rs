use crate::db::alarm::AlarmRepo;
use crate::rule_engine::evaluator::TriggerType;
use serde_json::Value as JsonValue;
use time::{Duration, OffsetDateTime};
use tracing::{error, warn};

/// Context describing the trigger event being evaluated.
#[derive(Debug, Clone)]
pub struct TriggerContext {
    pub product_id: String,
    pub device_id: String,
    pub trigger_type: TriggerType,
    /// The actual payload:
    /// - Property: the full properties object from the webhook
    /// - Event: the raw `payload.params` JSON object
    /// - DeviceOnline/DeviceOffline: json!({})
    pub trigger_value: JsonValue,
}

/// Action types supported by the rule engine.
#[derive(Debug, Clone)]
pub enum AlarmAction {
    Alarm { level: i16, message: String },
    Webhook { url: String },
}

/// Parse actions from a JSON array.
pub fn parse_actions(actions_json: &[JsonValue]) -> Vec<AlarmAction> {
    let mut actions = Vec::new();
    for action_val in actions_json {
        if let Some(obj) = action_val.as_object() {
            match obj.get("type").and_then(|v| v.as_str()) {
                Some("alarm") => {
                    let level = match obj.get("level") {
                        Some(v) if v.is_string() => match v.as_str().unwrap() {
                            "info" => 0,
                            "warning" => 1,
                            "critical" => 2,
                            _ => {
                                warn!("Unknown alarm level: {:?}", v);
                                0
                            }
                        },
                        Some(v) if v.is_number() => v.as_i64().unwrap_or(0) as i16,
                        _ => 0,
                    };
                    let message = obj
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    actions.push(AlarmAction::Alarm { level, message });
                }
                Some("webhook") => {
                    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                        actions.push(AlarmAction::Webhook {
                            url: url.to_string(),
                        });
                    } else {
                        warn!("Webhook action missing url field: {:?}", action_val);
                    }
                }
                Some(other) => {
                    warn!("Unknown action type: {}", other);
                }
                None => {
                    warn!("Action missing type field: {:?}", action_val);
                }
            }
        }
    }
    actions
}

/// Send a webhook POST with a JSON payload and configurable timeout.
///
/// Shared by both the initial action execution and the background retry task.
pub(crate) async fn send_webhook(
    url: &str,
    payload: &serde_json::Value,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(payload)
        .timeout(timeout)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        anyhow::bail!("Webhook returned status {}", status)
    }
}

pub struct ActionExecutor;

impl ActionExecutor {
    /// Execute the configured actions for a matched rule.
    ///
    /// 1. Creates the alarm record first (always).
    /// 2. Then executes any webhook actions.
    /// 3. Records webhook status on the alarm record.
    pub async fn execute_actions(
        actions: &[JsonValue],
        ctx: &TriggerContext,
        alarm_repo: &AlarmRepo,
        rule_id: i64,
        rule_name: &str,
    ) -> anyhow::Result<()> {
        let parsed = parse_actions(actions);

        // Find the alarm action (first one, if any)
        let alarm_action = parsed.iter().find_map(|a| match a {
            AlarmAction::Alarm { level, message } => Some((*level, message.clone())),
            _ => None,
        });

        let (level, message) = alarm_action.unwrap_or((0, String::new()));

        // Create alarm record
        let alarm_id = alarm_repo
            .insert_alarm(
                rule_id,
                rule_name,
                &ctx.product_id,
                &ctx.device_id,
                level,
                if message.is_empty() {
                    None
                } else {
                    Some(&message)
                },
                Some(&ctx.trigger_value),
                ctx.trigger_type.as_str(),
            )
            .await?;

        // Execute webhook actions and collect results
        let webhook_actions: Vec<&AlarmAction> = parsed
            .iter()
            .filter(|a| matches!(a, AlarmAction::Webhook { .. }))
            .collect();

        if !webhook_actions.is_empty() {
            let payload = serde_json::json!({
                "rule_name": rule_name,
                "product_id": ctx.product_id,
                "device_id": ctx.device_id,
                "trigger_type": ctx.trigger_type.as_str(),
                "trigger_value": ctx.trigger_value,
            });

            let mut any_webhook_failed = false;
            for action in &webhook_actions {
                if let AlarmAction::Webhook { url } = action {
                    match send_webhook(url, &payload, std::time::Duration::from_secs(5)).await {
                        Ok(()) => {}
                        Err(e) => {
                            error!("Webhook action failed for rule {}: {}", rule_id, e);
                            any_webhook_failed = true;
                        }
                    }
                }
            }

            // Single status update after all webhooks attempted
            if any_webhook_failed {
                if let Err(e) = alarm_repo
                    .update_alarm_webhook_status_with_retry(
                        alarm_id,
                        1,
                        alarm_repo.webhook_max_retries,
                        Some(
                            OffsetDateTime::now_utc()
                                + Duration::seconds(
                                    alarm_repo.webhook_retry_interval_seconds as i64,
                                ),
                        ),
                    )
                    .await
                {
                    error!(
                        "Failed to update webhook retry state for alarm {}: {}",
                        alarm_id, e
                    );
                }
            } else if let Err(e) = alarm_repo.update_alarm_webhook_status(alarm_id, 0).await {
                error!(
                    "Failed to update webhook status for alarm {}: {}",
                    alarm_id, e
                );
            }
        }

        Ok(())
    }
}
