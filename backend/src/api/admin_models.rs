use crate::db::models::{
    CertIssue, CommandStatus, DeviceConnectionStatus, DeviceStatusHistory, DeviceStatusWithSource,
    EventHistory, EventValidTemplate, EventValidTemplateStatus, PropertyCommand, PropertyHistory,
    PropertyLatest, RegistrationSource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

// 通用查询参数结构
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CommonQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID，可选
    pub device_id: Option<String>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

// 属性命令查询参数结构
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PropertyCommandQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID，可选
    pub device_id: Option<String>,
    /// 命令状态：0=pending, 1=sent, 2=success, 3=failed, 4=deleted
    pub status: Option<CommandStatus>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

// 设备状态查询参数结构
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CommonQuery2 {
    /// 产品ID
    pub product_id: Option<String>,
    /// 设备ID，可选
    #[serde(default)]
    pub device_id: Option<String>,
    /// 设备状态: online, offline
    #[serde(default)]
    pub status: Option<DeviceConnectionStatus>,
    /// 注册来源: 0=auto, 1=manual
    #[serde(default)]
    pub registration_source: Option<RegistrationSource>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    10
}

// 创建命令请求结构
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePropertyCommandRequest {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
    /// 命令内容
    pub command: JsonValue,
}

// 通用分页响应结构
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// 数据列表
    pub data: Vec<T>,
    /// 分页信息
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    /// 当前页码
    pub page: i64,
    /// 每页大小
    pub page_size: i64,
    /// 总记录数
    pub total: i64,
}

// 不包含总数的分页信息
#[derive(Debug, Serialize, ToSchema, Default)]
pub struct SimplePaginationInfo {
    /// 当前页码
    pub page: i64,
    /// 每页大小
    pub page_size: i64,
}

// 简单分页响应结构（不包含总数）
#[derive(Debug, Serialize, ToSchema)]
pub struct SimplePaginatedResponse<T> {
    /// 数据列表
    pub data: Vec<T>,
    /// 分页信息
    pub pagination: SimplePaginationInfo,
}

// 类型别名
pub type PropertyCommandListResponse = PaginatedResponse<PropertyCommand>;
pub type PropertyLatestListResponse = SimplePaginatedResponse<PropertyLatest>;

pub type CertificatesListResponse = SimplePaginatedResponse<CertIssue>;
pub type PropertyHistoryListResponse = SimplePaginatedResponse<PropertyHistory>;
pub type EventHistoryListResponse = SimplePaginatedResponse<EventHistory>;
pub type DeviceStatusListResponse = PaginatedResponse<DeviceStatusWithSource>;
pub type DeviceStatusHistoryListResponse = SimplePaginatedResponse<DeviceStatusHistory>;
pub type EventValidTemplateListResponse = PaginatedResponse<EventValidTemplate>;
// 产品查询参数结构
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProductQuery {
    /// 搜索关键字，模糊匹配产品名称或型号
    pub search: Option<String>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventValidTemplateQuery {
    pub product_id: Option<String>,
    pub event: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateEventValidTemplateRequest {
    pub product_id: String,
    pub event: String,
    pub description: Option<String>,
    pub schema: JsonValue,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateEventValidTemplateRequest {
    pub schema: Option<JsonValue>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateEventValidTemplateStatusRequest {
    pub status: EventValidTemplateStatus,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OtaVersionQuery {
    pub product_id: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

/// Query parameters for `GET /api/admin/factory/components/{componentSn}/changes`
/// (design §4.2.2 D). The repo has no shared `PaginationQuery`; each paginated
/// endpoint carries its own query struct following the existing convention
/// (`PropertyCommandQuery` / `CommonQuery` / etc.). `page` is 1-based.
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FactoryChangeLogQuery {
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateOtaVersionRequest {
    pub product_id: String,
    pub key: String,
    pub version: String,
    pub max_version: Option<String>,
    pub min_version: String,
    pub file_key: String,
    pub log: Option<JsonValue>,
    pub device_ids: Option<Vec<String>>,
    pub bin_length: i64,
    pub bin_md5: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateOtaVersionRequest {
    pub max_version: Option<String>,
    pub min_version: Option<String>,
    pub file_key: Option<String>,
    pub log: Option<JsonValue>,
    pub device_ids: Option<Vec<String>>,
    pub bin_length: Option<i64>,
    pub bin_md5: Option<String>,
}

pub type OtaVersionListResponse = PaginatedResponse<crate::db::models::OtaVersion>;

// ---- Shadow (desired state) DTOs (design shadow-device-support.md §5.2) ----

/// Set-Desired request body: a partial patch over desired state.
///
/// `desired` follows the RFC 7396 subset: non-null values set/overwrite the
/// desired key, `null` values delete it. An empty object `{}` is rejected by
/// the handler (US-PA-042 scenario 4) before reaching the repository.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetDesiredRequest {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
    /// 期望属性 patch（RFC 7396 子集：非 null 覆盖、null 删除）
    pub desired: serde_json::Map<String, JsonValue>,
}

/// Set-Desired response: the merged desired document, the delta computed for
/// this write, and whether a delta command was enqueued for delivery.
#[derive(Debug, Serialize, ToSchema)]
pub struct SetDesiredResponse {
    /// 合并后的完整 desired 文档（裸值）
    pub desired: JsonValue,
    /// 本次 Set-Desired 触发的 delta（待收敛属性，裸期望值）
    pub delta: JsonValue,
    /// 是否插入了 delta 命令（delta 非空为 true；在线即时/离线排队均为 true）
    pub pushed: bool,
}

/// Get-Delta response: the current desired document, the reported snapshot
/// (kept in the `{value, time}` shape for frontend consistency), and the
/// per-property delta. Document-level timestamps let the frontend show when
/// desired / reported were last updated without a per-key metadata layer.
#[derive(Debug, Serialize, ToSchema)]
pub struct ShadowView {
    /// 当前 desired 文档（裸值）；无则 `{}`
    pub desired: JsonValue,
    /// 当前 reported 快照（沿用 `{value, time}` 结构）；无则 `{}`
    pub reported: JsonValue,
    /// 逐属性 delta（裸期望值）；空表示已收敛
    pub delta: JsonValue,
    /// desired 文档最后更新时间；无 desired 行则 null
    #[serde(with = "time::serde::rfc3339::option")]
    pub desired_updated_time: Option<OffsetDateTime>,
    /// reported 快照最后更新时间；无 reported 行则 null
    #[serde(with = "time::serde::rfc3339::option")]
    pub reported_updated_time: Option<OffsetDateTime>,
}

/// Get-Delta query parameters. Field naming follows the existing
/// `PropertyCommandQuery` convention (snake_case query keys).
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ShadowQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
}
