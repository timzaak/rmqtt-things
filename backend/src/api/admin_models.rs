use crate::db::models::{
    ActionInvocation, CertIssue, CommandSource, CommandStatus, DeviceConnectionStatus,
    DeviceStatusHistory, DeviceStatusWithSource, EventHistory, EventValidTemplate,
    EventValidTemplateStatus, PropertyChartKey, PropertyCommand, PropertyHistory, PropertyLatest,
    PropertySeriesPoint, RegistrationSource,
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
    /// 命令来源：0=OneShot（一次性写入）, 1=DesiredDelta（Target 同步）；缺省返回全部
    pub source: Option<CommandSource>,
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

// ---- Unified device operation read model ----

/// 统一操作类型。camelCase 字面值与 SQL 投影常量一致：
/// `targetSync`（property_command.source=1）/ `directPropertyWrite`（source=0）/
/// `actionInvocation`（action_invocation）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum DeviceOperationType {
    TargetSync,
    DirectPropertyWrite,
    ActionInvocation,
}

/// 统一设备操作查询入参。
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DeviceOperationQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID，可选
    pub device_id: Option<String>,
    /// 操作类型：targetSync / directPropertyWrite / actionInvocation，可选
    pub operation_type: Option<DeviceOperationType>,
    /// 命令状态：0=pending, 1=sent, 2=success, 3=failed, 4=deleted
    pub status: Option<CommandStatus>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

/// 统一设备操作视图。`operation_id` 为稳定组合 ID（`property:{id}` /
/// `action:{id}`），`name` 对属性命令固定为 `Set properties` / `Sync target`，
/// 对动作取 `service_type`（前端摘要规则依赖这些字面值）。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceOperationView {
    /// 稳定组合 ID：`property:{id}` 或 `action:{id}`
    pub operation_id: String,
    pub operation_type: DeviceOperationType,
    /// `Set properties` / `Sync target` / action 的 serviceType
    pub name: String,
    /// 属性 command 或动作 params
    pub payload: JsonValue,
    pub status: CommandStatus,
    /// 创建时间
    #[serde(with = "time::serde::rfc3339")]
    pub created_time: OffsetDateTime,
    /// 最近状态变更时间
    #[serde(with = "time::serde::rfc3339")]
    pub updated_time: OffsetDateTime,
}

/// 统一操作分页响应别名。
pub type DeviceOperationListResponse = PaginatedResponse<DeviceOperationView>;

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

// ---- Action / service invocation admin DTOs (thing-model-extension) ----
// Field naming follows the action API convention (camelCase: productId / deviceId /
// serviceType / params) and mirrors the existing `CreatePropertyCommandRequest`
// pattern. `params` defaults to `{}` when omitted（空对象允许）.

/// 管理端发起动作 / 服务调用的请求体。
/// camelCase 与前端 `ActionInvokeDialog` 表单字段一致。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateActionCommandRequest {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
    /// 服务类型标识，如 `reboot` / `unlock`；协议层不枚举（A4）
    pub service_type: String,
    /// 动作入参，透传给设备；缺省为 `{}`，空对象允许
    #[serde(default = "default_empty_object")]
    pub params: JsonValue,
}

/// `create_service_command` 的响应。
/// `status` 反映入队后的瞬时状态：`pending`（设备未订阅/离线）或 `sent`（在线
/// 且已订阅，drain 已立即投递）。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateActionCommandResponse {
    /// 新建 action_invocation.id
    pub id: i64,
    /// 初始 `pending`（设备在线且订阅时可能直接 `sent`）
    pub status: CommandStatus,
}

/// 动作调用历史查询入参。对齐 `PropertyCommandQuery`，额外携带可选
/// `service_type` 过滤（动作侧的"类型"维度）。分页沿用既有默认值
/// （`page` 默认 1、`page_size` 默认 10）。
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActionCommandQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID，可选
    pub device_id: Option<String>,
    /// 服务类型，可选
    pub service_type: Option<String>,
    /// 命令状态：0=pending, 1=sent, 2=success, 3=failed, 4=deleted
    pub status: Option<CommandStatus>,
    /// 页码，默认为1
    #[serde(default = "default_page")]
    pub page: i64,
    /// 每页大小，默认为10
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

