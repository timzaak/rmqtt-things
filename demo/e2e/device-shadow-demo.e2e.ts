/**
 * Device Property Shadow Demo 测试
 *
 * 对应用户故事（Draft 来源：`.ai/user-stories/core/shadow-device-support.md`）：
 * - US-PA-042 设置设备期望状态（在线收敛 / 离线排队 / desired 不被一次性命令污染 / 空内容 400）
 * - US-PA-043 查看设备期望状态与差异（展示 desired/reported/delta / 偏离可见不自动收敛 / 下发失败期望保持）
 * - US-PA-044 在前端管理设备期望状态（设备上下文内查看 / 从界面设置 / 失败反馈）
 *
 * 覆盖映射：
 * - Scenario A -> US-PA-042 在线收敛 + US-PA-044 前端设置流程
 * - Scenario B -> US-PA-042 离线排队
 * - Scenario C -> US-PA-043 desired 不被一次性命令污染 + 偏离可见不自动收敛
 * - Scenario D -> US-PA-042 空对象须拒绝（400）
 * - Scenario E -> US-PA-043 命令 Failed 时 desired 保持
 *
 * 关键断言均落在持久业务状态（GET /api/admin/property/shadow 的 desired/delta，
 * GET /api/admin/property/command 的 status），不以 sonner/toast 为唯一验收依据。
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
 * 对齐前端 toKebabKey（PropertyShadowSection.tsx）：
 * camelCase 边界插连字符、连续大写末尾插连字符、非字母数字折叠为单连字符、去首尾、小写。
 * 用于构造动态 testid `shadow-status-${kebabKey}`。
 */
function shadowStatusSelector(key: string): string {
  const kebab = key
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase()
  return `[data-testid="shadow-status-${kebab}"]`
}

/** 限定到 "Desired State (Shadow)" 分区，避免误匹配其它 section。 */
function shadowSection(page: import('@playwright/test').Page) {
  return page
    .locator('section')
    .filter({ has: page.getByRole('heading', { name: 'Desired State (Shadow)' }) })
}

