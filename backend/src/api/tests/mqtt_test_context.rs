use super::simple_tests::{create_test_database, drop_test_schema, test_s3_endpoint};
use crate::api::handlers::{AppState, S3Client};
use crate::api::{AdminAppState, create_router};
use crate::cache::{InMemorySchemaCache, SchemaCache};
use crate::config::{
    AccessConfig, AuthConfig, Config, MqttConfig, MqttPublishConfig, MqttResponseConfig,
    PropertyCommandConfig, PropertyCommandPublishConfig, S3Config,
};
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;
use hmac::{Hmac, Mac};
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use reqwest::Client;
use serde::Serialize;
use sha1::Sha1;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tempfile::{TempDir, tempdir};
use test_context::AsyncTestContext;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;

const AUTH_SUFFIX: &str = "suffix_go";

pub struct MqttTestContext {
    pub backend_port: u16,
    pub rmqtt_mqtt_port: u16,
    pub _rmqtt_http_port: u16,
    pub _admin_pool: PgPool,
    pub schema_name: String,
    pub _app_state: Arc<AppState>,
    pub _admin_state: Arc<AdminAppState>,
    pub shutdown_tx: tokio::sync::watch::Sender<bool>,
    pub http_client: Client,
    pub _temp_dir: TempDir,
}

impl AsyncTestContext for MqttTestContext {
    async fn setup() -> MqttTestContext {
        let _ = tracing_subscriber::fmt().try_init();

        let rmqtt_mqtt_port: u16 = std::env::var("TEST_RMQTT_MQTT_PORT")
            .unwrap_or_else(|_| "11883".to_string())
            .parse()
            .expect("Invalid TEST_RMQTT_MQTT_PORT");
        let rmqtt_http_port: u16 = std::env::var("TEST_RMQTT_HTTP_PORT")
            .unwrap_or_else(|_| "16060".to_string())
            .parse()
            .expect("Invalid TEST_RMQTT_HTTP_PORT");
        let backend_port: u16 = std::env::var("TEST_BACKEND_PORT")
            .unwrap_or_else(|_| "18080".to_string())
            .parse()
            .expect("Invalid TEST_BACKEND_PORT");

        // Database
        let (admin_pool, schema_name, pool) = create_test_database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db_service = DatabaseService::new(pool);

        // Config
        let s3_config = S3Config {
            endpoint: test_s3_endpoint(),
            region: "us-east-1".to_string(),
            access_key: "fake_access".to_string(),
            secret_key: "fake_secret".to_string(),
            bucket: "fake_bucket".to_string(),
            directories: vec!["/*".to_string()],
            expired_seconds: 60,
        };
        let temp_dir = tempdir().unwrap();
        let config = Config {
            s3: Some(s3_config),
            mqtt: MqttConfig {
                url: format!("http://127.0.0.1:{rmqtt_http_port}/api/v1"),
                publish: MqttPublishConfig {
                    response: MqttResponseConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                    },
                },
                property_command: PropertyCommandConfig {
                    publish: PropertyCommandPublishConfig {
                        qos: 2,
                        retain: false,
                        clientid: "rmqtt_things".to_string(),
                        topic: "${productId}/$clientid/thing/service/property/set".to_string(),
                        retries: 2,
                    },
                },
                access: AccessConfig {
                    auth: AuthConfig {
                        suffix: AUTH_SUFFIX.to_string(),
                    },
                },
            },
            ..{
                let mut c = Config::default();
                c.ca.ca_dir = temp_dir.path().to_str().unwrap().to_string();
                c
            }
        };
        let config = Arc::new(config);
        crate::ca::init_ca(&config.ca).await.unwrap();

