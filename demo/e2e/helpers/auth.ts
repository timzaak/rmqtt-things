/**
 * 认证辅助函数
 *
 * 为 E2E 测试提供登录、登出等认证相关功能。
 *
 * 使用方式：
 * ```typescript
 * import { loginAsAdmin, logout } from './helpers/auth'
 * await loginAsAdmin(page)
 * ```
 *
 * 配置：
 * - BASE_URL: 后端地址（环境变量，默认 http://localhost:8080）
 * - 修改 DEMO_ADMIN 以匹配你的测试账号
 */

import { Page, type Response } from '@playwright/test'
import type { UnifiedLogger } from 'playwright-unified-logger'

const BASE_URL = process.env.BASE_URL || 'http://localhost:8080'

/**
 * 默认管理员账号 — 根据项目实际情况修改
 */
export const DEMO_ADMIN = {
  email: 'admin@example.com',
  password: 'password',
}

/**
 * 使用管理员账号登录
 *
 * @param page Playwright Page 对象
 * @param options.waitNavigation 是否等待导航完成（默认 true）
 */
export async function loginAsAdmin(
  page: Page,
  options: {
    waitNavigation?: boolean
    logger?: UnifiedLogger
  } = {}
): Promise<void> {
  const { waitNavigation = true, logger } = options

  logger?.testCode.log(`[Auth] 登录管理员`) ?? console.log(`[Auth] 登录管理员`)

  await clearSessionData(page)

  // 导航到管理后台登录页
  await page.goto(`${BASE_URL}/admin/login`, { waitUntil: 'domcontentloaded' })

  // 检查是否已登录（自动跳转到管理页面）
  if (page.url().includes('/admin/dashboard') || page.url().includes('/admin/devices')) {
    logger?.testCode.log(`[Auth] 已登录，跳过`) ?? console.log(`[Auth] 已登录，跳过`)
    return
  }

  try {
    // 等待登录表单出现 — 选择器根据项目实际情况修改
    await page.waitForSelector('input[type="text"], [data-testid="email-input"]', { timeout: 10000 })
    await page.waitForSelector('input[type="password"], [data-testid="password-input"]', { timeout: 10000 })

    const usernameInput = page.locator('input[type="text"], [data-testid="email-input"]').first()
    const passwordInput = page.locator('input[type="password"], [data-testid="password-input"]').first()

    await usernameInput.fill(DEMO_ADMIN.email)
    await passwordInput.fill(DEMO_ADMIN.password)

    const submitButton = page.locator('button[type="submit"]').first()
    await submitButton.click()

    // 等待登录 API 响应
    const loginResponse = await waitForLoginResponse(page)
    if (loginResponse && !loginResponse.ok()) {
      const errorBody = await loginResponse.text().catch(() => '')
      throw new Error(`Login failed: API returned ${loginResponse.status()} - ${errorBody}`)
    }

    if (waitNavigation) {
      await page.waitForURL('**/admin/**', { timeout: 10000 }).catch(() => {})
    }

    logger?.testCode.log(`[Auth] 登录成功`) ?? console.log(`[Auth] 登录成功`)
  } catch (error) {
    logger?.testCode.error(`[Auth] 登录失败:`, error) ?? console.error(`[Auth] 登录失败:`, error)
    throw error
  }
}

/**
 * 登出当前用户
 */
export async function logout(page: Page, logger?: UnifiedLogger): Promise<void> {
  logger?.testCode.log('[Auth] 执行登出') ?? console.log('[Auth] 执行登出')

  try {
    const logoutButton = page.locator('[data-testid="logout-button"]').first()
    if (await logoutButton.isVisible({ timeout: 2000 })) {
      await logoutButton.click()
      await page.waitForURL('**/login', { timeout: 5000 })
    }
  } catch {
    logger?.testCode.log('[Auth] UI 登出失败，清除会话') ?? console.log('[Auth] UI 登出失败，清除会话')
  } finally {
    await clearSessionData(page)
    await page.goto(`${BASE_URL}/admin/login`, { waitUntil: 'networkidle' })
  }
}

async function clearSessionData(page: Page): Promise<void> {
  await page.context().clearCookies()
  try {
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })
  } catch {
    // localStorage 访问被阻止时忽略
  }
}

async function waitForLoginResponse(page: Page): Promise<Response | null> {
  return page
    .waitForResponse(
      response => response.url().includes('/login') && response.request().method() === 'POST',
      { timeout: 10000 }
    )
    .catch(() => null)
}
