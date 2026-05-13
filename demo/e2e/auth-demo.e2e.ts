/**
 * 认证集成测试
 *
 * 验证 Herald SSO 认证流程与无认证模式均可正常工作。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { BASE_URL } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Auth integration', () => {
  test('detects auth config', async ({ page }) => {
    // 直接调用后端 API 验证配置端点
    const response = await page.request.get(`${BASE_URL}/api/auth/config`)
    expect(response.ok()).toBeTruthy()

    const config = await response.json()
    expect(config).toHaveProperty('enabled')
    expect(typeof config.enabled).toBe('boolean')
  })

  test('loginAsAdmin succeeds with Herald SSO', async ({ authenticatedPage, demoLogger }) => {
    // authenticatedPage fixture 已执行 loginAsAdmin，验证后续 API 调用不被 401 拦截
    const response = await authenticatedPage.request.get(`${BASE_URL}/api/admin/products`)
    demoLogger.testCode.log(`[Auth] 管理员 API 状态: ${response.status()}`)

    expect(response.status()).not.toBe(401)
  })

  test('navigates to admin page after login', async ({ authenticatedPage }) => {
    await authenticatedPage.goto(`${FRONTEND_URL}/devices`)

    await expect(authenticatedPage.getByRole('heading', { name: 'Devices' })).toBeVisible()
  })
})
