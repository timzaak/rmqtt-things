/**
 * Property Command Demo 测试
 *
 * 对应用户故事：US-PA-016 下发属性命令
 *
 * 验证场景：
 * 1. 设备在线时通过 UI 发送属性命令 -> 命令状态为 Sent
 * 2. 设备离线时通过 API 创建属性命令 -> 命令状态为 Pending，前端可见
 * 3. 删除 Pending 状态的命令 -> 命令从列表中移除
 *
 * DE-D02 适配（device-detail-experience 七区信息架构）：旧 PropertyCommandsSection
 * 已删除，一次性属性命令历史汇入统一 Operations 表（DeviceOperationsSection.tsx），
 * 下发入口收敛到 Operations tab 的 More ▾ → Direct property write
 * （DirectPropertyWriteDialog.tsx）。变化点：
 * - openCommandsTab → openOperationsTab：点击 device-tab-operations
 * - 分区 heading "Property Commands" → "Operations"（断言落在 device-operations-table）
 * - 旧 "Send Command" 按钮 → More ▾ → Direct property write（direct-property-write-dialog /
 *   command-json-input / target-conflict-warning）
 * 后端 API（POST /api/admin/property/command 创建、DELETE、GET 状态机）行为不变，
 * 持久 API 断言保持原样。
 *
 * 前置条件：系统中已有产品 "demo_product"。
 * 前置条件：RMQTT broker 运行在 MQTT_URL (默认 mqtt://127.0.0.1:1883)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 * 前置条件：前端运行在 FRONTEND_URL (默认 http://localhost:3000)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { Page } from '@playwright/test'
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

/**
 * 导航到设备详情页并切换到 Operations tab。
 *
 * DE-D02：设备详情页默认 activeTab='overview'，DeviceOperationsSection 仅在
 * activeTab==='operations' 时渲染（show.$id.tsx:126-128）。Tab 由 data-testid
 * `device-tab-operations` 定位（优先用 testid），分区 heading "Operations" 为
 * 持久锚点。Tab 选择器集中在 SELECTORS.deviceTabs.operationsTab。
 */
