use crate::api::{
    admin_handlers, alarm_handlers, auth_handlers, ca_handlers, handlers, ota_handlers,
    product_handlers,
};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityRequirement, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "RMQTT Things API",
        version = env!("CARGO_PKG_VERSION"),
        description = "RMQTT IoT Things Management API"
    ),
    modifiers(&SecurityAddon),
    paths(
        auth_handlers::get_auth_config,
        auth_handlers::auth,
        auth_handlers::acl,
        handlers::property_set_subscribe,
        handlers::property_post,
        handlers::property_set_reply,
        handlers::event_post,
        handlers::file_upload_handler,
        ota_handlers::ota_version_post,
        handlers::device_connect,
        handlers::device_disconnect,
        handlers::health_check,
        admin_handlers::get_property_latest,
        admin_handlers::get_property_commands,
        admin_handlers::create_property_command,
        admin_handlers::delete_property_commands,
        admin_handlers::get_property_history,
        admin_handlers::get_event_history,
        admin_handlers::get_device_status,
        admin_handlers::get_device_status_history,
        admin_handlers::get_event_valid_templates,
        admin_handlers::create_event_valid_template,
        admin_handlers::get_event_valid_template,
        admin_handlers::update_event_valid_template,
        admin_handlers::update_event_valid_template_status,
        ca_handlers::list_certs_handler,
        ca_handlers::issue_cert_handler,
        ca_handlers::update_cert_status_handler,
        ca_handlers::get_ca_cert_handler,
        product_handlers::list_products,
        product_handlers::create_product,
        product_handlers::get_product,
        product_handlers::update_product,
        admin_handlers::get_ota_versions,
        admin_handlers::create_ota_version,
        admin_handlers::get_ota_version,
        admin_handlers::update_ota_version,
        admin_handlers::delete_ota_version,
        admin_handlers::admin_file_upload_handler,
        alarm_handlers::list_alarm_rules,
        alarm_handlers::create_alarm_rule,
        alarm_handlers::get_alarm_rule,
        alarm_handlers::update_alarm_rule,
        alarm_handlers::update_alarm_rule_status,
        alarm_handlers::delete_alarm_rule,
        alarm_handlers::list_alarms,
        alarm_handlers::ack_alarm,
        alarm_handlers::clear_alarm,
    ),
    components(
        schemas(
            crate::api::admin_models::CertificatesListResponse,
            crate::api::admin_models::CommonQuery,
            crate::api::admin_models::CommonQuery2,
            crate::api::admin_models::CreateEventValidTemplateRequest,
            crate::api::admin_models::CreateOtaVersionRequest,
            crate::api::admin_models::CreatePropertyCommandRequest,
            crate::api::admin_handlers::DeletePropertyCommandsQuery,
            crate::api::admin_models::DeviceStatusHistoryListResponse,
            crate::api::admin_models::DeviceStatusListResponse,
            crate::api::admin_models::EventHistoryListResponse,
            crate::api::admin_models::EventValidTemplateListResponse,
            crate::api::admin_models::EventValidTemplateQuery,
            crate::api::admin_models::OtaVersionListResponse,
            crate::api::admin_models::OtaVersionQuery,
            crate::api::admin_models::PaginatedResponse<crate::db::models::DeviceStatusWithSource>,
            crate::api::admin_models::PaginatedResponse<crate::db::models::EventValidTemplate>,
            crate::api::admin_models::PaginatedResponse<crate::db::models::OtaVersion>,
            crate::api::admin_models::PaginatedResponse<crate::db::models::Product>,
            crate::api::admin_models::PaginatedResponse<crate::db::models::PropertyCommand>,
            crate::api::admin_models::PaginationInfo,
            crate::api::admin_models::ProductQuery,
            crate::api::admin_models::PropertyCommandListResponse,
            crate::api::admin_models::PropertyCommandQuery,
            crate::api::admin_models::PropertyHistoryListResponse,
            crate::api::admin_models::PropertyLatestListResponse,
            crate::api::admin_models::SimplePaginatedResponse<crate::db::models::CertIssue>,
            crate::api::admin_models::SimplePaginatedResponse<crate::db::models::DeviceStatusHistory>,
            crate::api::admin_models::SimplePaginatedResponse<crate::db::models::EventHistory>,
            crate::api::admin_models::SimplePaginatedResponse<crate::db::models::PropertyHistory>,
            crate::api::admin_models::SimplePaginatedResponse<crate::db::models::PropertyLatest>,
            crate::api::admin_models::SimplePaginationInfo,
            crate::api::admin_models::UpdateEventValidTemplateRequest,
            crate::api::admin_models::UpdateEventValidTemplateStatusRequest,
            crate::api::admin_models::UpdateOtaVersionRequest,
            crate::api::auth_handlers::AclPayload,
            crate::api::auth_handlers::AuthPayload,
            crate::api::auth_handlers::Access,
            crate::api::auth_handlers::AuthConfigResponse,
            crate::api::auth_handlers::MqttProtocol,
            crate::api::ca_handlers::IssueCertRequest,
            crate::api::ca_handlers::IssueCertResponse,
            crate::api::ca_handlers::UpdateCertStatusRequest,
            crate::api::ca_handlers::CaCertResponse,
            crate::api::error::ApiErrorResponse,
            crate::api::handlers::PropertySetReplyPayload,
            crate::api::web_models::AckStatus,
            crate::api::web_models::DeviceConnectRequest,
            crate::api::web_models::DeviceDisconnectRequest,
            crate::api::web_models::FileUploadRequest,
            crate::api::web_models::FileUploadResponse,
            crate::api::web_models::MqttPayload,
            crate::api::web_models::MqttResponse,
            crate::api::web_models::OtaReport,
            crate::api::web_models::RMqttPublishMessage,
            crate::api::web_models::RMqttSubscribeMessage,
            crate::db::models::CertIssue,
            crate::db::models::CertStatus,
            crate::db::models::CommandStatus,
            crate::db::models::CreateProductRequest,
            crate::db::models::DeviceConnectionStatus,
            crate::db::models::DeviceStatusWithSource,
            crate::db::models::RegistrationSource,
            crate::db::models::DeviceStatusHistory,
            crate::db::models::EventHistory,
            crate::db::models::EventValidTemplate,
            crate::db::models::EventValidTemplateStatus,
            crate::db::models::OtaVersion,
            crate::db::models::Product,
            crate::db::models::ProductStatus,
            crate::db::models::PropertyCommand,
            crate::db::models::PropertyHistory,
            crate::db::models::PropertyLatest,
            crate::db::models::UpdateProductRequest,
            crate::db::models::AlarmRule,
            crate::api::alarm_models::AlarmRuleQuery,
            crate::api::alarm_models::CreateAlarmRuleRequest,
            crate::api::alarm_models::UpdateAlarmRuleRequest,
            crate::api::alarm_models::UpdateAlarmRuleStatusRequest,
            crate::api::alarm_models::AlarmRuleResponse,
            crate::api::admin_models::PaginatedResponse<crate::db::models::AlarmRule>,
            crate::api::alarm_models::AlarmQuery,
            crate::api::alarm_models::ApiAlarmRecord,
            crate::api::alarm_models::AlarmRecordResponse,
            crate::api::admin_models::PaginatedResponse<crate::api::alarm_models::ApiAlarmRecord>,
        )
    ),
    tags(
        (name = "access", description = "RMQTT access callbacks"),
        (name = "thing", description = "RMQTT thing callbacks"),
        (name = "device", description = "Device lifecycle callbacks"),
        (name = "admin", description = "Administrative APIs"),
        (name = "system", description = "System APIs")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "cookie_auth",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("X-Auth"))),
            );
        }

        let security = vec![SecurityRequirement::new(
            "cookie_auth",
            Vec::<String>::new(),
        )];
        for (path, path_item) in openapi.paths.paths.iter_mut() {
            if !path.starts_with("/api/admin/") {
                continue;
            }

            for operation in [
                &mut path_item.get,
                &mut path_item.put,
                &mut path_item.post,
                &mut path_item.delete,
                &mut path_item.patch,
            ]
            .into_iter()
            .flatten()
            {
                operation.security = Some(security.clone());
            }
        }
    }
}
