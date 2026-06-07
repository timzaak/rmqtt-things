mod api;
mod ca;
mod cache;
mod config;
mod db;
mod rmqtt_client;
mod rule_engine;
mod telemetry;

use crate::cache::{InMemorySchemaCache, RedisSchemaCache, SchemaCache};
use crate::config::{CacheType, Config};
use crate::rmqtt_client::RmqttHttpClient;
use crate::telemetry::init_telemetry;
use api::handlers::{AppState, S3Client};
use clap::Parser;
use db::database::DatabaseService;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::api::{AdminAppState, create_router};
use crate::rule_engine::{InMemoryRuleStateStore, RedisRuleStateStore, RuleCache, RuleStateStore};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Export OpenAPI JSON to the specified file and exit
    #[arg(long)]
    export_openapi: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(output_path) = args.export_openapi {
        return api::export_openapi_to_file(&output_path);
    }

    let config_path = env::var("APP_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    let config = Arc::new(Config::from_file(&config_path)?);

    // Generate or validate CA files
    ca::init_ca(&config.ca).await?;

    let log_filter = || {
        EnvFilter::builder()
            .with_default_directive(Level::INFO.into())
            .from_env_lossy()
            .add_directive("h2=info".parse().unwrap())
            .add_directive("hyper_util=info".parse().unwrap())
            .add_directive("tower=info".parse().unwrap())
    };

    if let Err(e) = init_telemetry(&config.otel, log_filter) {
        eprintln!("Failed to initialize telemetry: {e}");
    }

    // 连接数据库
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database.url)
        .await?;
    // 运行数据库迁移
    sqlx::migrate!("./migrations").run(&pool).await?;

    let db_service = DatabaseService::new(pool, config.alarm.clone());
    let rmqtt_client = RmqttHttpClient::new(config.mqtt.clone());
    let rmqtt_client_clone = RmqttHttpClient::new(config.mqtt.clone());
    let schema_cache = match config.cache.cache_type {
        CacheType::Redis => {
            let redis_url = config
                .cache
                .redis_url
                .as_deref()
                .unwrap_or("redis://127.0.0.1/");
            let redis_cache = RedisSchemaCache::new(redis_url)?;
            SchemaCache::Redis(Arc::new(redis_cache))
        }
        CacheType::Memory => SchemaCache::InMemory(Arc::new(InMemorySchemaCache::new())),
    };
    let s3_client = if let Some(s3_config) = &config.s3 {
        Some(S3Client::new(s3_config)?)
    } else {
        None
    };
    let herald_client = config.herald.as_ref().map(|herald| {
        Arc::new(herald_sdk::Client::new(
            herald.base_url.clone(),
            herald.api_key.clone(),
            None,
        ))
    });

    let app_state = Arc::new(AppState {
        db: db_service.clone(),
        rmqtt_client,
        config: Arc::clone(&config),
        cache: schema_cache.clone(),
        s3_client: s3_client.clone(),
    });

    let cancel_token = CancellationToken::new();
    let task_set: Arc<Mutex<JoinSet<()>>> = Arc::new(Mutex::new(JoinSet::new()));

    let state_store: Arc<dyn RuleStateStore> = match config.cache.cache_type {
        CacheType::Redis => {
            let redis_url = config
                .cache
                .redis_url
                .as_deref()
                .unwrap_or("redis://127.0.0.1/");
            Arc::new(RedisRuleStateStore::new(redis_url)?)
        }
        CacheType::Memory => Arc::new(InMemoryRuleStateStore::new()),
    };
    let rule_cache = RuleCache::new(state_store);

    let admin_state = Arc::new(AdminAppState {
        db: db_service.clone(),
        rmqtt_client: rmqtt_client_clone,
        cache: schema_cache,
        config: Arc::clone(&config),
        s3_client,
        rule_cache,
        task_set: Arc::clone(&task_set),
    });

    let retry_interval = config.alarm.webhook_retry_interval_seconds;
    let max_retries = config.alarm.webhook_max_retries;

    let router = create_router(config, app_state, admin_state, herald_client);

    // Spawn the background webhook retry task
    {
        let task_set_clone = task_set.clone();
        let db_clone = db_service;
        let cancel_clone = cancel_token.clone();
        task_set_clone.lock().await.spawn(webhook_retry_task(
            db_clone,
            cancel_clone,
            retry_interval,
            max_retries,
        ));
    }

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!("Listening on port {port}");

    let shutdown_token = cancel_token.clone();
    let app = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
        }) => {
            if let Err(e) = result {
                warn!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received ctrl_c, shutting down");
        }
    }

    // Always cancel, even if server exited on its own (idempotent).
    cancel_token.cancel();

    // Drain remaining tasks with 10s timeout
    match tokio::time::timeout(Duration::from_secs(10), async {
        let mut set = task_set.lock().await;
        while set.join_next().await.is_some() {}
    })
    .await
    {
        Ok(()) => info!("All tasks completed"),
        Err(_) => warn!("Timeout waiting for tasks to complete, forcing shutdown"),
    }

    info!("Shutting down...");
    Ok(())
}

