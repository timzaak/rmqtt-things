/**
 * Factory Metadata (产线写入 + 管理端只读读出闭环) Demo 测试
 *
 * 对应用户故事（Draft 来源：`.ai/user-stories/core/support-multiple-device.md`）：
 * - US-PA-045 分批上报子组件元数据（含乱序正常落地 + 幂等覆盖写一条 change log）
 * - US-PA-046 上报设备-子组件关联（异步组装 + 一设备多子组件）
 * - US-PA-047 管理员查询设备出厂元数据与子组件清单（完整视图 + 部分到达 + 未上报 404 边界）
 *
 * 覆盖映射：
 * - Scenario A -> US-PA-046 场景 1 (关联/元数据异步到达组装) + US-PA-047 场景 1/2 (完整 + 部分到达)
 * - Scenario B -> US-PA-045 场景 3 (同 SN 重复上报幂等覆盖写一条 change log)
 * - Scenario C -> US-PA-046 场景 2 (一设备多子组件) + US-PA-047 (未上报 404 边界)
 *
 * 验收面：仅 HTTP API（factory PUT Bearer + admin GET cookie），**不**进入设备详情
 * UI 正文断言（FE-A01 已在前端阶段独立验收 UI）。
 *
 * 关键断言均落在持久业务状态（admin GET 返回的合并视图 / change log），**不**以
 * toast/sonner 为唯一验收依据。
 *
 * 失败归因（参见 DE-D01 item）：
 * - Scenario A/C 失败优先归因后端 left-join / 404 边界（BE-D01/D03）；
 * - Scenario B 失败优先归因后端 change_log 写入或幂等覆盖（BE-D01/D02）；
 * - 纯测试侧问题（imports / helper 签名 / selector 字面量）由 demo-dev 自治修复。
 *
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 * 前置条件：后端 `[factory] api_keys` 含 `FACTORY_API_KEY`
 *           （fallback `factory-api-key-please-change`，见
 *           `backend/config.example.toml` 注释示例）。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { getJson } from './helpers/api'
import { upsertComponent, replaceAssociations } from './helpers/factory-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

/**
 * admin GET 轮询超时（对齐 `device-shadow-demo.e2e.ts` 的 `POLL_TIMEOUT`）。
 * factory 写入在 admin 库为同步可见（同一 backend 进程 / 同一 DB），15s 余量
 * 覆盖 CI 慢机 / DB 连接抖动。
 */
const POLL_TIMEOUT = 15_000

/**
 * admin 设备合并视图（设计 §4.2.2 C）。
 *
 * 字段命名对齐 `backend/src/api/factory_handlers.rs::FactoryDeviceView`：
 * `device_metadata` 字段在 Rust 端用 `#[serde(rename = "deviceMetadata")]`，
 * 其余字段（`deviceSn`、`components`）亦为 camelCase。
 */
interface FactoryDeviceView {
  deviceSn: string
  /** 设备级元数据（本轮保留，始终为 null —— 见 FactoryDeviceView 文档）。 */
  deviceMetadata: Record<string, unknown> | null
  components: FactoryComponentView[]
}

/**
 * admin 设备合并视图中的单个子组件（设计 §4.2.2 C）。
 *
 * 字段命名对齐 `backend/src/api/factory_handlers.rs::FactoryComponentView`：
 * `component_sn` → `componentSn`、`component_type` → `componentType`、
 * `file_attachments` → `fileAttachments`、`updated_at` → `updatedAt`
 * （均经 `#[serde(rename = ...)]`）。`metadata`/`componentType`/`updatedAt` 在
 * 子组件元数据尚未到达时为 `null`；`fileAttachments` 在该情况下回落为 `[]`。
 */
interface FactoryComponentView {
  componentSn: string
  componentType: string | null
  metadata: Record<string, unknown> | null
  fileAttachments: unknown[]
  /** RFC3339 字符串；元数据未到达时为 null。 */
  updatedAt: string | null
}

/**
 * change log 行（设计 §4.2.2 D + §4.3.2）。
 *
 * 字段命名对齐 `backend/src/db/models.rs::FactoryMetadataChangeLog`：snake_case
 * 原样输出（`#[serde(with = "time::serde::rfc3339")]` 只影响时间格式，不影响
 * 字段名）。`before` 在首次上报时为 null（`UpsertOutcome::Created`）；覆盖时
 * 为非 null 快照。
 *
 * 注意：`before`/`after` 是子组件元数据行的 JSONB 快照（见
 * `backend/src/db/factory_metadata.rs::upsert_component`），其结构为
 * `{ component_type, metadata, file_attachments, updated_at }`（snake_case），
 * **不是**扁平的 `{ calibration: ... }`。因此断言 `after.metadata.calibration`
 * 而非 `after.calibration`。
 */
