use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(default)]
    pub otel: OtelConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub s3: Option<S3Config>,
    #[serde(default)]
    pub ca: CAConfig,
    #[serde(default)]
    pub herald: Option<HeraldConfig>,
    #[serde(default)]
    pub alarm: AlarmConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CAConfig {
    pub ca_dir: String,
    pub name: String,
    pub valid_days: i64,
    pub domain: String,
}

impl Default for CAConfig {
    fn default() -> Self {
        Self {
            ca_dir: "conf".to_string(),
            name: "RMQTT Thing CA".to_string(),
            valid_days: 365 * 100 + 3,
            domain: "*.fornetcode.com".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub directories: Vec<String>,
    pub expired_seconds: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeraldConfig {
    pub base_url: String,
    pub api_key: String,
    pub realm_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    Memory,
    Redis,
}

fn default_cache_type() -> CacheType {
    CacheType::Memory
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    pub cache_type: CacheType,
    pub redis_url: Option<String>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: default_cache_type(),
            redis_url: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttConfig {
    pub url: String,
    pub publish: MqttPublishConfig,
    #[serde(default)]
    pub property_command: PropertyCommandConfig,
    #[serde(default)]
    pub access: AccessConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttPublishConfig {
    pub response: MqttResponseConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PropertyCommandConfig {
    pub publish: PropertyCommandPublishConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PropertyCommandPublishConfig {
    #[serde(default = "default_qos")]
    pub qos: u8,
    #[serde(default)]
    pub retain: bool,
    pub clientid: String,
    pub topic: String,
    #[serde(default = "default_retries")]
    pub retries: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttResponseConfig {
    #[serde(default = "default_qos")]
    pub qos: u8,
    #[serde(default)]
    pub retain: bool,
    pub clientid: String,
}

fn default_qos() -> u8 {
    2
}

fn default_retries() -> u8 {
    2
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:6000/api/api/v1/mqtt/publish".to_string(),
            publish: MqttPublishConfig {
                response: MqttResponseConfig {
                    qos: 2,
                    retain: false,
                    clientid: "rmqtt_things".to_string(),
                },
            },
            property_command: PropertyCommandConfig::default(),
            access: AccessConfig::default(),
        }
    }
}

impl Default for PropertyCommandConfig {
    fn default() -> Self {
        Self {
            publish: PropertyCommandPublishConfig {
                qos: 2,
                retain: false,
                clientid: "rmqtt_things".to_string(),
                topic: "thing/$clientid/thing/service/property/set".to_string(),
                retries: 2,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct OtelConfig {
    pub log: Option<String>,
    pub trace: Option<String>,
    pub metrics: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    #[serde(default = "default_openapi_enabled")]
    pub openapi_enabled: bool,
    pub serve_web_path: Option<String>,
    /// 运行时 Schema 校验总开关。**同时**控制属性上报（property_post）和事件上报
    ///（event_post）的 JSON Schema 校验；命名为 `property_schema_validator` 是历史
    /// 原因，实际门控两类上报。默认 false（不校验）。
    #[serde(default = "default_property_schema_validator")]
    pub property_schema_validator: bool,
}

fn default_openapi_enabled() -> bool {
    true
}

fn default_property_schema_validator() -> bool {
    false
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            openapi_enabled: true,
            serve_web_path: None,
            property_schema_validator: false,
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AccessConfig {
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    pub suffix: String,
    #[serde(default = "default_auth_timestamp_tolerance_secs")]
    pub timestamp_tolerance_secs: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            suffix: "default_suffix".to_string(),
            timestamp_tolerance_secs: default_auth_timestamp_tolerance_secs(),
        }
    }
}

fn default_auth_timestamp_tolerance_secs() -> i64 {
    300
}

fn default_webhook_max_retries() -> i16 {
    3
}

fn default_webhook_retry_interval_seconds() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlarmConfig {
    #[serde(default = "default_webhook_max_retries")]
    pub webhook_max_retries: i16,
    #[serde(default = "default_webhook_retry_interval_seconds")]
    pub webhook_retry_interval_seconds: u64,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            webhook_max_retries: default_webhook_max_retries(),
            webhook_retry_interval_seconds: default_webhook_retry_interval_seconds(),
        }
    }
}
