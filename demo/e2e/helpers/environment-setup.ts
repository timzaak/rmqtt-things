/**
 * 环境验证工具
 *
 * 在测试运行前验证后端服务是否就绪。
 *
 * 使用方式：
 * ```typescript
 * import { verifyTestEnvironment } from './helpers/environment-setup'
 * await verifyTestEnvironment(page)
 * ```
 *
 * 依赖：
 * - 后端提供 GET /api/health 接口
 */

import { Page } from '@playwright/test'
import type { UnifiedLogger } from 'playwright-unified-logger'
import { fetchAuthConfig } from './auth'

export const BASE_URL = process.env.BASE_URL || 'http://localhost:8080'

export interface VerifyEnvironmentOptions {
  /** 跳过后端健康检查 */
  skipBackendCheck?: boolean
  /** 日志记录器 */
  logger?: UnifiedLogger
}

interface ValidationResult {
  healthy: boolean
  errors?: string[]
}

/**
 * 验证测试环境状态
 */
export async function verifyTestEnvironment(
  _page: Page | null,
  options: VerifyEnvironmentOptions = {}
): Promise<void> {
  const { skipBackendCheck = false, logger } = options

  logger?.testCode.log('[Env] 验证测试环境...') ?? console.warn('[Env] 验证测试环境...')

  if (!skipBackendCheck) {
    await verifyBackendConnections(logger)
    await verifyHeraldConnection(logger)
  }

  logger?.testCode.log('[Env] 环境验证通过') ?? console.warn('[Env] 环境验证通过')
}

async function verifyBackendConnections(logger?: UnifiedLogger): Promise<void> {
  const result = await validateBackendHealth({
    maxRetries: 3,
    retryDelay: 2000,
    timeout: 10000,
  }, logger)

  if (!result.healthy) {
    throw new Error(`Backend health check failed:\n${result.errors?.join('\n') || 'Unknown error'}`)
  }

  logger?.testCode.log('[Env] 后端服务连接正常') ?? console.warn('[Env] 后端服务连接正常')
}

async function validateBackendHealth(options: {
  maxRetries: number
  retryDelay: number
  timeout: number
}, logger?: UnifiedLogger): Promise<ValidationResult> {
  const { maxRetries, retryDelay, timeout } = options

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      const response = await fetch(`${BASE_URL}/api/health`, {
        method: 'GET',
        signal: AbortSignal.timeout(timeout),
      })

      if (response.ok) {
        return { healthy: true }
      }
    } catch (error) {
      if (attempt < maxRetries - 1) {
        logger?.testCode.log(`[Env] 健康检查失败，重试 ${attempt + 1}/${maxRetries}...`) ?? console.warn(`[Env] 健康检查失败，重试 ${attempt + 1}/${maxRetries}...`)
        await new Promise(resolve => setTimeout(resolve, retryDelay))
      } else {
        return {
          healthy: false,
          errors: [`Health check failed after ${maxRetries} attempts: ${error}`],
        }
      }
    }
  }

  return { healthy: false, errors: ['Health check failed: Max retries exceeded'] }
}

async function verifyHeraldConnection(logger?: UnifiedLogger): Promise<void> {
  const config = await fetchAuthConfig()

  if (!config.enabled || !config.login_url) {
    logger?.testCode.log('[Env] Herald SSO 未启用，跳过检查') ?? console.warn('[Env] Herald SSO 未启用，跳过检查')
    return
  }

  // Derive Herald base URL from login_url (e.g. http://host:13000/default/auth/login -> http://host:13000)
  const heraldBaseUrl = config.login_url.replace(/\/[^/]*\/auth\/login$/, '')

  try {
    const resp = await fetch(heraldBaseUrl, {
      method: 'GET',
      signal: AbortSignal.timeout(5000),
    })
    // 只要能连上即可，不要求特定状态码
    logger?.testCode.log(`[Env] Herald SSO 连接正常 (${resp.status})`) ?? console.warn(`[Env] Herald SSO 连接正常 (${resp.status})`)
  } catch (error) {
    throw new Error(`Herald SSO service is not available at ${heraldBaseUrl} but auth is enabled: ${error}`)
  }
}
