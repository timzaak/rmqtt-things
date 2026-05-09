/**
 * Device Detail Demo 测试
 *
 * 对应用户故事：US-PA-020（设备详情页面）
 *
 * 验证管理员可以查看设备详情页面的各区域，且每个区域都有数据展示。
 * 前置条件：系统中已有产品和设备连接记录（通过 seed_demo_data 初始化）。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const DEVICE_ID = 'demo-device'

test.describe('Device detail page (US-PA-020)', () => {
  test('shows device detail page with all section headings', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    const sectionHeadings = [
      'Device Info',
      'Latest Properties',
      'Property History',
      'Event History',
      'Property Commands',
      'Connection History',
    ]
    for (const headingText of sectionHeadings) {
      await expect(page.getByRole('heading', { name: headingText })).toBeVisible()
    }
  })

  test('shows Back to Devices link on detail page', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    const backLink = page.getByRole('link', { name: /Back to Devices/ })
    await expect(backLink).toBeVisible()
  })

  test('shows device info with seeded data', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Device Info' })).toBeVisible()

    // Device ID and Product ID should show seeded values
    await expect(page.getByText('demo-device')).toBeVisible()
    await expect(page.getByText('demo_product')).toBeVisible()

    // Status should show Online (green text)
    // Scope to Device Info section to avoid matching Connection History table
    const deviceInfoSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Device Info' }) })
    await expect(deviceInfoSection.getByText('Online', { exact: true })).toBeVisible()
  })

  test('shows latest properties with data rows', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Latest Properties' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No latest properties')).not.toBeVisible()

    // Should show seeded property values (temperature, humidity, power)
    // Scope to the Latest Properties section to avoid matching Property History
    const latestSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Latest Properties' }) })
    await expect(latestSection.getByText(/temperature/)).toBeVisible()
  })

  test('shows property history with data rows', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Property History' })).toBeVisible()

    await expect(page.getByText('No property history')).not.toBeVisible()
  })

  test('shows event history with data rows', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Event History' })).toBeVisible()

    await expect(page.getByText('No event history')).not.toBeVisible()
  })

  test('shows property commands with data and Send Command button', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Property Commands' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No commands')).not.toBeVisible()

    // Should show seeded command status
    await expect(page.getByText('Pending')).toBeVisible()

    // Send Command button should be visible
    const sendCommandButton = page.getByRole('button', { name: 'Send Command' })
    await expect(sendCommandButton).toBeVisible()
  })

  test('shows connection history with data rows', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`)

    await expect(page.getByRole('heading', { name: 'Connection History' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No connection history')).not.toBeVisible()
  })
})
