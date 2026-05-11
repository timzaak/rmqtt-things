/**
 * Certificate Detail Demo 测试
 *
 * 对应用户故事：US-PA-023 查看证书详情
 *
 * 验证场景：
 * 1. 在证书列表点击 Show 链接，进入详情页查看完整证书信息
 * 2. 从详情页点击返回链接，导航回证书列表
 *
 * 前置条件：系统中已有产品 "Demo Smart Light" (model_no: demo_product)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { issueCertAndGetId } from './helpers/cert-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Certificate detail demo (US-PA-023)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null as any)
  })

  test('shows certificate detail with all fields (Scenario 1)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `detail-device-${Date.now()}`

    // Precondition: issue a certificate via API and get its ID
    const certId = await issueCertAndGetId(deviceId)

    // Navigate to certs list
    await page.goto(`${FRONTEND_URL}/certs`)
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    // Find the row for our device and click the Show link
    const row = page.locator('tr').filter({ hasText: deviceId })
    await expect(row).toBeVisible()
    await row.getByRole('link', { name: 'Show' }).click()

    // Verify we navigated to the detail page
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/certs/show/${certId}`))
    await expect(page.getByRole('heading', { name: 'Certificate Detail' })).toBeVisible()

    // Verify all expected fields are displayed on the detail page
    // ID
    await expect(page.getByText(String(certId), { exact: true })).toBeVisible()
    // Product
    await expect(page.getByText('demo_product')).toBeVisible()
    // Device ID
    await expect(page.getByText(deviceId)).toBeVisible()
    // Certificate PEM content (starts with BEGIN CERTIFICATE)
    await expect(page.getByText(/BEGIN CERTIFICATE/)).toBeVisible()
    // Status should show Active (Normal -> Active mapping)
    await expect(page.getByText('Active')).toBeVisible()
    // Start At and End At fields should be visible with datetime values
    await expect(page.getByText(/Start At/)).toBeVisible()
    await expect(page.getByText(/End At/)).toBeVisible()
    // Created At
    await expect(page.getByText(/Created At/)).toBeVisible()
  })

  test('navigates back to certificates list from detail page (Scenario 2)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `back-device-${Date.now()}`

    // Precondition: issue a certificate and navigate to detail page
    const certId = await issueCertAndGetId(deviceId)

    // Go directly to the detail page URL
    await page.goto(`${FRONTEND_URL}/certs/show/${certId}`)
    await expect(page.getByRole('heading', { name: 'Certificate Detail' })).toBeVisible()

    // Click the back link
    await page.getByRole('link', { name: 'Back to Certificates' }).click()

    // Verify we are back on the certs list page
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/certs`))
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()
  })
})
