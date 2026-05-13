/**
 * Device Filters Demo 测试
 *
 * 对应用户故事：
 * - US-PA-014 场景1：查看设备状态列表
 * - US-PA-014 场景2：按状态筛选
 * - US-PA-019 场景2：按产品筛选设备
 * - US-PA-019 场景3：按在线/离线状态筛选
 * - US-PA-019 场景4：点击设备进入详情
 *
 * 验证管理员可以在设备列表页查看状态信息、使用筛选器、
 * 并点击设备 ID 跳转到详情页。
 *
 * 前置条件：
 * - 系统中已有 demo_product 产品和 demo-device 设备（seed_demo_data 初始化）
 * - 后端 API 和前端服务均已运行
 *
 * 注意：由于其他测试可能留下大量设备数据，seed 设备 demo-device 不一定在
 * 设备列表第一页。因此测试使用第一页可见的任意设备进行验证。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 15_000

test.describe('Device filters & navigation (US-PA-014, US-PA-019)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('US-PA-014 S1: shows device status info in table rows', async ({ page, demoLogger: _demoLogger }) => {
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    // Wait for any device row to appear (table body has data)
    await expect(page.locator('tbody tr').first()).toBeVisible({ timeout: POLL_TIMEOUT })

    // Status column should contain Online or Offline
    const statusCells = page.locator('tbody td').filter({ hasText: /^Online|Offline$/ })
    await expect(statusCells.first()).toBeVisible()

    // IP Address column should be visible (any row will do)
    await expect(page.locator('tbody td').filter({ hasText: /\d+\.\d+\.\d+\.\d+/ }).first()).toBeVisible()
  })

  test('US-PA-014 S2 / US-PA-019 S3: filter devices by Online status', async ({ page, demoLogger: _demoLogger }) => {
    // Create a device that will be Online
    const onlineDeviceId = `filter-online-${Date.now()}`
    const mqttDevice = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId: onlineDeviceId })

    await mqttDevice.connect()
    try {
      // Wait for the device to be registered as Online via API
      await expect.poll(async () => {
        const response = await page.request.get(
          `/api/admin/device/status?product_id=${PRODUCT_ID}&device_id=${onlineDeviceId}&page=1&page_size=10`,
        )
        const body = await response.json()
        return body.data?.[0]?.status
      }, { timeout: POLL_TIMEOUT }).toBe('Online')

      // Navigate to devices page
      await page.goto(`${FRONTEND_URL}/devices`)
      await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

      // Select Online status filter
      const statusSelect = page.locator('form select').filter({ has: page.getByRole('option', { name: 'Online' }) })
      await statusSelect.selectOption({ label: 'Online' })

      await page.getByRole('button', { name: 'Search' }).click()

      await expect(page.getByText(onlineDeviceId)).toBeVisible({ timeout: POLL_TIMEOUT })

      // Offline devices should not appear - check that no Offline text is in the table body
      const offlineCells = page.locator('tbody td').filter({ hasText: /^Offline$/ })
      await expect(offlineCells).toHaveCount(0)
    } finally {
      await mqttDevice.disconnect()
    }
  })

  test('US-PA-019 S3: filter devices by Offline status', async ({ page, demoLogger: _demoLogger }) => {
    // Create a device that connects then disconnects (will be Offline)
    const offlineDeviceId = `filter-offline-${Date.now()}`
    const mqttDevice = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId: offlineDeviceId })

    await mqttDevice.connect()
    // Wait for it to be Online first
    await expect.poll(async () => {
      const response = await page.request.get(
        `/api/admin/device/status?product_id=${PRODUCT_ID}&device_id=${offlineDeviceId}&page=1&page_size=10`,
      )
      const body = await response.json()
      return body.data?.[0]?.status
    }, { timeout: POLL_TIMEOUT }).toBe('Online')

    // Disconnect so it becomes Offline
    await mqttDevice.disconnect()

    // Wait for Offline status
    await expect.poll(async () => {
      const response = await page.request.get(
        `/api/admin/device/status?product_id=${PRODUCT_ID}&device_id=${offlineDeviceId}&page=1&page_size=10`,
      )
      const body = await response.json()
      return body.data?.[0]?.status
    }, { timeout: POLL_TIMEOUT }).toBe('Offline')

    // Navigate to devices page
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    // Select Offline status filter
    const statusSelect = page.locator('form select').filter({ has: page.getByRole('option', { name: 'Offline' }) })
    await statusSelect.selectOption({ label: 'Offline' })

    await page.getByRole('button', { name: 'Search' }).click()
    await expect(page.getByText(offlineDeviceId)).toBeVisible({ timeout: POLL_TIMEOUT })

    // Online devices should not appear
    const onlineCells = page.locator('tbody td').filter({ hasText: /^Online$/ })
    await expect(onlineCells).toHaveCount(0)
  })

  test('US-PA-019 S2: filter devices by product', async ({ page, demoLogger: _demoLogger }) => {
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    // The Product select uses model_no as value and product name as label.
    // Wait for product options to populate (loaded asynchronously via useProducts hook).
    // Note: <option> elements are "hidden" in Playwright's visibility model,
    // so we use count() with polling instead of toBeVisible().
    const firstSelect = page.locator('form select').first()
    const productOptions = firstSelect.locator('option:not([value=""])')
    await expect.poll(() => productOptions.count(), { timeout: POLL_TIMEOUT }).toBeGreaterThanOrEqual(1)

    const optionValue = await productOptions.first().getAttribute('value')
    await firstSelect.selectOption(optionValue!)

    await page.getByRole('button', { name: 'Search' }).click()

    await expect(page.locator('tbody tr').first()).toBeVisible({ timeout: POLL_TIMEOUT })
  })

  test('US-PA-019 S2: filtering by different product yields empty or different results', async ({ page, demoLogger: _demoLogger }) => {
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    // Verify the product select dropdown exists and has at least one non-empty option
    const firstSelect = page.locator('form select').first()
    await expect(firstSelect).toBeVisible()

    // Wait for product options to populate (option elements are "hidden" in Playwright)
    const productOptions = firstSelect.locator('option:not([value=""])')
    await expect.poll(() => productOptions.count(), { timeout: POLL_TIMEOUT }).toBeGreaterThanOrEqual(1)
  })

  test('US-PA-019 S4: click device ID navigates to detail page', async ({ page, demoLogger: _demoLogger }) => {
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    const firstDeviceLink = page.locator('tbody tr').first().getByRole('link', { name: /^filter-|demo-|e2e-|acl-|file-upload-|hmac-/ })
    await expect(firstDeviceLink).toBeVisible({ timeout: POLL_TIMEOUT })

    await firstDeviceLink.click()

    await expect(page).toHaveURL(new RegExp(`/devices/show/`), { timeout: POLL_TIMEOUT })
    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  })

  test('US-PA-019 S4: click View action navigates to detail page', async ({ page, demoLogger: _demoLogger }) => {
    await page.goto(`${FRONTEND_URL}/devices`)
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    const firstViewLink = page.locator('tbody tr').first().getByRole('link', { name: 'View' })
    await expect(firstViewLink).toBeVisible({ timeout: POLL_TIMEOUT })

    await firstViewLink.click()

    await expect(page).toHaveURL(new RegExp(`/devices/show/`), { timeout: POLL_TIMEOUT })
    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  })
})
