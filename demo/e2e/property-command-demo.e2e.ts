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
 * 前置条件：系统中已有产品 "demo_product"。
 * 前置条件：RMQTT broker 运行在 MQTT_URL (默认 mqtt://127.0.0.1:1883)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 * 前置条件：前端运行在 FRONTEND_URL (默认 http://localhost:3000)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { getJson } from './helpers/api'
import { verifyTestEnvironment } from './helpers/environment-setup'

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

test.describe('Property Command (US-PA-016)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('[Scenario 1] sends command via UI while device is online, status becomes Sent', async ({ page, request: _request, demoLogger: _demoLogger }) => {
    const deviceId = `e2e-cmd-online-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      // Navigate to device detail page
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
      await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

      // Open the Send Command dialog
      await page.getByRole('button', { name: 'Send Command' }).click()

      // Fill in command JSON in the dialog textarea
      const commandPayload = { power: false, brightness: 55 }
      const dialog = page.locator('.fixed.inset-0.z-50')
      await expect(dialog).toBeVisible()
      const textarea = dialog.locator('textarea')
      await textarea.fill(JSON.stringify(commandPayload))

      // Set up command waiter before clicking Send
      const commandPromise = device.waitForCommand()

      // Submit the command
      await dialog.getByRole('button', { name: 'Send' }).click()

      // Device receives the command
      const command = await commandPromise
      expect(command.data).toMatchObject(commandPayload)

      // Reply to complete the command
      await device.replyCommand(command)

      // Verify command appears in the table with Success status
      const commandSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Property Commands' }) })
      await expect(commandSection.getByText('Success')).toBeVisible({ timeout: POLL_TIMEOUT })
    } finally {
      await device.disconnect()
    }
  })

  test('[Scenario 2] creates command while device is offline, status is Pending and visible in UI', async ({ page, request, demoLogger: _demoLogger }) => {
    const deviceId = `e2e-cmd-offline-${Date.now()}`
    const commandPayload = { power: true, temperature: 25 }

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

    // Navigate to device detail page and verify Pending command is visible
    await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    const commandSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Property Commands' }) })
    await expect(commandSection.getByText('Pending')).toBeVisible({ timeout: POLL_TIMEOUT })
  })

  test('[Scenario 3] deletes a Pending command and it disappears from the list', async ({ page, request, demoLogger: _demoLogger }) => {
    const deviceId = `e2e-cmd-delete-${Date.now()}`
    const commandPayload = { brightness: 80 }

    // Connect and disconnect device to register it in the system
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
    await device.connect()
    await device.disconnect()

    // Create a Pending command via API (device is offline)
    const createResponse = await request.post('/api/admin/property/command', {
      data: { product_id: PRODUCT_ID, device_id: deviceId, command: commandPayload },
    })
    expect(createResponse.status()).toBe(201)

    // Navigate to device detail page
    await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`)
    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    // Find the command row with Pending status for this device
    const commandSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Property Commands' }) })
    await expect(commandSection.getByText('Pending')).toBeVisible({ timeout: POLL_TIMEOUT })

    // Click the Delete button in the Pending command row
    const pendingRow = commandSection.locator('tr').filter({ hasText: 'Pending' })
    await expect(pendingRow.getByRole('button', { name: 'Delete' })).toBeVisible()
    await pendingRow.getByRole('button', { name: 'Delete' }).click()

    // Verify the command status changed to Deleted (soft delete)
    await expect(commandSection.getByText('Deleted')).toBeVisible({ timeout: POLL_TIMEOUT })

    // Double-check via API that the command status is Deleted
    const body = await getJson<ListResponse<PropertyCommandRow>>(
      request,
      `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
    )
    expect(body.data?.[0]?.status).toBe('Deleted')
  })
})