/// Retry webhooks for a single alarm record.
///
/// Retrieves the rule's actions to find webhook URLs, reconstructs the payload
/// from AlarmRecord fields, and calls each webhook. Returns Ok(()) only if all
/// webhooks succeed.
async fn retry_single_webhook(
    db: &DatabaseService,
    alarm: &db::models::AlarmRecord,
) -> anyhow::Result<()> {
    let alarm_repo = db.alarm();

    let actions_json = match alarm_repo.get_rule_actions(alarm.rule_id).await? {
        Some(actions) => actions,
        None => {
            warn!(
                "Rule {} no longer exists, skipping retry for alarm {}",
                alarm.rule_id, alarm.id
            );
            return Ok(());
        }
    };

    let actions_arr = actions_json.as_array().cloned().unwrap_or_default();
    let parsed = rule_engine::actions::parse_actions(&actions_arr);

    let webhook_urls: Vec<String> = parsed
        .iter()
        .filter_map(|a| match a {
            rule_engine::actions::AlarmAction::Webhook { url } => Some(url.clone()),
            _ => None,
        })
        .collect();

    if webhook_urls.is_empty() {
        warn!(
            "No webhook actions found for rule {}, skipping retry for alarm {}",
            alarm.rule_id, alarm.id
        );
        return Ok(());
    }

    let payload = serde_json::json!({
        "rule_name": alarm.rule_name,
        "product_id": alarm.product_id,
        "device_id": alarm.device_id,
        "trigger_type": alarm.trigger_type,
        "trigger_value": alarm.trigger_value,
    });

    for url in &webhook_urls {
        rule_engine::actions::send_webhook(url, &payload, Duration::from_secs(5)).await?;
    }

    Ok(())
}

/// Background task that periodically queries pending webhook retries and re-executes them.
async fn webhook_retry_task(
    db: DatabaseService,
    cancel_token: CancellationToken,
    retry_interval: u64,
    _max_retries: i16,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(retry_interval));
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Webhook retry task shutting down");
                break;
            }
            _ = interval.tick() => {
                let alarm_repo = db.alarm();
                let pending = match alarm_repo.query_pending_retries().await {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("Failed to query pending retries: {}", e);
                        continue;
                    }
                };
                for alarm in &pending {
                    let result = retry_single_webhook(&db, alarm).await;
                    match result {
                        Ok(()) => {
                            if let Err(e) = alarm_repo.mark_webhook_success(alarm.id).await {
                                error!("Failed to mark webhook success for alarm {}: {}", alarm.id, e);
                            }
                        }
                        Err(e) => {
                            warn!("Webhook retry failed for alarm {}: {}", alarm.id, e);
                            if let Err(e) = alarm_repo.decrement_retry_and_schedule_next(alarm.id).await {
                                error!("Failed to decrement retry for alarm {}: {}", alarm.id, e);
                            }
                        }
                    }
                }
            }
        }
    }
}
