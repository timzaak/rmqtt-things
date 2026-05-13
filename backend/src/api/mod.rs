use crate::cache::SchemaCache;
use crate::config::Config;
use crate::db::database::DatabaseService;
use crate::rmqtt_client::RmqttHttpClient;

use anyhow::bail;
use axum::Router;
use axum::routing::{get, patch, post};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod admin_handlers;
pub mod admin_models;
pub mod auth_handlers;
pub mod ca_handlers;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod openapi;
pub mod ota_handlers;
pub mod product_handlers;
#[cfg(test)]
mod tests;
pub mod utils;
pub mod web_models;

use crate::api::handlers::S3Client;
use crate::api::middleware::{HeraldAuthState, herald_auth_middleware};
use crate::api::openapi::ApiDoc;
use crate::api::web_models::MqttResponse;

pub struct AdminAppState {
    pub db: DatabaseService,
    pub rmqtt_client: RmqttHttpClient,
    pub cache: SchemaCache,
    pub config: Arc<Config>,
    pub s3_client: Option<S3Client>,
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
) -> Router {
    let state = Arc::new(ApiState {
        app: app_state,
        admin: admin_state,
    });

    let device_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/auth/config", get(auth_handlers::get_auth_config))
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
        .route("/ota/version", post(ota_handlers::ota_version_post))
        .route("/device/connect", post(handlers::device_connect))
        .route("/device/disconnect", post(handlers::device_disconnect));

    let admin_routes = Router::new()
        .route("/admin/property", get(admin_handlers::get_property_latest))
        .route(
            "/admin/property/command",
            get(admin_handlers::get_property_commands)
                .post(admin_handlers::create_property_command)
                .delete(admin_handlers::delete_property_commands),
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
                .patch(admin_handlers::update_event_valid_template),
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

    let api_routes = device_routes.merge(admin_routes);

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
        router = router.fallback_service(ServeDir::new(web_path));
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
