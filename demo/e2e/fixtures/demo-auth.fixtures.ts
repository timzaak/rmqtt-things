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
import { loginAsAdmin } from '../helpers/auth'

export const test = base.extend<{
  demoLogger: UnifiedLogger
  authenticatedPage: Page
  testStartTime: number
}>({
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
  testStartTime: async (_, use) => {
    const startTime = Date.now()
    await use(startTime)
  },

  /**
   * Fixture: Authenticated Page
   *
   * 验证环境后执行管理员登录，返回已认证的 Page。
   * 注意：此 fixture 会增加约 5-10 秒的测试设置时间。
   */
  authenticatedPage: async ({ page, demoLogger: _demoLogger, testStartTime: _testStartTime }, use) => {
    await verifyTestEnvironment(page, { logger: _demoLogger })
    await loginAsAdmin(page, { logger: _demoLogger })
    await use(page)
  },
})

export { expect } from '@playwright/test'
