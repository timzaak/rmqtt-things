/**
 * Certificate Revoke/Invalidate Demo 测试
 *
 * 对应用户故事：US-PA-006 吊销/作废证书
 *
 * 验证场景：
 * 1. 吊销 Normal 状态证书 -> 状态变更为 Revoked
 * 2. 作废 Normal 状态证书 -> 状态变更为 Invalid
 * 3. 非 Normal 状态证书不显示 Revoke 和 Invalidate 操作按钮
 *
 * 前置条件：系统中已有产品 "Demo Smart Light" (model_no: demo_product)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { issueCert, updateCertStatus } from './helpers/cert-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Certificate revoke/invalidate demo (US-PA-006)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('revokes a Normal certificate and status updates to Revoked (Scenario 1)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `revoke-device-${Date.now()}`

    // Precondition: issue a Normal certificate via API
    await issueCert(deviceId)

    // Navigate to certs list
    await page.goto(`${FRONTEND_URL}/certs`)
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    // Find the row for our device and verify it shows Active (Normal)
    const row = page.locator('tr').filter({ hasText: deviceId })
    await expect(row.getByText('Active')).toBeVisible()

    // Click Revoke button in that row
    await row.getByRole('button', { name: 'Revoke' }).click()

    // ConfirmDialog appears — click the confirm button (text is "Revoke")
    const confirmDialog = page.locator('.fixed.inset-0.z-50')
    await expect(confirmDialog).toBeVisible()
    await confirmDialog.getByRole('button', { name: 'Revoke' }).click()

    // Wait for the status to update to Revoked in the list
    await expect(row.getByText('Revoked')).toBeVisible({ timeout: 15000 })
  })

  test('invalidates a Normal certificate and status updates to Invalid (Scenario 2)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `invalidate-device-${Date.now()}`

    // Precondition: issue a Normal certificate via API
    await issueCert(deviceId)

    // Navigate to certs list
    await page.goto(`${FRONTEND_URL}/certs`)
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    // Find the row for our device and verify it shows Active (Normal)
    const row = page.locator('tr').filter({ hasText: deviceId })
    await expect(row.getByText('Active')).toBeVisible()

    // Click Invalidate button in that row
    await row.getByRole('button', { name: 'Invalidate' }).click()

    // ConfirmDialog appears — click the confirm button (text is "Invalidate")
    const confirmDialog = page.locator('.fixed.inset-0.z-50')
    await expect(confirmDialog).toBeVisible()
    await confirmDialog.getByRole('button', { name: 'Invalidate' }).click()

    // Wait for the status to update to Invalid in the list
    await expect(row.locator('span').filter({ hasText: 'Invalid' })).toBeVisible({ timeout: 15000 })
  })

  test('does not show action buttons for non-Normal certificates (Scenario 3)', async ({ page, demoLogger: _demoLogger }) => {
    const revokedDeviceId = `revoked-row-${Date.now()}`
    const invalidDeviceId = `invalid-row-${Date.now()}`

    // Issue two certificates and immediately change their status via API
    await Promise.all([
      issueCert(revokedDeviceId).then(() => updateCertStatus('demo_product', revokedDeviceId, 2)),
      issueCert(invalidDeviceId).then(() => updateCertStatus('demo_product', invalidDeviceId, 1)),
    ])

    // Navigate to certs list
    await page.goto(`${FRONTEND_URL}/certs`)
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    // Verify the Revoked certificate row has no Revoke or Invalidate buttons
    const revokedRow = page.locator('tr').filter({ hasText: revokedDeviceId })
    await expect(revokedRow.locator('span').filter({ hasText: 'Revoked' })).toBeVisible()
    await expect(revokedRow.getByRole('button', { name: 'Revoke' })).not.toBeVisible()
    await expect(revokedRow.getByRole('button', { name: 'Invalidate' })).not.toBeVisible()

    // Verify the Invalid certificate row has no Revoke or Invalidate buttons
    const invalidRow = page.locator('tr').filter({ hasText: invalidDeviceId })
    await expect(invalidRow.locator('span').filter({ hasText: 'Invalid' })).toBeVisible()
    await expect(invalidRow.getByRole('button', { name: 'Revoke' })).not.toBeVisible()
    await expect(invalidRow.getByRole('button', { name: 'Invalidate' })).not.toBeVisible()
  })
})