        let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
        let schema_cache = SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new()));
        let s3_client = config.s3.as_ref().map(|s3| S3Client::new(s3).unwrap());

        let app_state = Arc::new(AppState {
            db: db_service.clone(),
            rmqtt_client: rmqtt_client.clone(),
            config: config.clone(),
            cache: schema_cache.clone(),
            s3_client: s3_client.clone(),
        });
        let admin_state = Arc::new(AdminAppState {
            db: db_service,
            rmqtt_client,
            config: config.clone(),
            cache: schema_cache,
            s3_client,
            rule_cache: crate::rule_engine::RuleCache::new(),
        });

        // Start real axum server
        let router = create_router(config, app_state.clone(), admin_state.clone(), None);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{backend_port}"))
            .await
            .expect("Failed to bind backend port");

        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
                .unwrap();
        });

        // Verify RMQTT is reachable
        let http_client = Client::new();
        let stats_url = format!("http://127.0.0.1:{rmqtt_http_port}/api/v1/stats");
        let mut rmqtt_ok = false;
        for _ in 0..20 {
            if http_client.get(&stats_url).send().await.is_ok() {
                rmqtt_ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        assert!(
            rmqtt_ok,
            "RMQTT not reachable at {stats_url}; run scripts/test-start.py first"
        );

        MqttTestContext {
            backend_port,
            rmqtt_mqtt_port,
            _rmqtt_http_port: rmqtt_http_port,
            _admin_pool: admin_pool,
            schema_name,
            _app_state: app_state,
            _admin_state: admin_state,
            shutdown_tx,
            http_client,
            _temp_dir: temp_dir,
        }
    }

    async fn teardown(self) {
        let _ = self.shutdown_tx.send(true);
        drop_test_schema(&self._admin_pool, &self.schema_name).await;
    }
}

impl MqttTestContext {
    pub async fn admin_get(&self, path: &str) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}{path}", self.backend_port);
        let resp = self.http_client.get(&url).send().await.unwrap();
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        (status, body)
    }

    pub async fn admin_post_json<T: Serialize>(&self, path: &str, body: &T) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}{path}", self.backend_port);
        let resp = self.http_client.post(&url).json(body).send().await.unwrap();
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        (status, body)
    }

    pub async fn admin_patch_json<T: Serialize>(&self, path: &str, body: &T) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}{path}", self.backend_port);
        let resp = self
            .http_client
            .patch(&url)
            .json(body)
            .send()
            .await
            .unwrap();
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        (status, body)
    }

    pub async fn admin_post_json_with_headers<T: Serialize>(
        &self,
        path: &str,
        body: &T,
        headers: &[(&str, &str)],
    ) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}{path}", self.backend_port);
        let mut req = self.http_client.post(&url).json(body);
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        let resp = req.send().await.unwrap();
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        (status, body)
    }

    pub async fn connect_device(&self, product_id: &str, device_id: &str) -> MqttDeviceClient {
        let password = generate_mqtt_password(device_id, AUTH_SUFFIX);
        let mut options = rumqttc::MqttOptions::new(device_id, "127.0.0.1", self.rmqtt_mqtt_port);
        options.set_credentials(product_id, &password);
        options.set_keep_alive(Duration::from_secs(30));
        options.set_clean_session(true);

        let (client, eventloop) = rumqttc::AsyncClient::new(options, 10);

        let mut device = MqttDeviceClient {
            client,
            eventloop,
            product_id: product_id.to_string(),
            device_id: device_id.to_string(),
        };

        // Wait for ConnAck
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            tokio::select! {
                result = device.eventloop.poll() => {
                    match result {
                        Ok(rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_))) => break,
                        Ok(_) => continue,
                        Err(e) => panic!("MQTT connection failed: {e}"),
                    }
                }
                _ = tokio::time::sleep_until(deadline) => {
                    panic!("Timeout waiting for MQTT ConnAck");
                }
            }
        }

        // Explicitly subscribe to property/set topic (auto-subscription uses wildcard
        // topic that doesn't appear in RMQTT subscriptions API)
        let set_topic = format!("{}/{}/thing/service/property/set", product_id, device_id);
        device
            .client
            .subscribe(&set_topic, rumqttc::QoS::ExactlyOnce)
            .await
            .unwrap();

        // Drain events until SubAck arrives
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            tokio::select! {
                result = device.eventloop.poll() => {
                    match result {
                        Ok(rumqttc::Event::Incoming(rumqttc::Incoming::SubAck(_))) => break,
                        Ok(_) => continue,
                        Err(e) => panic!("EventLoop error during subscribe: {e}"),
                    }
                }
                _ = tokio::time::sleep_until(drain_deadline) => {
                    panic!("Timeout waiting for SubAck on {set_topic}");
                }
            }
        }

        device
    }
}

