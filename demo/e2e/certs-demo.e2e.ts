/**
 * Certificates Demo 测试
 *
 * 对应用户故事：US-PA-005 查看证书列表 (DEMO-003)
 *               US-PA-004 签发设备证书完整 E2E 流程 (DEMO-006)
 *
 * 验证管理员可以在后台查看证书列表，并导航到签发页面。
 * 验证管理员可以通过签发表单成功签发设备证书并在列表中确认。
 * 前置条件：系统中已有产品 "Demo Smart Light" (model_no: demo_product)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Certificates demo', () => {
  test('shows certificates list page with actions', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/certs`)

    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Issue Certificate' })).toBeVisible()
  })

  test('shows search form with product and device filters', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/certs`)

    await expect(page.locator('form').getByText('Product')).toBeVisible()
    await expect(page.locator('form').getByText('Device ID')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Search' })).toBeVisible()
  })

  test('navigates to issue certificate page', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/certs`)

    await page.getByRole('link', { name: 'Issue Certificate' }).click()
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/certs/create`))
    await expect(page.getByRole('heading', { name: 'Issue Certificate' })).toBeVisible()
  })

  test('issue certificate form has required fields', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/certs/create`)

    await expect(page.getByLabel('Product')).toBeVisible()
    await expect(page.getByLabel('Device ID')).toBeVisible()
    await expect(page.getByLabel('Start At')).toBeVisible()
    await expect(page.getByLabel('End At')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Issue' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Cancel' })).toBeVisible()
  })

  test('issues a certificate end-to-end and shows it in the list (DEMO-006 Scenario 1)', async ({ page }) => {
    const deviceId = `demo-device-${Date.now()}`

    await page.goto(`${FRONTEND_URL}/certs/create`)
    await expect(page.getByRole('heading', { name: 'Issue Certificate' })).toBeVisible()

    await page.getByLabel('Product').selectOption({ label: 'Demo Smart Light' })
    await page.getByLabel('Device ID').fill(deviceId)
    // Start At and End At are pre-filled with valid defaults; ensure they are present
    await expect(page.getByLabel('Start At')).not.toHaveValue('')
    await expect(page.getByLabel('End At')).not.toHaveValue('')

    await page.getByRole('button', { name: 'Issue' }).click()

    // The form is replaced by an inline success panel
    await expect(page.getByText('Certificate Issued Successfully')).toBeVisible()

    await page.getByRole('link', { name: 'Back to Certificates' }).click()
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/certs`))
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    await expect(page.getByText(deviceId)).toBeVisible()
    // The list page maps CertStatus.Normal -> "Active"
    await expect(
      page.locator('tr').filter({ hasText: deviceId }).getByText('Active')
    ).toBeVisible()
  })

  test('shows validation error when Device ID is empty (DEMO-006 Scenario 2)', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/certs/create`)
    await expect(page.getByRole('heading', { name: 'Issue Certificate' })).toBeVisible()

    await page.getByRole('button', { name: 'Issue' }).click()

    // Browser native validation blocks submission; URL should not change
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/certs/create`))

    // Page did not navigate away — field and heading still visible
    await expect(page.getByLabel('Device ID')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Issue Certificate' })).toBeVisible()
  })
})
