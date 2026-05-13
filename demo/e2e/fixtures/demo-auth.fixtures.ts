/**
 * Demo 测试 Fixtures
 *
 * 扩展 Playwright base test，提供可复用的测试 fixture：
 * - demoLogger: 预配置的 UnifiedLogger（自动 finalize）
 * - authenticatedPage: 已登录的 Page（管理员身份）
 * - testStartTime: 测试开始时间戳（用于数据清理）
 *
 * 使用方式：
 * ```typescript
 * import { test, expect } from '../fixtures/demo-auth.fixtures'
 *
 * test('my test', async ({ page, demoLogger }) => {
 *   console.log('开始测试') // 自动被 UnifiedLogger 捕获
 * })
 * ```
 */

import { test as base, type Page } from '@playwright/test'
import { UnifiedLogger } from 'playwright-unified-logger'
import { verifyTestEnvironment } from '../helpers/environment-setup'
import { loginAsAdmin, ensureAuthCookie } from '../helpers/auth'

type DemoFixtures = {
  demoLogger: UnifiedLogger
  testStartTime: number
  authenticatedPage: Page
}

export const test = base.extend<DemoFixtures>({
  /**
   * Fixture: Page (auto-auth)
   *
   * 覆盖内置 page fixture：自动检测 Herald SSO 并注入 X-Auth cookie。
   * 所有使用 { page } 的测试自动获得认证，无需逐个文件处理。
   */
  page: async ({ page }, use) => {
    await ensureAuthCookie(page)
    await use(page)
  },

  /**
   * Fixture: Request (shares auth with page)
   *
   * 覆盖内置 request fixture：使用 page.request 以共享浏览器上下文的 cookie。
   * 确保 request.post/get 等 API 调用也携带认证信息。
   */
  request: async ({ page }, use) => {
    await use(page.request)
  },

  /**
   * Fixture: Demo Logger
   *
   * 创建 UnifiedLogger 实例，测试结束后自动打印摘要并保存日志文件。
   */
  demoLogger: async ({ page }, use, testInfo) => {
    const logger = new UnifiedLogger(page, testInfo.title)
    await use(logger)
    logger.printSummary('[Demo] Test Summary')
    await logger.finalize()
  },

  /**
   * Fixture: Test Start Time
   *
   * 记录测试开始时间，用于后续数据清理。
   */
  // eslint-disable-next-line no-empty-pattern -- Playwright requires object destructuring for fixture dependencies.
  testStartTime: async ({}, use) => {
    const startTime = Date.now()
    await use(startTime)
  },

  /**
   * Fixture: Authenticated Page
   *
   * 验证环境后执行管理员登录，返回已认证的 Page。
   * 自动检测认证模式：Herald SSO 启用时走 API 登录，否则直接导航。
   * 注意：此 fixture 会增加约 5-10 秒的测试设置时间。
   */
  authenticatedPage: async ({ page, demoLogger, testStartTime: _testStartTime }, use) => {
    await verifyTestEnvironment(page, { logger: demoLogger })
    await loginAsAdmin(page, { logger: demoLogger })
    await use(page)
  },

})

export { expect } from '@playwright/test'
