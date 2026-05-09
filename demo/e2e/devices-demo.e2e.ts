/**
 * Devices List Demo 测试
 *
 * 对应用户故事：US-PA-019（设备列表页面）
 *
 * 验证管理员可以在 Web 后台查看设备列表，并按产品筛选设备；
 * 前置条件：系统中已有产品和设备连接记录（通过 seed_demo_data 初始化）。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Devices list page (US-PA-019)', () => {
  test('shows device list page with heading', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices`)

    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()
  })

  test('navigates to devices from sidebar', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/products`)
    await page.getByRole('link', { name: 'Devices' }).click()

    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/devices`))
    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()
  })

  test('shows filter controls for Product and Status', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices`)

    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    // Product filter dropdown
    const productLabel = page.getByText('Product', { exact: true })
    await expect(productLabel).toBeVisible()

    // Status filter dropdown (scoped to form to avoid matching table column header)
    const statusLabel = page.locator('form').getByText('Status', { exact: true })
    await expect(statusLabel).toBeVisible()
  })

  test('shows table headers for device list columns', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/devices`)

    await expect(page.getByRole('heading', { name: 'Devices' })).toBeVisible()

    const expectedHeaders = ['Device ID', 'Product ID', 'Status', 'IP Address', 'Last Online', 'Last Offline', 'Actions']
    for (const headerText of expectedHeaders) {
      await expect(page.getByRole('columnheader', { name: headerText })).toBeVisible()
    }
  })
})