async function openOperationsTab(page: Page, deviceId: string): Promise<void> {
  // waitUntil:'commit' 在收到响应头即返回，避免 Vite dev server 下
  // 'load'/'domcontentloaded' 偶发不触发导致 30s 超时（DE-TR01 复现）。
  await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`, {
    waitUntil: 'commit',
  })
  await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  await page.locator(SELECTORS.deviceTabs.operationsTab).click()
  await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
}

test.describe('Property Command (US-PA-016)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('[Scenario 1] Send command online, device replies and status becomes Success', async ({ page, request, demoLogger: _demoLogger }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-cmd-online-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 导航到设备详情页并切换到 Operations tab（DE-D02：属性命令入口收敛到
      // Operations.region 的 More ▾ → Direct property write）。
      await openOperationsTab(page, deviceId)

      // DE-D02：下发入口为 More ▾ → Direct property write（SELECTORS.operations.*，
      // 禁止硬编码 testid）。
      await page.locator(SELECTORS.operations.moreActionsButton).click()
      await page.locator(SELECTORS.operations.directPropertyWriteButton).click()

      // Fill in command JSON in the dialog textarea
      const commandPayload = { power: false, brightness: 55 }
      const dialog = page.locator(SELECTORS.operations.directWriteDialog)
      await expect(dialog).toBeVisible()
      await dialog.locator(SELECTORS.operations.directWriteJsonInput).fill(JSON.stringify(commandPayload))

      // Set up command waiter before clicking Send
      const commandPromise = device.waitForCommand()

      // Submit the command
      await dialog.getByRole('button', { name: 'Send' }).click()

      // Device receives the command
      const command = await commandPromise
      // Protocol assertion (spec envelope): params carries the submitted
      // property object directly; id is a property:{db_id} association string.
      expect(command.params).toMatchObject(commandPayload)
      expect(command.id).toEqual(expect.stringMatching(/^property:\d+$/))

      // Reply to complete the command
      await device.replyCommand(command)

      // DE-D02：命令状态在统一 Operations 表中显示（device-operations-table）。
      // StatusBadge 渲染 CommandStatus 枚举字符串（StatusBadge.tsx），Success 行可见。
      const operationsTable = page.locator(SELECTORS.operations.table)
      await expect(operationsTable.getByText('Success', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  test('[Scenario 2] creates command while device is offline, status is Pending and visible in UI', async ({ page, request, demoLogger: _demoLogger }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-cmd-offline-${Date.now()}`
    const commandPayload = { power: true, temperature: 25 }

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      // Connect and disconnect device to register it in the system
      const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
      await device.connect()
      await device.disconnect()

      // Create command via API while device is offline
      const createResponse = await request.post('/api/admin/property/command', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, command: commandPayload },
      })
      expect(createResponse.status()).toBe(201)

      // Verify via API that command is Pending
      await expect.poll(async () => {
        const body = await getJson<ListResponse<PropertyCommandRow>>(
          request,
          `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
        )
        const row = body.data?.[0]
        if (!row) throw new Error('Command not found')
        return String(row.status)
      }, { timeout: POLL_TIMEOUT }).toBe('Pending')

      // 导航到设备详情页并切换到 Operations tab，确认 Pending 命令在统一表中可见
      await openOperationsTab(page, deviceId)

      const operationsTable = page.locator(SELECTORS.operations.table)
      await expect(operationsTable.getByText('Pending', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  test('[Scenario 3] deletes a Pending command and it disappears from the list', async ({ page, request, demoLogger: _demoLogger }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-cmd-delete-${Date.now()}`
    const commandPayload = { brightness: 80 }

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      // Connect and disconnect device to register it in the system
      const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
      await device.connect()
      await device.disconnect()

      // Create a Pending command via API (device is offline)
      const createResponse = await request.post('/api/admin/property/command', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, command: commandPayload },
      })
      expect(createResponse.status()).toBe(201)

      // 导航到设备详情页并切换到 Operations tab（DE-D02：Pending 行在统一表中）。
      await openOperationsTab(page, deviceId)
      const operationsTable = page.locator(SELECTORS.operations.table)
      await expect(operationsTable.getByText('Pending', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })

      // DE-D02：统一 Operations 表第一阶段不暴露行内 Delete 入口（设计 §5.3：
      // "第一阶段可不展示统一取消按钮，以减少误操作"；DeviceOperationsSection 列定义
      // 无 Action/Delete 列）。Pending 命令的取消入口在原 DELETE /admin/property/command
      // 端点（ids 查询参数；仅 status=Pending 的行可被软删）。先从 GET 列表提取命令 id，
      // 再调用 DELETE，与前端基线 UI 不可达的取消路径等价。
      const beforeDelete = await getJson<ListResponse<PropertyCommandRow>>(
        request,
        `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
      )
      const pendingRow = beforeDelete.data?.find((row) => String(row.status) === 'Pending')
      expect(pendingRow, 'Pending command should exist before delete').toBeDefined()
      const commandId = pendingRow!.id

      const deleteResponse = await request.delete('/api/admin/property/command', {
        // 后端 DeletePropertyCommandsQuery.ids: Vec<i64>，axum Query 解析器要求
        // 重复的 `ids=` 查询参数；Playwright params 仅接受标量值，故显式构造
        // URLSearchParams 以正确表达单元素序列（ids=123 => vec![123]）。
        params: new URLSearchParams([['ids', String(commandId)]]),
      })
      expect(deleteResponse.status()).toBeLessThan(300)

      // DE-D02：统一操作表反映状态变更。重新加载页面触发 useDeviceOperations 重新查询
      // （前端不会主动失效），Deleted 行可见。用 page.reload() 而非再次 page.goto，
      // 避免 Vite dev server 下连续同 URL goto 偶发卡死（DE-TR01 修复）。
      await page.reload({ waitUntil: 'commit' })
      await page.locator(SELECTORS.deviceTabs.operationsTab).click()
      await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
      await expect(operationsTable.getByText('Deleted', { exact: true })).toBeVisible({ timeout: POLL_TIMEOUT })

      // 持久业务状态：API 层确认该命令 status=Deleted（软删）。
      const body = await getJson<ListResponse<PropertyCommandRow>>(
        request,
        `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
      )
      expect(body.data?.[0]?.status).toBe('Deleted')
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
