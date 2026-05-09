/**
 * Login Page Object
 *
 * 封装登录页面操作。
 * 选择器定义在 selectors.ts 中，便于统一维护。
 *
 * 使用方式：
 * ```typescript
 * const loginPage = new LoginPage(page, logger)
 * await loginPage.goto()
 * await loginPage.login({ email: 'admin@example.com', password: 'password' })
 * ```
 */

import { Page, Locator, expect } from '@playwright/test'
import { SELECTORS } from '../selectors'
import { BasePage } from './base-page'
import type { UnifiedLogger } from 'playwright-unified-logger'

export interface LoginCredentials {
  email: string
  password: string
}

export class LoginPage extends BasePage {
  readonly container: Locator
  readonly title: Locator
  readonly emailInput: Locator
  readonly passwordInput: Locator
  readonly submitButton: Locator
  readonly errorMessage: Locator

  constructor(page: Page, logger?: UnifiedLogger) {
    super(page, logger)
    this.container = page.locator(SELECTORS.login.container)
    this.title = page.locator(SELECTORS.login.title)
    this.emailInput = page.locator(SELECTORS.login.emailInput)
    this.passwordInput = page.locator(SELECTORS.login.passwordInput)
    this.submitButton = page.locator(SELECTORS.login.submitButton)
    this.errorMessage = page.locator(SELECTORS.login.errorMessage)
  }

  /**
   * 导航到登录页
   */
  async goto(): Promise<void> {
    const BASE_URL = process.env.BASE_URL || 'http://localhost:8080'
    await this.page.goto(`${BASE_URL}/admin/login`, { waitUntil: 'domcontentloaded' })

    // 检查是否已登录并跳转
    const currentUrl = this.page.url()
    if (currentUrl.includes('/admin/dashboard') || currentUrl.includes('/admin/devices')) {
      return
    }

    await expect(this.container).toBeVisible()
  }

  async waitForReady(): Promise<void> {
    await expect(this.container).toBeVisible()
    await expect(this.title).toBeVisible()
    await expect(this.emailInput).toBeVisible()
    await expect(this.passwordInput).toBeVisible()
    await expect(this.submitButton).toBeVisible()
  }

  async fillLoginForm(credentials: LoginCredentials): Promise<void> {
    await this.fillField(this.emailInput, credentials.email)
    await this.fillField(this.passwordInput, credentials.password)
  }

  async submit(): Promise<void> {
    await this.smartClick(this.submitButton)
  }

  /**
   * 使用凭证登录
   */
  async login(credentials: LoginCredentials): Promise<void> {
    await this.fillLoginForm(credentials)
    await this.submit()

    const loginResponse = await this.page.waitForResponse(
      response => response.url().includes('/login') && response.request().method() === 'POST',
      { timeout: 10000 }
    ).catch(() => null)

    if (loginResponse && !loginResponse.ok()) {
      const errorBody = await loginResponse.text().catch(() => '')
      throw new Error(`Login failed: API returned ${loginResponse.status()} - ${errorBody}`)
    }
  }

  async getErrorMessage(): Promise<string> {
    const visible = await this.isVisible(this.errorMessage)
    if (!visible) return ''
    return await this.getText(this.errorMessage)
  }

  async hasError(): Promise<boolean> {
    return await this.isVisible(this.errorMessage)
  }

  async isOnLoginPage(): Promise<boolean> {
    const url = this.getUrl()
    return url.includes('/login') && await this.isVisible(this.container)
  }
}