pub struct MqttDeviceClient {
    client: rumqttc::AsyncClient,
    eventloop: rumqttc::EventLoop,
    pub product_id: String,
    pub device_id: String,
}

#[derive(Debug)]
pub struct PropertyCommandMessage {
    pub id: String,
    pub ids: Vec<i64>,
    pub data: serde_json::Value,
}

impl MqttDeviceClient {
    pub async fn post_properties(&mut self, params: serde_json::Value) {
        let topic = format!(
            "{}/{}/thing/event/property/post",
            self.product_id, self.device_id
        );
        let payload = serde_json::json!({
            "id": format!("prop-{}", Uuid::new_v4().simple()),
            "ack": 0,
            "params": params,
        });
        self.client
            .publish(
                &topic,
                rumqttc::QoS::AtLeastOnce,
                false,
                serde_json::to_vec(&payload).unwrap(),
            )
            .await
            .unwrap();
        self.drain_events(Duration::from_millis(200)).await;
    }

    pub async fn post_event(&mut self, params: serde_json::Value) {
        let topic = format!(
            "{}/{}/thing/event/test/post",
            self.product_id, self.device_id
        );
        let payload = serde_json::json!({
            "id": format!("event-{}", Uuid::new_v4().simple()),
            "ack": 0,
            "params": params,
        });
        self.client
            .publish(
                &topic,
                rumqttc::QoS::AtLeastOnce,
                false,
                serde_json::to_vec(&payload).unwrap(),
            )
            .await
            .unwrap();
        self.drain_events(Duration::from_millis(200)).await;
    }

    pub async fn wait_for_command(&mut self, timeout: Duration) -> PropertyCommandMessage {
        let set_topic = format!(
            "{}/{}/thing/service/property/set",
            self.product_id, self.device_id
        );
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            tokio::select! {
                result = self.eventloop.poll() => {
                    match result {
                        Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(p))) => {
                            if p.topic == set_topic {
                                let raw: serde_json::Value =
                                    serde_json::from_slice(&p.payload).unwrap();
                                let params = &raw["params"];
                                return PropertyCommandMessage {
                                    id: raw["id"].as_str().unwrap_or_default().to_string(),
                                    ids: params["ids"]
                                        .as_array()
                                        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
                                        .unwrap_or_default(),
                                    data: params["data"].clone(),
                                };
                            }
                        }
                        Ok(_) => continue,
                        Err(e) => panic!("EventLoop error while waiting for command: {e}"),
                    }
                }
                _ = tokio::time::sleep_until(deadline) => {
                    panic!("Timeout waiting for property command on {set_topic}");
                }
            }
        }
    }

    pub async fn reply_command(&mut self, command: &PropertyCommandMessage, code: u32) {
        let reply_topic = format!(
            "{}/{}/thing/service/property/set_reply",
            self.product_id, self.device_id
        );
        let payload = serde_json::json!({
            "id": command.id,
            "code": code,
            "data": command.ids,
        });
        self.client
            .publish(
                &reply_topic,
                rumqttc::QoS::AtLeastOnce,
                false,
                serde_json::to_vec(&payload).unwrap(),
            )
            .await
            .unwrap();
        self.drain_events(Duration::from_millis(200)).await;
    }

    pub async fn disconnect(mut self) {
        self.client.disconnect().await.unwrap();
        // Drain remaining events
        let _ = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if self.eventloop.poll().await.is_err() {
                    break;
                }
            }
        })
        .await;
    }

    async fn drain_events(&mut self, duration: Duration) {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            tokio::select! {
                result = self.eventloop.poll() => {
                    if result.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep_until(deadline) => break,
            }
        }
    }
}

fn generate_mqtt_password(device_id: &str, suffix: &str) -> String {
    let nonce: String = rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let to_sign = format!("{device_id}.{nonce}.{timestamp}.{suffix}");
    let mut mac = HmacSha1::new_from_slice(suffix.as_bytes()).unwrap();
    mac.update(to_sign.as_bytes());
    let result = mac.finalize();
    let hash = hex::encode(result.into_bytes());
    format!("{nonce}.{timestamp}.{hash}")
}
