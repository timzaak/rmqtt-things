use crate::db::alarm::AlarmRepo;
use crate::rule_engine::evaluator::TriggerType;
use serde_json::Value as JsonValue;
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
            )
            .await?;

        // Execute webhook actions
        for action in &parsed {
            if let AlarmAction::Webhook { url } = action {
                let webhook_result = Self::execute_webhook(url, ctx, rule_name).await;
                let status: i16 = match webhook_result {
                    Ok(()) => 0, // success
                    Err(e) => {
                        error!("Webhook action failed for rule {}: {}", rule_id, e);
                        1 // failed
                    }
                };
                if let Err(e) = alarm_repo
                    .update_alarm_webhook_status(alarm_id, status)
                    .await
                {
                    error!(
                        "Failed to update webhook status for alarm {}: {}",
                        alarm_id, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Execute a webhook action with a 5-second timeout.
    async fn execute_webhook(
        url: &str,
        ctx: &TriggerContext,
        rule_name: &str,
    ) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "rule_name": rule_name,
            "product_id": ctx.product_id,
            "device_id": ctx.device_id,
            "trigger_type": ctx.trigger_type.as_str(),
            "trigger_value": ctx.trigger_value,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            anyhow::bail!("Webhook returned status {}", status)
        }
    }
}
