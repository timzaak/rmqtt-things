use crate::cache::SchemaCache;
use crate::config::Config;
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;

use anyhow::bail;
use axum::Router;
use axum::routing::{get, patch, post, put};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod admin_handlers;
pub mod admin_models;
pub mod alarm_handlers;
pub mod alarm_models;
pub mod auth_handlers;
pub mod ca_handlers;
pub mod error;
pub mod factory_handlers;
pub mod factory_middleware;
pub mod handlers;
pub mod middleware;
pub mod openapi;
pub mod ota_handlers;
pub mod product_handlers;
pub mod shadow;
#[cfg(test)]
mod tests;
pub mod utils;
pub mod web_models;

use crate::api::factory_middleware::FactoryAuthState;
use crate::api::handlers::S3Client;
use crate::api::middleware::{HeraldAuthState, herald_auth_middleware, internal_ip_middleware};
use crate::api::openapi::ApiDoc;
use crate::api::web_models::MqttResponse;

pub struct AdminAppState {
    pub db: DatabaseService,
    pub rmqtt_client: RmqttHttpClient,
    pub cache: SchemaCache,
    pub config: Arc<Config>,
    pub s3_client: Option<S3Client>,
    pub rule_cache: crate::rule_engine::RuleCache,
    pub task_set: Arc<Mutex<JoinSet<()>>>,
}

#[derive(Clone)]
pub struct ApiState {
    pub app: Arc<handlers::AppState>,
    pub admin: Arc<AdminAppState>,
}

