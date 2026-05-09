use crate::config::MqttConfig;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Serialize)]
pub struct PublishRequest {
    pub topic: String,
    pub clientid: String,
    pub payload: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct RmqttHttpClient {
    client: Client,
    pub config: MqttConfig,
}

impl RmqttHttpClient {
    pub fn new(config: MqttConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn publish_response(&self, original_topic: &str, payload: &str) -> Result<()> {
        let reply_topic = format!("{original_topic}_reply");

        let request = PublishRequest {
            topic: reply_topic.clone(),
            clientid: self.config.publish.response.clientid.clone(),
            payload: payload.to_string(),
            encoding: Some("plain".to_string()),
            qos: Some(self.config.publish.response.qos),
            retain: Some(self.config.publish.response.retain),
        };

        info!("Publishing response to topic: {}", reply_topic);

        let response = self
            .client
            .post(format!("{}/mqtt/publish", &self.config.url))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Successfully published response to {}", reply_topic);
        } else {
            warn!(
                "Failed to publish response: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    pub async fn publish_command(&self, request: PublishRequest) -> Result<()> {
        let retries = self.config.property_command.publish.retries;
        for attempt in 0..=retries {
            info!(
                "Publishing command to RMQTT: {}, attempt: {}",
                request.topic,
                attempt + 1
            );
            let response = self
                .client
                .post(format!("{}/mqtt/publish", &self.config.url))
                .json(&request)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    info!("Successfully published command to {}", request.topic);
                    return Ok(());
                }
                Ok(resp) => {
                    warn!(
                        "Failed to publish command: {} - {}, attempt: {}",
                        resp.status(),
                        resp.text().await.unwrap_or_default(),
                        attempt + 1
                    );
                }
                Err(e) => {
                    warn!(
                        "Error publishing command: {:?}, attempt: {}",
                        e,
                        attempt + 1
                    );
                }
            }
            if attempt < retries {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
        Err(anyhow::anyhow!(
            "Failed to publish command after {} retries",
            retries
        ))
    }

    #[allow(dead_code)]
    pub async fn check_client_online(&self, client_id: &str) -> Result<bool> {
        // 构建 RMQTT HTTP API URL，从 publish URL 中提取基础 URL
        let check_url = format!("{}/clients/{}/online", self.config.url, client_id);

        info!(
            "Checking if client {} is online via: {}",
            client_id, check_url
        );

        let response = self.client.get(&check_url).send().await?;

        if response.status().is_success() {
            let online: bool = response.json().await?;
            info!("Client {} online status: {}", client_id, online);
            Ok(online)
        } else {
            warn!(
                "Failed to check client online status: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
            // 如果检查失败，默认认为设备离线
            Ok(false)
        }
    }

    pub async fn get_subscriptions(&self, client_id: &str) -> Result<Vec<Subscription>> {
        let url = format!("{}/subscriptions?clientid={}", self.config.url, client_id);
        info!("Getting subscriptions for client {}: {}", client_id, url);

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let subscriptions: Vec<Subscription> = response.json().await?;
            info!(
                "Client {} has {} subscriptions",
                client_id,
                subscriptions.len()
            );
            Ok(subscriptions)
        } else {
            warn!(
                "Failed to get subscriptions for client {}: {} - {}",
                client_id,
                response.status(),
                response.text().await.unwrap_or_default()
            );
            Ok(Vec::new())
        }
    }

    pub async fn is_subscribed_to_properties(
        &self,
        product_id: &str,
        client_id: &str,
    ) -> Result<bool> {
        let subscriptions = self.get_subscriptions(client_id).await?;
        let topic_to_check = format!("{product_id}/{client_id}/thing/service/property/set");
        let is_subscribed = subscriptions
            .iter()
            .any(|sub| mqtt_topic_matches(&sub.topic, &topic_to_check));
        info!(
            "Client {} is subscribed to {}: {}",
            client_id, topic_to_check, is_subscribed
        );
        Ok(is_subscribed)
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Subscription {
    #[serde(alias = "topic_filter")]
    pub topic: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default)]
    pub opts: Option<SubscriptionOpts>,
    #[serde(default)]
    pub share: Option<String>,
    #[serde(default)]
    pub clientid: String,
    #[serde(default)]
    pub node_id: u64,
    #[serde(default)]
    pub client_addr: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SubscriptionOpts {
    pub qos: u8,
}

/// Check if an MQTT subscription filter (may contain `+` and `#` wildcards) matches a concrete topic.
fn mqtt_topic_matches(filter: &str, topic: &str) -> bool {
    if filter == topic {
        return true;
    }
    let mut filter_parts = filter.split('/');
    let mut topic_parts = topic.split('/');
    loop {
        match (filter_parts.next(), topic_parts.next()) {
            (Some("#"), _) => return true,
            (None, None) => return true,
            (Some(f), Some(t)) if f == "+" || f == t => {}
            _ => return false,
        }
    }
}
