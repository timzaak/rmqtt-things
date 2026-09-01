/**
 * Action Invocation Demo 测试
 *
 * 对应用户故事（docs/user-stories/01-platform-admin-user-stories.md）：
 * - US-PA-048 调用设备动作 / 服务（场景 1 在线回环 / 场景 2 离线排队上线投递 /
 *   场景 3 不污染影子）
 * - US-PA-049 在前端区分动作调用与属性下发（独立动作 Tab + 历史）
 *
 * 传输层复用故事：US-PA-016（下发命令）、US-DV-009（离线命令排队与上线投递）。
 *
 * 协议契约：
 * - 平台下发：`{id: "action:{db_id}", params, ack: 1}`，topic
 *   `{productId}/{deviceId}/thing/service/{serviceType}/set`。
 * - 设备回复：`{id, data?, code}`，topic `.../set_reply`；任意 2xx 为 Success。
 *
 * 关键断言均落在持久业务状态（GET /api/admin/service/command 的 status、
 * GET /api/admin/property/shadow 的 desired/reported、动作历史表行），
 * 不以 sonner/toast 为唯一验收依据。
 *
 * device-detail-experience 七区信息架构适配：旧 ActionInvocationsSection
 * 已删除，动作调用历史汇入统一 Operations 表（DeviceOperationsSection.tsx），
 * 调用入口收敛到 Operations tab 的 Run Action 按钮（ActionInvokeDialog.tsx）。
 * 变化点：
 * - openActionsTab → openOperationsTab：点击 device-tab-operations
 * - 分区 heading "Action Invocations" → "Operations"（动作行用 type 列 "Action" 筛选）
 * - 旧 action-invoke-button → run-action-button（SELECTORS.operations.runActionButton）
 * - 对话框 testid 保留（ActionInvokeDialog.tsx 未删）：action-invoke-dialog /
 *   service-type-input / params-input / submit-button / cancel-button，统一改用
 *   SELECTORS.operations.* 引用，禁止硬编码。
 * 后端 API（POST /api/admin/service/command 创建、GET 状态、shadow desired）行为不变。
 *
 * 前置条件：系统中已有产品 "demo_product"。
 * 前置条件：RMQTT broker 运行在 MQTT_URL (默认 mqtt://127.0.0.1:1883)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 * 前置条件：前端运行在 FRONTEND_URL (默认 http://localhost:3000)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { getJson } from './helpers/api'
import { findSeedProductId, getProduct, updateProduct } from './helpers/product-api'
import { verifyTestEnvironment } from './helpers/environment-setup'
import { SELECTORS } from './selectors'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 15_000

interface ListResponse<T> {
  data?: T[]
}

interface ActionInvocationRow {
  id: number
  serviceType?: string
  params?: Record<string, unknown>
  status?: string | number
}

interface PropertyCommandRow {
  id: number
  status?: string | number
  command?: unknown
}

interface ShadowView {
  desired?: Record<string, unknown>
  reported?: Record<string, unknown>
  delta?: Record<string, unknown>
}

/**
 * 导航到设备详情页并切换到 Operations tab。
 *
 * 设备详情页默认 activeTab='overview'，DeviceOperationsSection 仅在
 * activeTab==='operations' 时渲染（show.$id.tsx:126-128）。Tab 由 data-testid
 * `device-tab-operations` 定位（优先用 testid），分区 heading "Operations" 为
 * 持久锚点。Tab 选择器集中在 SELECTORS.deviceTabs.operationsTab。
 */
