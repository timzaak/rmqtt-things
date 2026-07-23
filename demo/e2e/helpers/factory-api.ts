/**
 * Factory (production-line) API 鉴权薄封装 (support-multiple-device feature,
 * design §4.2.2 A/B + §5.2).
 *
 * 这些 helper 对接 `backend/src/api/factory_handlers.rs` 暴露的 factory 写入
 * 端点。factory 写路径运行在 `factory_auth_middleware` 之后（见
 * `backend/src/api/factory_middleware.rs`），与既有的 Herald OAuth / HMAC 设备
 * 证书 / 内网 IP 白名单三套认证机制完全隔离：请求必须携带
 * `Authorization: Bearer <key>`，且 key 必须出现在后端 `[factory] api_keys`
 * 配置项中（空配置拒绝所有请求，返回 401）。
 *
 * 设计选择：
 * - 与 `helpers/api.ts`（管理端 cookie 鉴权 GET）解耦：factory 写请求走独立的
 *   `request` 参数（不携带 admin cookie），由调用方在 `data-testid`/UI 流程外
 *   通过 Playwright `request` fixture 直接发起。
 * - helper **不**调用 `logger.finalize()`；日志由统一 fixture 的 `demoLogger`
 *   接入（参见 `fixtures/demo-auth.fixtures.ts` 的 demoLogger fixture）。
 * - 不在此处断言响应状态：返回原始 `APIResponse`，由测试侧根据场景（204 / 400 /
 *   401）自行 `expect(...).toBe(...)`，以保留失败归因面的精确性。
 */

import type { APIRequestContext, APIResponse } from '@playwright/test'

/**
 * Factory API Key，环境变量优先，fallback `factory-api-key-please-change`
 * （与 `backend/config.example.toml` 注释示例一致）。
 *
 * 集成环境（accept slot）启动时，须确保后端 `[factory] api_keys` 列表含此 key；
 * 否则所有 `/api/factory/*` 请求会被 `factory_auth_middleware` 以 401 拒绝
 * （详见 `backend/src/api/factory_middleware.rs` 的 `empty_key_list_rejects_everything` 测试）。
 */
export const FACTORY_API_KEY =
  process.env.FACTORY_API_KEY ?? 'factory-api-key-please-change'

/** 共享 Bearer 鉴权头。所有 factory 写请求都必须携带。 */
function factoryAuthHeaders(): Record<string, string> {
  return { Authorization: `Bearer ${FACTORY_API_KEY}` }
}

/**
 * `upsertComponent` body（设计 §4.2.2 A）。
 *
 * 三个字段全部可选（后端在缺省时分别替换为 `componentType="camera"`、
 * `metadata={}`、`fileAttachments=[]`）。一个完全空的请求会创建一条空占位行
 * —— 这是被允许的，调用方可以先 upsert 关联、再 upsert 元数据（见 Scenario A
 * 的乱序到达）。
 */
export interface UpsertComponentBody {
  /** 自由文本组件类型（缺省 `"camera"`）。 */
  componentType?: string
  /** 结构化元数据（标定值等）。缺省 `{}`。 */
  metadata?: Record<string, unknown>
  /** 文件附件引用。每个 `fileKey` 必须先经 `POST /api/factory/file/upload` 取得。 */
  fileAttachments?: Array<{
    fileKey: string
    fileName: string
    contentType?: string
    sizeBytes?: number
  }>
}

/**
 * PUT `/api/factory/components/{componentSn}` — upsert 子组件元数据。
 *
 * 设计 §4.2.2 A + §5.1：repo 层在 upsert 发生覆盖时于同一事务内写一条
 * `factory_metadata_change_log`（R5）。后端响应为 **204 No Content**（无 body）；
 * 调用方应直接断言 `response.status() === 204`。
 *
 * @param request Playwright `APIRequestContext`（factory 写路径不应携带 admin cookie）。
 * @param componentSn 子组件 SN（与设备 SN 同字符集，后端用 `validate_identifier` 校验）。
 * @param body 三字段均可选的 upsert 体。
 * @returns 原始 `APIResponse`，由调用方断言状态码。
 */
export async function upsertComponent(
  request: APIRequestContext,
  componentSn: string,
  body: UpsertComponentBody,
): Promise<APIResponse> {
  return request.put(`/api/factory/components/${componentSn}`, {
    headers: factoryAuthHeaders(),
    data: body,
  })
}

/** `replaceAssociations` 中的单个子组件项。 */
export interface ComponentAssociationItem {
  /** 子组件 SN（与设备 SN 同字符集）。 */
  componentSn: string
  /** 可选类型提示；合并视图里元数据表的值优先（设计 §4.2.2 C）。 */
  componentType?: string
}

/**
 * PUT `/api/factory/devices/{deviceSn}/components` — 全量替换设备的子组件关联。
 *
 * 设计 §4.2.2 B：**full-replace** 语义——未出现在 `components` 列表里的关联会被
 * 删除；内容完全相同的重复提交是幂等的（设计 §6.1
 * `replace_associations_full_replace_is_idempotent`）。该端点 **不**写 change log
 * （R5 将日志范围限定在子组件元数据覆盖上）。后端响应为 **204 No Content**。
 *
 * @param request Playwright `APIRequestContext`（factory 写路径不应携带 admin cookie）。
 * @param deviceSn 设备 SN（与 MQTT client_id 同命名空间）。
 * @param components 子组件列表（full-replace）。
 * @returns 原始 `APIResponse`，由调用方断言状态码。
 */
export async function replaceAssociations(
  request: APIRequestContext,
  deviceSn: string,
  components: ComponentAssociationItem[],
): Promise<APIResponse> {
  return request.put(`/api/factory/devices/${deviceSn}/components`, {
    headers: factoryAuthHeaders(),
    data: { components },
  })
}
