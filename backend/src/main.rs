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
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;

use crate::api::{AdminAppState, create_router};

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

    let db_service = DatabaseService::new(pool);
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

    let admin_state = Arc::new(AdminAppState {
        db: db_service,
        rmqtt_client: rmqtt_client_clone,
        cache: schema_cache,
        config: Arc::clone(&config),
        s3_client,
        rule_cache: crate::rule_engine::RuleCache::new(),
    });

    let router = create_router(config, app_state, admin_state, herald_client);

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!("Listening on port {port}");

    tokio::select! {
        result = axum::serve(listener, router) => {
            result?;
            info!("Server stopped");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }
    info!("Shutting down...");
    Ok(())
}