async function openOperationsTab(
  page: import('@playwright/test').Page,
  deviceId: string,
): Promise<void> {
  // waitUntil:'commit' 在收到响应头即返回，避免 Vite dev server 下
  // 'load'/'domcontentloaded' 偶发不触发导致 30s 超时。
  await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`, {
    waitUntil: 'commit',
  })
  await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  await page.locator(SELECTORS.deviceTabs.operationsTab).click()
  await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
}

test.describe('Action Invocation (US-PA-048 / US-PA-049)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  // ---------------------------------------------------------------------------
  // Scenario 1 — US-PA-048 在线设备动作回环（含非 200 的 2xx 成功语义）
  // ---------------------------------------------------------------------------
  test('[US-PA-048 Scenario 1] online device action round-trip via UI', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 全新 deviceId 需要 auto_provisioning 才能通过 MQTT 认证（与 shadow-demo 对齐）
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-action-online-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()
      // 订阅 reboot 动作主题：订阅 webhook 触发离线投递，在线时即建立投递通道
      await device.subscribeAction('reboot')

      await openOperationsTab(page, deviceId)

      // 打开 Invoke Action 对话框（入口为 Run Action 按钮，禁止硬编码 testid）
      await page.locator(SELECTORS.operations.runActionButton).click()
      const dialog = page.locator(SELECTORS.operations.actionDialog)
      await expect(dialog).toBeVisible()

      const params = { delaySeconds: 5 }
      await dialog.locator(SELECTORS.operations.actionServiceTypeInput).fill('reboot')
      await dialog.locator(SELECTORS.operations.actionParamsInput).fill(JSON.stringify(params))

      // 在 submit 前挂 waitForAction，避免 eager publish-on-subscribe / 立即 drain 的竞态
      const actionPromise = device.waitForAction()
      await dialog.locator(SELECTORS.operations.dialogSubmitButton).click()

      // 设备收到标准动作 envelope：id 形如 action:{db_id}，params 透传
      const action = await actionPromise
      expect(action.id).toEqual(expect.stringMatching(/^action:\d+$/))
      expect(action.params).toMatchObject(params)
      // ack=1 是 spec 请求格式约定（设备必须回复 set_reply）
      expect((action.raw as { ack?: number }).ack).toBe(1)

      // 回复 202（顺带覆盖非 200 的 2xx 成功语义）
      await device.replyAction(action, 202)

      // 主断言（持久状态）：统一 Operations 表内该动作调用 Success 行可见
      // （device-operations-table 取代旧 action-invocation-table）。
      const operationsTable = page.locator(SELECTORS.operations.table)
      await expect(operationsTable.getByText('Success', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })

      // 交叉验证持久业务状态（API）
      await expect.poll(
        async () => {
          const body = await getJson<ListResponse<ActionInvocationRow>>(
            request,
            `/api/admin/service/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return body.data?.find((row) => row.serviceType === 'reboot')?.status
        },
        { timeout: POLL_TIMEOUT },
      ).toBe('Success')
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 2 — US-PA-048 离线排队、上线投递（US-DV-009 传输层）
  // ---------------------------------------------------------------------------
  test('[US-PA-048 Scenario 2] offline action queued, delivered on connect', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-action-offline-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
    const serviceType = 'unlock'
    const params = { token: 'abc123' }

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      // 先 connect 再 disconnect 注册设备（参考 property-command-demo Scenario 2）
      await device.connect()
      await device.disconnect()

      // 离线状态下通过 API 创建动作调用（body camelCase）
      const createResponse = await request.post('/api/admin/service/command', {
        data: {
          productId: PRODUCT_ID,
          deviceId,
          serviceType,
          params,
        },
      })
      expect(createResponse.status()).toBe(201)

      // 通过 API 断言存在 Pending 调用（持久状态）
      await expect.poll(
        async () => {
          const body = await getJson<ListResponse<ActionInvocationRow>>(
            request,
            `/api/admin/service/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return body.data?.find((row) => row.serviceType === serviceType)?.status
        },
        { timeout: POLL_TIMEOUT },
      ).toBe('Pending')

      // 设备重连：RMQTT auto-subscription 规则（rmqtt-auto-subscription.toml）
      // 会在 connect 握手期间自动订阅 `+/${deviceId}/thing/service/+/set`，
      // 加上 DemoMqttDevice.connect() 内部订阅 property/set，都会命中
      // `client_subscribe` webhook -> `service_set_subscribe` handler，
      // 该 handler 原子 drain 所有 Pending 动作调用并立即发布（handlers.rs:634）。
      // 因此 drain 发生在 connect() 返回之前：必须在 connect 之前就挂上
      // waitForAction，否则消息到达时 waiter 还没注册而被丢弃。
      // （waitForAction 只往 actionWaiters 数组 push 一个 Promise resolver，
      // 不依赖 MQTT 连接，与 mqtt-device-flow-demo US-DV-009 同样的处理。）
      //
      // 订阅竞态修复（方案 1）：将 serviceType 传给 connect()，
      // 使其返回前显式订阅 `thing/service/${serviceType}/set` 并 await SUBACK，
      // 同时记入 subscribedActionTopics。这样 broker drain 投递时设备已有确认的
      // 订阅，且 message handler 的 action 分支能正确 dispatch（否则集合为空时
      // 即使消息通过 wildcard 投递到达也会被丢弃）。在线场景的 subscribeAction()
      // 调用顺序不受影响（本场景已通过 connect 预订阅，不再二次 subscribeAction）。
      const actionPromise = device.waitForAction()
      await device.connect({ serviceTypes: [serviceType] })
      try {
        // subscribedActionTopics 已由 connect() 预订阅填充，replyAction 的
        // resolveServiceType 可解析 serviceType，无需再 subscribeAction。

        const action = await actionPromise
        expect(action.id).toEqual(expect.stringMatching(/^action:\d+$/))
        expect(action.params).toMatchObject(params)

        // 回复 200，状态收敛为 Success
        await device.replyAction(action, 200)

        await expect.poll(
          async () => {
            const body = await getJson<ListResponse<ActionInvocationRow>>(
              request,
              `/api/admin/service/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
            )
            return body.data?.find((row) => row.serviceType === serviceType)?.status
          },
          { timeout: POLL_TIMEOUT },
        ).toBe('Success')

        // 辅助 UI 断言（非主验收）：统一 Operations 表可见该调用的 Success 行
        await openOperationsTab(page, deviceId)
        const operationsTable = page.locator(SELECTORS.operations.table)
        await expect(operationsTable.getByText('Success', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })
      } finally {
        await device.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 3 + US-PA-049 — 不污染影子且历史可区分
  // ---------------------------------------------------------------------------
  test('[US-PA-048 Scenario 3 + US-PA-049] action does not pollute shadow and is isolated from property commands', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-action-shadow-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 通过 shadow desired API 写入 desired，记录快照
      const desiredPayload = { brightness: 42 }
      // 在 PUT 前注册 command waiter，避免 eager publish-on-trigger 丢失 delta 命令
      const shadowCommandPromise = device.waitForCommand()
      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)

      // 设备回复 delta 命令，使其收敛为 Success（desired 落库）
      const shadowCommand = await shadowCommandPromise
      expect(shadowCommand.params).toMatchObject(desiredPayload)
      await device.replyCommand(shadowCommand)

      // 记录 shadow 快照（desired / reported）
      const snapshot = await getJson<ShadowView>(
        request,
        `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
      )

      // 订阅动作主题并完成一次动作调用（API 通道，与场景 1 区分入口）
      await device.subscribeAction('buzzer')
      const actionParams = { duration: 2 }
      const actionPromise = device.waitForAction()
      const createActionResponse = await request.post('/api/admin/service/command', {
        data: {
          productId: PRODUCT_ID,
          deviceId,
          serviceType: 'buzzer',
          params: actionParams,
        },
      })
      expect(createActionResponse.status()).toBe(201)

      const action = await actionPromise
      expect(action.id).toEqual(expect.stringMatching(/^action:\d+$/))
      expect(action.params).toMatchObject(actionParams)
      await device.replyAction(action, 200)

      // 主断言 1（持久状态）：动作调用完成后 shadow desired/reported 与快照一致（无污染）
      await expect.poll(
        async () => {
          const after = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return {
            desired: after.desired ?? {},
            reported: after.reported ?? {},
          }
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual({
        desired: snapshot.desired ?? {},
        reported: snapshot.reported ?? {},
      })

      // 主断言 2（数据层隔离，A2）：动作调用不出现在 property_command 列表
      const propertyCommands = await getJson<ListResponse<PropertyCommandRow>>(
        request,
        `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=50`,
      )
      // 该设备只产生过 shadow DesiredDelta 命令，不应有任何 service_type=buzzer 的动作行
      // （property_command 表本身不存 service_type，故只需确认 property 行数等于
      // shadow delta 产生的 1 行，未混入动作调用）。
      const propertyRowCount = propertyCommands.data?.length ?? 0
      expect(propertyRowCount).toBe(1)

      // 主断言 3（数据层隔离反向）：动作调用历史包含 buzzer 行
      await expect.poll(
        async () => {
          const body = await getJson<ListResponse<ActionInvocationRow>>(
            request,
            `/api/admin/service/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return body.data?.some((row) => row.serviceType === 'buzzer') ?? false
        },
        { timeout: POLL_TIMEOUT },
      ).toBe(true)

      // 主断言 4（US-PA-049 前端可区分）：无独立动作 Tab，故在统一
      // Operations 表内按 Type 列筛选 "Action" 验证动作调用可区分：buzzer 调用
      // 作为 Action 类型行可见。属性命令区（同表内 Direct write / Target sync 类型）
      // 不出现该动作调用（More ▾ → Direct property write 仍走
      // property_command 表，与 action_invocation 物理隔离）。
      await openOperationsTab(page, deviceId)
      const operationsTable = page.locator(SELECTORS.operations.table)

      // 本场景同时产生 targetSync（desired 写入）和 actionInvocation
      // （buzzer）两类 Success 行，统一表中 getByText('Success') 会触发 strict mode
      // violation。本断言的意图是验证 buzzer 动作调用作为 Action 类型行可见且成功，
      // 因此限定到含 'buzzer' 的行，再断言其 Type=Action 与 Status=Success。
      const buzzerRow = operationsTable.locator('tr', { hasText: 'buzzer' })
      await expect(buzzerRow).toBeVisible({ timeout: POLL_TIMEOUT })
      await expect(buzzerRow.getByText('Action', { exact: true })).toBeVisible()
      await expect(buzzerRow.getByText('Success', { exact: true })).toBeVisible()
      // 动作调用 name 列（database.rs:478 service_type AS name）：buzzer 可见
      await expect(operationsTable.getByText('buzzer')).toBeVisible()

      // 反向隔离：按类型筛选 Direct write，buzzer 动作行应不再出现在筛选后的表中
      // （operation-type-filter 的 directPropertyWrite option value）。
      await page.locator(SELECTORS.operations.typeFilter).selectOption('directPropertyWrite')
      await expect(operationsTable.getByText('buzzer')).toHaveCount(0)
      await expect(operationsTable.getByText('Action', { exact: true })).toHaveCount(0)
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
