/**
 * Action Invocation Demo 测试
 *
 * 对应用户故事（Draft 来源：`.ai/user-stories/core/thing-model-extension.md`）：
 * - US-TME-002 调用设备动作 / 服务（场景 1 在线回环 / 场景 2 离线排队上线投递 /
 *   场景 3 不污染影子）
 * - US-TME-003 在前端区分动作调用与属性下发（独立动作 Tab + 历史）
 *
 * 传输层复用故事：US-PA-016（下发命令）、US-DV-009（离线命令排队与上线投递）。
 *
 * 协议契约（thing-model-extension 设计 §5.1 / §5.2）：
 * - 平台下发：`{id: "action:{db_id}", params, ack: 1}`，topic
 *   `{productId}/{deviceId}/thing/service/{serviceType}/set`。
 * - 设备回复：`{id, data?, code}`，topic `.../set_reply`；任意 2xx 为 Success。
 *
 * 关键断言均落在持久业务状态（GET /api/admin/service/command 的 status、
 * GET /api/admin/property/shadow 的 desired/reported、动作历史表行），
 * 不以 sonner/toast 为唯一验收依据。
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

/** Navigate to the device detail page and open the Actions tab. */
async function openActionsTab(page: import('@playwright/test').Page, deviceId: string): Promise<void> {
  await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
  await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  // The Actions tab is rendered alongside Commands / Shadow (show.$id.tsx TABS).
  // Tab labels centralized in SELECTORS.deviceTabs (DE-D04).
  await page.getByRole('tab', { name: SELECTORS.deviceTabs.actionsTab }).click()
  // The section heading is the persistent anchor for the action history table.
  await expect(page.getByRole('heading', { name: 'Action Invocations' })).toBeVisible()
}

test.describe('Action Invocation (US-TME-002 / US-TME-003)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  // ---------------------------------------------------------------------------
  // Scenario 1 — US-TME-002 在线设备动作回环（含非 200 的 2xx 成功语义）
  // ---------------------------------------------------------------------------
  test('[US-TME-002 Scenario 1] online device action round-trip via UI', async ({
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

      await openActionsTab(page, deviceId)

      // 打开 Invoke Action 对话框（SELECTORS.actions.*，禁止硬编码 testid）
      await page.locator(SELECTORS.actions.invokeButton).click()
      const dialog = page.locator(SELECTORS.actions.invokeDialog)
      await expect(dialog).toBeVisible()

      const params = { delaySeconds: 5 }
      await dialog.locator(SELECTORS.actions.serviceTypeInput).fill('reboot')
      await dialog.locator(SELECTORS.actions.paramsInput).fill(JSON.stringify(params))

      // 在 submit 前挂 waitForAction，避免 eager publish-on-subscribe / 立即 drain 的竞态
      const actionPromise = device.waitForAction()
      await dialog.locator(SELECTORS.actions.submitButton).click()

      // 设备收到标准动作 envelope：id 形如 action:{db_id}，params 透传
      const action = await actionPromise
      expect(action.id).toEqual(expect.stringMatching(/^action:\d+$/))
      expect(action.params).toMatchObject(params)
      // ack=1 是 spec 请求格式约定（设备必须回复 set_reply）
      expect((action.raw as { ack?: number }).ack).toBe(1)

      // 回复 202（顺带覆盖非 200 的 2xx 成功语义，设计 §5.1）
      await device.replyAction(action, 202)

      // 主断言（持久状态）：动作历史表出现该调用且状态为 Success
      const actionsSection = page
        .locator('section')
        .filter({ has: page.getByRole('heading', { name: 'Action Invocations' }) })
      await expect(actionsSection.getByText('Success')).toBeVisible({ timeout: POLL_TIMEOUT })

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
  // Scenario 2 — US-TME-002 离线排队、上线投递（US-DV-009 传输层）
  // ---------------------------------------------------------------------------
  test('[US-TME-002 Scenario 2] offline action queued, delivered on connect', async ({
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

      // 离线状态下通过 API 创建动作调用（body camelCase，设计 §4.2.2）
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
      // 订阅竞态修复（DE-D05 缺陷 C 方案 1）：将 serviceType 传给 connect()，
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

        // 辅助 UI 断言：动作 Tab 的历史表可见该调用（非主验收）
        await openActionsTab(page, deviceId)
        const actionsSection = page
          .locator('section')
          .filter({ has: page.getByRole('heading', { name: 'Action Invocations' }) })
        await expect(actionsSection.getByText('Success')).toBeVisible({ timeout: POLL_TIMEOUT })
      } finally {
        await device.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 3 + US-TME-003 — 不污染影子且历史可区分
  // ---------------------------------------------------------------------------
  test('[US-TME-002 Scenario 3 + US-TME-003] action does not pollute shadow and is isolated from property commands', async ({
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

      // 主断言 4（US-TME-003 前端可区分）：动作 Tab 历史表可见该调用，
      // 属性命令区不出现该动作调用。
      await openActionsTab(page, deviceId)
      const actionsSection = page
        .locator('section')
        .filter({ has: page.getByRole('heading', { name: 'Action Invocations' }) })
      await expect(actionsSection.getByText('Success')).toBeVisible({ timeout: POLL_TIMEOUT })
      await expect(actionsSection.getByText('buzzer')).toBeVisible()

      // 切到 Commands Tab，确认属性命令区不渲染 buzzer 动作调用
      await page.getByRole('tab', { name: SELECTORS.deviceTabs.commandsTab }).click()
      const commandsSection = page
        .locator('section')
        .filter({ has: page.getByRole('heading', { name: 'Property Commands' }) })
      // buzzer 是 service_type，只存在于 action_invocation；属性命令区不应出现该文本
      await expect(commandsSection.getByText('buzzer')).toHaveCount(0)
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
