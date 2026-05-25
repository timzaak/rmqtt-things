/**
 * 认证辅助函数
 *
 * 为 E2E 测试提供登录、登出等认证相关功能。
 * 自动检测 Herald SSO 是否启用，支持两种认证模式。
 */

import { Page } from '@playwright/test'
import type { UnifiedLogger } from 'playwright-unified-logger'

const BASE_URL = process.env.BASE_URL || 'http://localhost:8080'

/**
 * 默认管理员账号 — 匹配 Herald SSO 测试环境
 */
export const DEMO_ADMIN = {
  email: 'admin@rmqtt-things.local',
  password: 'password',
}

/** GET /api/auth/config 返回结构 */
export interface AuthConfig {
  enabled: boolean
  login_url?: string | null
  herald_login_url?: string | null
}

/**
 * 获取后端认证配置
 */
export async function fetchAuthConfig(): Promise<AuthConfig> {
  const resp = await fetch(`${BASE_URL}/api/auth/config`, {
    method: 'GET',
    signal: AbortSignal.timeout(5000),
  })
  if (!resp.ok) {
    throw new Error(`Failed to fetch auth config: ${resp.status}`)
  }
  return resp.json() as Promise<AuthConfig>
}

/**
 * 通过 Herald API 直接登录，提取 X-Auth token 并注入浏览器
 */
async function loginViaHeraldApi(
  page: Page,
  heraldUrl: string,
  options: { logger?: UnifiedLogger } = {}
): Promise<void> {
  const { logger } = options
  const loginUrl = `${heraldUrl}/api/auth/rmqtt/login`

  logger?.testCode.log(`[Auth] Herald SSO 登录: ${loginUrl}`) ?? console.warn(`[Auth] Herald SSO 登录: ${loginUrl}`)

  const resp = await fetch(loginUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email: DEMO_ADMIN.email,
      password: DEMO_ADMIN.password,
      clientId: 'admin-web-console',
    }),
    signal: AbortSignal.timeout(10000),
  })

  if (!resp.ok) {
    const body = await resp.text().catch(() => '')
    throw new Error(`Herald login failed (${resp.status}): ${body}`)
  }

  // 从 Set-Cookie 提取 X-Auth token
  const setCookieHeaders = resp.headers.getSetCookie()
  const xAuthCookie = setCookieHeaders.find((h) => h.startsWith('X-Auth='))
  if (!xAuthCookie) {
    throw new Error('Herald login response missing X-Auth cookie')
  }

  const token = xAuthCookie.replace('X-Auth=', '').split(';')[0]

  // 注入浏览器 cookie — domain 从 BASE_URL 提取以兼容 localhost 和 127.0.0.1
  const domain = new URL(BASE_URL).hostname
  await page.context().addCookies([
    { name: 'X-Auth', value: token, domain, path: '/' },
  ])

  logger?.testCode.log(`[Auth] Herald SSO 登录成功，token 已注入`) ?? console.warn(`[Auth] Herald SSO 登录成功，token 已注入`)
}

/**
 * 确保浏览器上下文包含认证 cookie（不导航）
 *
 * 自动检测 Herald SSO 是否启用。启用时通过 API 登录并注入 X-Auth cookie。
 * 供 fixture 在每个测试前自动调用，避免各测试文件重复处理认证。
 */
export async function ensureAuthCookie(
  page: Page,
  options: { logger?: UnifiedLogger } = {}
): Promise<void> {
  const { logger } = options
  const config = await fetchAuthConfig()

  if (!config.enabled || !config.herald_login_url) {
    return
  }

  // Derive Herald API base URL from herald_login_url (e.g. http://host:13000/default/auth/login -> http://host:13000)
  const heraldBaseUrl = config.herald_login_url.replace(/\/[^/]*\/auth\/login$/, '')
  const loginUrl = `${heraldBaseUrl}/api/auth/rmqtt/login`
  logger?.testCode.log(`[Auth] 注入认证 cookie: ${loginUrl}`) ?? console.warn(`[Auth] 注入认证 cookie: ${loginUrl}`)

  const resp = await fetch(loginUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email: DEMO_ADMIN.email,
      password: DEMO_ADMIN.password,
      clientId: 'admin-web-console',
    }),
    signal: AbortSignal.timeout(10000),
  })

  if (!resp.ok) {
    const body = await resp.text().catch(() => '')
    throw new Error(`Herald login failed (${resp.status}): ${body}`)
  }

  const setCookieHeaders = resp.headers.getSetCookie()
  const xAuthCookie = setCookieHeaders.find((h) => h.startsWith('X-Auth='))
  if (!xAuthCookie) {
    throw new Error('Herald login response missing X-Auth cookie')
  }

  const token = xAuthCookie.replace('X-Auth=', '').split(';')[0]
  const domain = new URL(BASE_URL).hostname
  await page.context().addCookies([
    { name: 'X-Auth', value: token, domain, path: '/' },
  ])

  logger?.testCode.log('[Auth] 认证 cookie 已注入') ?? console.warn('[Auth] 认证 cookie 已注入')
}

/**
 * 使用管理员账号登录
 *
 * 自动检测认证模式：Herald SSO 启用时走 API 登录，否则直接导航。
 */
export async function loginAsAdmin(
  page: Page,
  options: {
    logger?: UnifiedLogger
  } = {}
): Promise<void> {
  const { logger } = options

  logger?.testCode.log(`[Auth] 登录管理员`) ?? console.warn(`[Auth] 登录管理员`)

  await clearSessionData(page)

  const config = await fetchAuthConfig()

  if (config.enabled && config.herald_login_url) {
    const heraldBaseUrl = config.herald_login_url.replace(/\/[^/]*\/auth\/login$/, '')
    await loginViaHeraldApi(page, heraldBaseUrl, { logger })
    // 导航到后台页面验证会话生效
    await page.goto(`${BASE_URL}/admin/devices`, { waitUntil: 'domcontentloaded' })
  } else {
    // 无认证模式，直接导航
    await page.goto(`${BASE_URL}/admin/devices`, { waitUntil: 'domcontentloaded' })
  }

  logger?.testCode.log(`[Auth] 登录完成`) ?? console.warn(`[Auth] 登录完成`)
}

/**
 * 登出当前用户
 */
export async function logout(page: Page, logger?: UnifiedLogger): Promise<void> {
  logger?.testCode.log('[Auth] 执行登出') ?? console.warn('[Auth] 执行登出')
  await clearSessionData(page)
  logger?.testCode.log('[Auth] 会话已清除') ?? console.warn('[Auth] 会话已清除')
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