/// 动作调用视图。字段对齐 GET /api/admin/service/command 响应
/// （`id / serviceType / params / status / createdTime / updatedTime`），
/// camelCase 与前端 hook 解包一致。复用既有 `ActionInvocation` 行模型并裁剪
/// 不暴露给前端的 `product_id` / `device_id`（已在查询参数中由调用方持有）。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActionInvocationView {
    pub id: i64,
    pub service_type: String,
    pub params: JsonValue,
    pub status: CommandStatus,
    /// 创建时间（action 入队）
    #[serde(with = "time::serde::rfc3339")]
    pub created_time: OffsetDateTime,
    /// 最后状态变更时间
    #[serde(with = "time::serde::rfc3339")]
    pub updated_time: OffsetDateTime,
}

impl From<ActionInvocation> for ActionInvocationView {
    fn from(row: ActionInvocation) -> Self {
        Self {
            id: row.id,
            service_type: row.service_type,
            params: row.params,
            status: row.status,
            created_time: row.created_time,
            updated_time: row.updated_time,
        }
    }
}

/// DELETE /api/admin/service/command 的 query 入参。对齐
/// `DeletePropertyCommandsQuery`（软删 Pending 行）。
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DeleteActionCommandsQuery {
    pub ids: Vec<i64>,
}

fn default_empty_object() -> JsonValue {
    JsonValue::Object(serde_json::Map::new())
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
pub type ActionInvocationListResponse = PaginatedResponse<ActionInvocationView>;
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

/// Query parameters for `GET /api/admin/factory/sn/{sn}/changes`
/// (paginated, time-descending). The repo has no shared `PaginationQuery`; each paginated
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

// ---- Shadow (desired state) DTOs ----

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

// ---- Property history chart DTOs ----

/// 单序列单查询返回点数上限；超限时按真实记录步长抽样并置 `downsampled=true`
pub const MAX_SERIES_POINTS: i64 = 1000;
/// 单次序列查询的属性键上限
pub const MAX_SERIES_KEYS: usize = 5;
/// 数值属性发现默认回看窗口（天），覆盖最长预设时间档
pub const DEFAULT_KEY_LOOKBACK_DAYS: i32 = 30;
/// 单个属性键的最大长度
pub const MAX_SERIES_KEY_LENGTH: usize = 128;

/// 数值属性发现查询入参（`GET /api/admin/property/history/keys`）。
/// 图表固定单设备，故 `device_id` 必填（区别于表格端点的可选 `device_id`）。
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PropertyChartKeysQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
    /// 发现窗口天数，默认 30，允许 1..=366
    pub lookback_days: Option<i32>,
}

/// 数值属性发现响应。设备从未上报时 `data` 为空数组（200），由前端渲染引导空态。
#[derive(Debug, Serialize, ToSchema)]
pub struct PropertyChartKeysResponse {
    /// 数值属性列表，按 sampleCount 降序
    pub data: Vec<PropertyChartKey>,
}

/// 图表序列查询入参（`GET /api/admin/property/history/series`）。
/// `keys` 以重复 query 参数传递（`keys=a&keys=b`）；`start_time` / `end_time`
/// 为 RFC3339，闭区间且 `end_time > start_time`。
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PropertySeriesQuery {
    /// 产品ID
    pub product_id: String,
    /// 设备ID
    pub device_id: String,
    /// 属性键列表，1..=5 个
    pub keys: Vec<String>,
    /// 起始时间（含），RFC3339
    pub start_time: String,
    /// 结束时间（含），RFC3339，必须晚于 start_time
    pub end_time: String,
}

/// 单个属性键的图表序列视图。
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PropertySeriesView {
    pub key: String,
    /// 范围内数值型匹配记录总数（count 查询结果）
    pub total_points: i64,
    /// `total_points > MAX_SERIES_POINTS` 时为 true
    pub downsampled: bool,
    /// 抽样步长（每 stride 条真实记录取第 1 条），未降精度时为 1
    pub stride: i64,
    /// 数据点，时间升序；每个点都来自真实上报记录
    pub points: Vec<PropertySeriesPoint>,
}

/// 图表序列查询响应。序列顺序与请求 `keys` 一致。
#[derive(Debug, Serialize, ToSchema)]
pub struct PropertySeriesListResponse {
    pub data: Vec<PropertySeriesView>,
}