pub fn create_router(
    config: Arc<Config>,
    app_state: Arc<handlers::AppState>,
    admin_state: Arc<AdminAppState>,
    herald_client: Option<Arc<herald_sdk::Client>>,
    factory_auth_state: Arc<FactoryAuthState>,
) -> Router {
    let state = Arc::new(ApiState {
        app: app_state,
        admin: admin_state,
    });

    let health_route = Router::new().route("/health", get(handlers::health_check));

    // Browser-facing auth endpoints. NOT behind internal_ip_middleware — these
    // are reached from the admin web console over the public network (a user on
    // a non-private IP must still be able to start OAuth, refresh an access
    // token, and log out). Only the rmqtt-broker webhook routes below are
    // internal-IP-gated.
    let auth_routes = Router::new()
        .route("/auth/config", get(auth_handlers::get_auth_config))
        .route("/auth/oauth/start", get(auth_handlers::oauth_start))
        .route("/auth/oauth/callback", get(auth_handlers::oauth_callback))
        .route("/auth/refresh", post(auth_handlers::refresh_token))
        .route("/auth/logout", post(auth_handlers::logout));

    let webhook_routes = Router::new()
        .route("/access/auth", post(auth_handlers::auth))
        .route("/access/acl", post(auth_handlers::acl))
        .route(
            "/thing/property/set_subscribe",
            post(handlers::property_set_subscribe),
        )
        .route("/thing/property/post", post(handlers::property_post))
        .route(
            "/thing/property/set_reply",
            post(handlers::property_set_reply),
        )
        .route("/thing/event/post", post(handlers::event_post))
        .route("/thing/file/upload", post(handlers::file_upload_handler))
        .route(
            "/thing/factory-metadata/get",
            post(factory_handlers::factory_metadata_get_handler),
        )
        .route("/ota/version", post(ota_handlers::ota_version_post))
        .route("/device/connect", post(handlers::device_connect))
        .route("/device/disconnect", post(handlers::device_disconnect))
        .layer(axum::middleware::from_fn(internal_ip_middleware));

    let device_routes = health_route.merge(webhook_routes);

    let admin_routes = Router::new()
        .route("/admin/property", get(admin_handlers::get_property_latest))
        .route(
            "/admin/property/command",
            get(admin_handlers::get_property_commands)
                .post(admin_handlers::create_property_command)
                .delete(admin_handlers::delete_property_commands),
        )
        .route(
            "/admin/property/shadow/desired",
            put(admin_handlers::set_property_desired),
        )
        .route(
            "/admin/property/shadow",
            get(admin_handlers::get_property_shadow),
        )
        .route(
            "/admin/property/history",
            get(admin_handlers::get_property_history),
        )
        .route("/admin/event", get(admin_handlers::get_event_history))
        .route(
            "/admin/device/status",
            get(admin_handlers::get_device_status),
        )
        .route(
            "/admin/device/status/history",
            get(admin_handlers::get_device_status_history),
        )
        .route(
            "/admin/valid/event",
            get(admin_handlers::get_event_valid_templates)
                .post(admin_handlers::create_event_valid_template),
        )
        .route(
            "/admin/valid/event/{id}",
            get(admin_handlers::get_event_valid_template)
                .patch(admin_handlers::update_event_valid_template)
                .delete(admin_handlers::delete_event_valid_template),
        )
        .route(
            "/admin/valid/event/{id}/status",
            patch(admin_handlers::update_event_valid_template_status),
        )
        .route(
            "/admin/ca/cert",
            get(ca_handlers::list_certs_handler).post(ca_handlers::issue_cert_handler),
        )
        .route("/admin/ca/pem", get(ca_handlers::get_ca_cert_handler))
        .route("/admin/ca/cert/{id}", get(ca_handlers::get_cert_handler))
        .route(
            "/admin/ca/cert/status",
            patch(ca_handlers::update_cert_status_handler),
        )
        .route(
            "/admin/product",
            get(product_handlers::list_products).post(product_handlers::create_product),
        )
        .route(
            "/admin/product/{id}",
            get(product_handlers::get_product).patch(product_handlers::update_product),
        )
        .route(
            "/admin/ota/version",
            get(admin_handlers::get_ota_versions).post(admin_handlers::create_ota_version),
        )
        .route(
            "/admin/ota/version/{id}",
            get(admin_handlers::get_ota_version)
                .put(admin_handlers::update_ota_version)
                .delete(admin_handlers::delete_ota_version),
        )
        .route(
            "/admin/file/upload",
            post(admin_handlers::admin_file_upload_handler),
        )
        // GET /admin/file/download-url — presigned S3 download URL for file
        // attachments (design §4.2.2 F / §5.5). Shares the existing
        // `admin_routes` group and Herald middleware; `extract_permission`
        // already maps `/admin/file/*` to the `product` resource, so Herald
        // `product:read` governs this GET (single-tenant deployments pass
        // through). No path params, so no axum 0.8 `{name}` syntax needed.
        .route(
            "/admin/file/download-url",
            get(admin_handlers::admin_file_download_url_handler),
        )
        .route(
            "/admin/alarm-rule",
            get(alarm_handlers::list_alarm_rules).post(alarm_handlers::create_alarm_rule),
        )
        .route(
            "/admin/alarm-rule/{id}",
            get(alarm_handlers::get_alarm_rule)
                .patch(alarm_handlers::update_alarm_rule)
                .delete(alarm_handlers::delete_alarm_rule),
        )
        .route(
            "/admin/alarm-rule/{id}/status",
            patch(alarm_handlers::update_alarm_rule_status),
        )
        .route("/admin/alarm", get(alarm_handlers::list_alarms))
        .route("/admin/alarm/{id}/ack", patch(alarm_handlers::ack_alarm))
        .route(
            "/admin/alarm/{id}/clear",
            patch(alarm_handlers::clear_alarm),
        )
        // Factory admin read routes (design §4.2.2 C/D + §5.4). These share the
        // existing admin_routes group — Herald `device:read` applies once
        // `extract_permission` maps `/admin/factory/*` to the `device` resource
        // (middleware/mod.rs). Single-tenant (no Herald) deployments pass through.
        .route(
            "/admin/factory/devices/{device_sn}",
            get(factory_handlers::get_factory_device_view_handler),
        )
        .route(
            "/admin/factory/sn/{sn}/changes",
            get(factory_handlers::query_component_changes_handler),
        );

    let admin_routes = match (config.herald.as_ref(), herald_client) {
        (Some(herald_config), Some(herald_sdk)) => {
            admin_routes.layer(axum::middleware::from_fn_with_state(
                HeraldAuthState {
                    herald_sdk,
                    client_id: herald_config.client_id.clone().into(),
                },
                herald_auth_middleware,
            ))
        }
        (_, _) => admin_routes,
    };

    // Independent factory routes behind `factory_auth_middleware` (design §5.4).
    // Shares `Arc<ApiState>` state type with the other route groups (axum 0.8
    // `Router::merge` requires a single shared state type). Path params use the
    // 0.8 `{name}` syntax — `:name` would be treated as a literal segment.
    let factory_routes = Router::new()
        .route(
            "/factory/file/upload",
            post(factory_handlers::factory_file_upload_handler),
        )
        .route(
            "/factory/components/{component_sn}",
            put(factory_handlers::upsert_component_handler),
        )
        .route(
            "/factory/devices/{device_sn}",
            put(factory_handlers::upsert_device_metadata_handler),
        )
        .route(
            "/factory/devices/{device_sn}/components",
            put(factory_handlers::replace_associations_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            factory_auth_state,
            factory_middleware::factory_auth_middleware,
        ));

    let api_routes = device_routes
        .merge(admin_routes)
        .merge(factory_routes)
        .merge(auth_routes);

    let otel_enabled = config.otel.trace.is_some() || config.otel.log.is_some();

    let mut router = Router::new().nest("/api", api_routes);

    if !otel_enabled {
        const X_REQUEST_ID: &str = "x-request-id";
        let x_request_id = X_REQUEST_ID.parse::<axum::http::HeaderName>().unwrap();
        router = router.layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(
                    x_request_id.clone(),
                    MakeRequestUuid,
                ))
                .layer(PropagateRequestIdLayer::new(x_request_id))
                .layer(TraceLayer::new_for_http()),
        );
    } else {
        router = router.layer(TraceLayer::new_for_http());
    }

    let mut router = router.with_state(state);

    if config.api.openapi_enabled {
        info!("OpenAPI UI enabled at http://127.0.0.1:8080/swagger");
        router = router
            .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", ApiDoc::openapi()));
    } else {
        debug!("OpenAPI UI is disabled");
    }

    if let Some(web_path) = &config.api.serve_web_path
        && !web_path.is_empty()
    {
        info!("Serving static files from: {}", web_path);
        let index_html = format!("{}/index.html", web_path.trim_end_matches('/'));
        router =
            router.fallback_service(ServeDir::new(web_path).fallback(ServeFile::new(index_html)));
    }

    router
}

pub fn export_openapi_to_file(output_path: &Path) -> anyhow::Result<()> {
    let json = ApiDoc::openapi().to_pretty_json()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, json)?;
    println!("OpenAPI JSON exported to: {}", output_path.display());
    Ok(())
}

async fn ack_response(
    id: String,
    rmqtt_client: &RmqttHttpClient,
    topic: &str,
) -> anyhow::Result<()> {
    let response = MqttResponse {
        id,
        code: 200,
        data: None,
    };

    let response_payload = serde_json::to_string(&response)?;

    if let Err(e) = rmqtt_client
        .publish_response(topic, &response_payload)
        .await
    {
        bail!("Failed to publish response: {}", e);
    }
    Ok(())
}