test.describe('Property Shadow (US-PA-042/043/044)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  // ---------------------------------------------------------------------------
  // Scenario A — US-PA-042 在线设 desired -> 推送 -> 收敛（含 US-PA-044 前端设置）
  // ---------------------------------------------------------------------------
  test('[Scenario A] US-PA-042/US-PA-044 set desired online via UI, device replies, delta converges', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-shadow-online-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 导航到设备详情页，确认 Shadow 分区存在
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
      await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
      await expect(page.getByRole('heading', { name: 'Desired State (Shadow)' })).toBeVisible()

      // 初始空态：desired 为空时前端渲染该提示
      await expect(shadowSection(page).getByText('No desired state set')).toBeVisible()

      // 打开 Set Desired State 对话框
      await page.locator(SELECTORS.shadow.setButton).click()

      // 在对话框编辑器中填入合法 JSON 对象
      const desiredPayload = { brightness: 80 }
      const dialog = page.locator('.fixed.inset-0.z-50')
      await expect(dialog).toBeVisible()
      await dialog.locator(SELECTORS.shadow.desiredEditor).fill(JSON.stringify(desiredPayload))

      // 在点击 submit 前启动命令等待，避免竞态
      const commandPromise = device.waitForCommand()

      // 提交 -> 对话框应关闭
      await dialog.locator(SELECTORS.shadow.submitButton).click()
      await expect(page.locator(SELECTORS.shadow.submitButton)).not.toBeVisible({ timeout: POLL_TIMEOUT })

      // 设备端：收到 delta 命令，data 应匹配发送的 desired
      const command = await commandPromise
      expect(command.data).toMatchObject(desiredPayload)

      // 设备回复 200，随后上报 reported 收敛
      await device.replyCommand(command)
      await device.postProperties(desiredPayload)

      // 断言收敛后持久状态：delta 应不含 brightness（已收敛）
      await expect.poll(
        async () => {
          const body = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return Object.prototype.hasOwnProperty.call(body.delta ?? {}, 'brightness')
        },
        { timeout: POLL_TIMEOUT },
      ).toBe(false)
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario B — US-PA-042 离线设 desired -> 排队 -> 上线投递
  // ---------------------------------------------------------------------------
  test('[Scenario B] US-PA-042 set desired while offline, delta queued then delivered on connect', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-shadow-offline-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      // 先 connect 再 disconnect 注册设备（参考 property-command-demo Scenario 2）
      await device.connect()
      await device.disconnect()

      // 离线状态下通过 API 设置 desired
      const desiredPayload = { power: true }
      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)
      const putBody = await putResponse.json()
      expect(putBody.pushed).toBe(true)

      // 通过 API 轮询断言存在 Pending 命令
      await expect.poll(
        async () => {
          const body = await getJson<ListResponse<PropertyCommandRow>>(
            request,
            `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return body.data?.[0]?.status
        },
        { timeout: POLL_TIMEOUT },
      ).toBe('Pending')

      // 设备上线，等待 delta 命令投递
      await device.connect()
      try {
        const command = await device.waitForCommand()
        expect(command.data).toMatchObject(desiredPayload)
        await device.replyCommand(command)

        // 断言命令状态收敛为 Success
        await expect.poll(
          async () => {
            const body = await getJson<ListResponse<PropertyCommandRow>>(
              request,
              `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
            )
            return String(body.data?.[0]?.status)
          },
          { timeout: POLL_TIMEOUT },
        ).toBe('Success')

        // 辅助 UI 断言：导航到详情页可见 Shadow 分区（非主验收）
        await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
        await expect(page.getByRole('heading', { name: 'Desired State (Shadow)' })).toBeVisible()
      } finally {
        await device.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario C — US-PA-043 desired 不被一次性命令污染 + 偏离可见不自动收敛
  // ---------------------------------------------------------------------------
  test('[Scenario C] US-PA-043 one-shot command does not pollute desired, delta stays visible', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-shadow-pollute-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 通过 API 设 desired { colorTemp: 400 }
      const desiredPayload = { colorTemp: 400 }
      // 在触发 PUT 之前注册 command waiter，避免 eager publish-on-trigger
      // 导致 delta 命令在 waiter 注册前就已下发而丢失（与 Scenario A/E 一致）。
      const shadowCommandPromise = device.waitForCommand()
      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)

      // 等待 delta 命令投递并回复，使其状态收敛为 Success
      const shadowCommand = await shadowCommandPromise
      expect(shadowCommand.data).toMatchObject(desiredPayload)
      await device.replyCommand(shadowCommand)

      // 通过一次性属性命令通道（非 shadow desired 端点）下发与 desired 不同的值
      const oneShotPayload = { colorTemp: 999 }
      // 在触发一次性命令前注册 waiter，避免 eager publish-on-trigger 导致命令丢失
      const oneShotCommandPromise = device.waitForCommand()
      const cmdResponse = await request.post('/api/admin/property/command', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, command: oneShotPayload },
      })
      expect(cmdResponse.status()).toBe(201)

      // 设备收到一次性命令并回复 200
      const oneShotCommand = await oneShotCommandPromise
      expect(oneShotCommand.data).toMatchObject(oneShotPayload)
      await device.replyCommand(oneShotCommand)

      // 上报 reported 为一次性命令的值（仍偏离 desired）
      await device.postProperties(oneShotPayload)

      // 断言持久状态：desired 未被一次性命令改变，delta 仍反映差异
      await expect.poll(
        async () => {
          const body = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return {
            desiredColorTemp: (body.desired ?? {}).colorTemp,
            deltaColorTemp: (body.delta ?? {}).colorTemp,
          }
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual({ desiredColorTemp: 400, deltaColorTemp: 400 })

      // 辅助 UI 断言：导航到详情页，delta 行可见。DesiredDelta 命令已被设备
      // ack（Success）但 reported 仍偏离期望值（一次性命令覆盖为 999），
      // 故 Status 显示 "Replied, not converged"（非主验收）。
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
      await expect(page.getByRole('heading', { name: 'Desired State (Shadow)' })).toBeVisible()
      await expect(
        shadowSection(page).locator(shadowStatusSelector('colorTemp')),
      ).toBeVisible({ timeout: POLL_TIMEOUT })
      await expect(shadowSection(page).getByText('Replied, not converged')).toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario D — US-PA-042 空对象须拒绝（400）
  // ---------------------------------------------------------------------------
  test('[Scenario D] US-PA-042 empty desired object is rejected with 400', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-shadow-empty-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      // 仅注册设备（connect 再 disconnect）
      await device.connect()
      await device.disconnect()

      // 空对象 desired 应返回 400，文案匹配
      const response = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: {} },
      })
      expect(response.status()).toBe(400)
      const text = await response.text()
      expect(text).toContain('desired must be a non-empty JSON object')

      // 辅助 UI 断言：通过 UI 提交空对象，前端视图 desired 仍为空（持久业务状态，非 toast）
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
      await expect(page.getByRole('heading', { name: 'Desired State (Shadow)' })).toBeVisible()

      await page.locator(SELECTORS.shadow.setButton).click()
      const dialog = page.locator('.fixed.inset-0.z-50')
      await expect(dialog).toBeVisible()
      await dialog.locator(SELECTORS.shadow.desiredEditor).fill('{}')
      await dialog.locator(SELECTORS.shadow.submitButton).click()

      // 持久业务状态：shadow 视图仍为空态
      await expect(shadowSection(page).getByText('No desired state set')).toBeVisible()
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario E — US-PA-043 命令 Failed 时 desired 保持
  // ---------------------------------------------------------------------------
  test('[Scenario E] US-PA-043 command Failed leaves desired intact', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-shadow-failed-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 通过 API 设 desired { brightness: 60 }
      const desiredPayload = { brightness: 60 }

      // 在触发 PUT 之前注册 command waiter，避免后端在设备订阅确认后立即推送 delta 命令导致丢失
      const commandPromise = device.waitForCommand()

      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)

      // 设备收到 delta 命令后用非 200 回复（Failed）
      const command = await commandPromise
      expect(command.data).toMatchObject(desiredPayload)
      await device.replyCommand(command, 500)

      // 断言持久状态：desired 未变；对应命令状态为 Failed
      await expect.poll(
        async () => {
          const shadowBody = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          const cmdBody = await getJson<ListResponse<PropertyCommandRow>>(
            request,
            `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return {
            desiredBrightness: (shadowBody.desired ?? {}).brightness,
            cmdStatus: String(cmdBody.data?.[0]?.status ?? ''),
          }
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual({ desiredBrightness: 60, cmdStatus: 'Failed' })

      // 辅助 UI 断言：delta 行可见。DesiredDelta 命令设备回复失败（Failed），
      // 故 Status 显示 "Delivery failed"（持久业务状态为主验收）。
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
      await expect(page.getByRole('heading', { name: 'Desired State (Shadow)' })).toBeVisible()
      await expect(
        shadowSection(page).locator(shadowStatusSelector('brightness')),
      ).toBeVisible({ timeout: POLL_TIMEOUT })
      await expect(shadowSection(page).getByText('Delivery failed')).toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