interface FactoryChangeLogRow {
  id: number
  component_sn: string
  /** 上一次的子组件元数据行快照（snake_case 嵌套对象）；首次上报为 null。 */
  before: {
    component_type: string
    metadata: Record<string, unknown>
    file_attachments: unknown[]
    updated_at: string
  } | null
  /** 当前写入后的子组件元数据行快照（snake_case 嵌套对象）。 */
  after: {
    component_type: string
    metadata: Record<string, unknown>
    file_attachments: unknown[]
    updated_at: string
  }
  /** 写入归因 label：factory 写路径固定为 `"factory"`。 */
  actor: string
  /** RFC3339 字符串。 */
  created_at: string
}

/**
 * change log 分页响应（设计 §4.2.2 D）。
 *
 * 实测 shape（已对照 `backend/src/api/admin_models.rs::PaginatedResponse` +
 * `factory_handlers.rs::query_component_changes_handler`）：嵌套 `{ data, pagination }`，
 * 非扁平 `{ items, total, page, page_size }`。`page`/`page_size`/`total` 为 i64。
 */
interface PaginatedChangeLog {
  data: FactoryChangeLogRow[]
  pagination: {
    page: number
    page_size: number
    total: number
  }
}

test.describe('Factory Metadata (US-PA-045/046/047)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  // ---------------------------------------------------------------------------
  // Scenario A — 乱序到达：先报设备-子组件关联、后报子组件元数据
  //   US-PA-046 场景 1（关联/元数据异步到达后可被组装）
  //   US-PA-047 场景 1（查询设备子组件清单与元数据）
  //   US-PA-047 场景 2（部分数据未到达时仍可查询，不报错、不阻塞）
  // ---------------------------------------------------------------------------
  test('[Scenario A] US-PA-046 out-of-order arrival: associations first, metadata later, left-join returns partial then full view', async ({
    request,
    demoLogger: _demoLogger,
  }) => {
    const deviceSn = `e2e-factory-dev-${Date.now()}`
    const camSn = `e2e-factory-cam-${Date.now()}`

    // Step 1: 先报关联（子组件元数据尚未到达）。
    const assocResp = await replaceAssociations(request, deviceSn, [
      { componentSn: camSn },
    ])
    expect(assocResp.status()).toBe(204)

    // Step 2: 此时元数据未到 —— admin GET 应返回 200（US-PA-047 场景 2：
    // 部分数据未到达时不报错、不阻塞），left-join 返回当前存在的部分：
    // 关联存在但 metadata/fileAttachments/updatedAt 为 null/空。
    await expect.poll(
      async () => {
        const body = await getJson<FactoryDeviceView>(
          request,
          `/api/admin/factory/devices/${deviceSn}`,
        )
        return {
          componentSn: body.components[0]?.componentSn,
          metadata: body.components[0]?.metadata,
          fileAttachments: body.components[0]?.fileAttachments,
          updatedAt: body.components[0]?.updatedAt,
        }
      },
      { timeout: POLL_TIMEOUT },
    ).toEqual({
      componentSn: camSn,
      metadata: null,
      fileAttachments: [],
      updatedAt: null,
    })

    // Step 3: 后报子组件元数据（乱序正常落地）。
    const upsertResp = await upsertComponent(request, camSn, {
      componentType: 'camera',
      metadata: { calibration: 42 },
    })
    expect(upsertResp.status()).toBe(204)

    // Step 4: 复查同一 GET —— 此时组装完成（US-PA-046 场景 1 + US-PA-047 场景 1）。
    await expect.poll(
      async () => {
        const body = await getJson<FactoryDeviceView>(
          request,
          `/api/admin/factory/devices/${deviceSn}`,
        )
        return {
          componentSn: body.components[0]?.componentSn,
          calibration: (body.components[0]?.metadata as { calibration?: unknown })
            ?.calibration,
        }
      },
      { timeout: POLL_TIMEOUT },
    ).toEqual({ componentSn: camSn, calibration: 42 })
  })

  // ---------------------------------------------------------------------------
  // Scenario B — 幂等覆盖写一条 change log（同 SN 重复 PUT）
  //   US-PA-045 场景 3（同 SN 重复上报，平台覆盖并写一条变更日志）
  // ---------------------------------------------------------------------------
  test('[Scenario B] US-PA-045 idempotent overwrite writes a change log with before/after', async ({
    request,
    demoLogger: _demoLogger,
  }) => {
    const camSn = `e2e-factory-cam-${Date.now()}`

    // Step 1: 首次上报（before=null，仅 Created，**不**写 change log）。
    const firstResp = await upsertComponent(request, camSn, {
      metadata: { calibration: 1 },
    })
    expect(firstResp.status()).toBe(204)

    // Step 2: 同 SN 再次上报（覆盖，写一条 before=旧值 的 change log）。
    const overwriteResp = await upsertComponent(request, camSn, {
      metadata: { calibration: 2 },
    })
    expect(overwriteResp.status()).toBe(204)

    // Step 3: 查询 change log —— 实测 shape 为 `{ data, pagination }`（非扁平
    // `{items,total,page,page_size}`），行字段 snake_case。
    await expect.poll(
      async () => {
        const body = await getJson<PaginatedChangeLog>(
          request,
          `/api/admin/factory/components/${camSn}/changes?page=1&page_size=20`,
        )
        return {
          // shape 守卫：响应结构必须为 { data, pagination }，pagination 必须含
          // page/page_size/total（实测差异：非扁平 `{items,total,page,page_size}`）。
          hasData: Array.isArray(body.data),
          paginationKeys: Object.keys(body.pagination ?? {}).sort(),
          pagination: body.pagination,
          total: body.pagination?.total,
        }
      },
      { timeout: POLL_TIMEOUT },
    ).toEqual({
      hasData: true,
      paginationKeys: ['page', 'page_size', 'total'],
      pagination: { page: 1, page_size: 20, total: 1 },
      total: 1,
    })

    // Step 4: 行字段与 before/after 快照内容断言。
    // 首次 Created 不写 change log，覆盖后只有 1 条 change log 行：
    //   before = { metadata: { calibration: 1 }, ... }
    //   after  = { metadata: { calibration: 2 }, ... }
    // actor 固定为 "factory"（factory_middleware 的 FactoryCaller label）。
    const logBody = await getJson<PaginatedChangeLog>(
      request,
      `/api/admin/factory/components/${camSn}/changes?page=1&page_size=20`,
    )
    expect(logBody.data.length).toBeGreaterThanOrEqual(1)

    const row = logBody.data[0]
    expect(row.component_sn).toBe(camSn)
    expect(row.actor).toBe('factory')
    // before/after 是子组件元数据行的快照（snake_case 嵌套对象），断言走
    // .metadata.calibration 路径而非顶层 calibration。
    expect((row.before?.metadata as { calibration?: unknown })?.calibration).toBe(1)
    expect((row.after.metadata as { calibration?: unknown })?.calibration).toBe(2)
  })

  // ---------------------------------------------------------------------------
  // Scenario C — 一设备多子组件 + 无任何数据返回 404
  //   US-PA-046 场景 2（一个设备关联多个子组件，查询时看到全部清单）
  //   US-PA-047（未上报设备 GET 返回 404，区分「未上报」与「部分到达」）
  // ---------------------------------------------------------------------------
  test('[Scenario C] US-PA-046 multiple sub-components per device + never-reported device returns 404', async ({
    request,
    demoLogger: _demoLogger,
  }) => {
    const deviceSn = `e2e-factory-dev-${Date.now()}`
    const camSn = `e2e-factory-cam-${Date.now()}`
    const cam2Sn = `e2e-factory-cam2-${Date.now()}`

    // Step 1: 一设备关联两个子组件（full-replace）。
    const assocResp = await replaceAssociations(request, deviceSn, [
      { componentSn: camSn },
      { componentSn: cam2Sn },
    ])
    expect(assocResp.status()).toBe(204)

    // Step 2: 两个子组件分别 upsert 元数据。
    const camResp = await upsertComponent(request, camSn, {
      componentType: 'camera',
      metadata: { calibration: 100 },
    })
    expect(camResp.status()).toBe(204)
    const cam2Resp = await upsertComponent(request, cam2Sn, {
      componentType: 'camera',
      metadata: { calibration: 200 },
    })
    expect(cam2Resp.status()).toBe(204)

    // Step 3: admin GET —— components.length === 2，两个 componentSn 均可见。
    await expect.poll(
      async () => {
        const body = await getJson<FactoryDeviceView>(
          request,
          `/api/admin/factory/devices/${deviceSn}`,
        )
        const sns = body.components.map((c) => c.componentSn).sort()
        return {
          count: body.components.length,
          sns,
          calibrations: sns.map((sn) => {
            const c = body.components.find((x) => x.componentSn === sn)
            return (c?.metadata as { calibration?: unknown })?.calibration
          }),
        }
      },
      { timeout: POLL_TIMEOUT },
    ).toEqual({
      count: 2,
      sns: [camSn, cam2Sn].sort(),
      calibrations: [100, 200],
    })

    // Step 4: 从未上报任何关联/元数据的设备 SN —— 严格 404（区分「未上报」
    // 与「部分到达」，后者在 Scenario A 已验证返回 200 + 部分字段）。
    const emptyDeviceSn = `e2e-factory-empty-${Date.now()}`
    const emptyResp = await request.get(
      `/api/admin/factory/devices/${emptyDeviceSn}`,
    )
    expect(emptyResp.status()).toBe(404)
  })
})
