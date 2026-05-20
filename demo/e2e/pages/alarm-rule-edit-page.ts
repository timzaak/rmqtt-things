/**
 * Alarm Rule Edit Page Object
 *
 * Encapsulates the alarm rule edit form (/alarm-rules/edit/:id).
 * User story: US-PA-031 (edit alarm rule)
 *
 * Product and trigger type fields are disabled (read-only) on the edit page.
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'
import { SELECTORS } from '../selectors'

export class AlarmRuleEditPage extends BasePage {
  readonly heading: Locator
  readonly productInputDisabled: Locator
  readonly triggerTypeInputDisabled: Locator
  readonly nameInput: Locator
  readonly submitButton: Locator
  readonly cancelButton: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Edit Alarm Rule' })
    this.productInputDisabled = page.locator(SELECTORS.alarmRules.productInputDisabled)
    this.triggerTypeInputDisabled = page.locator(SELECTORS.alarmRules.triggerTypeInputDisabled)
    this.nameInput = page.locator(SELECTORS.alarmRules.nameInput)
    this.submitButton = page.locator(SELECTORS.alarmRules.submitButton)
    this.cancelButton = page.locator(SELECTORS.alarmRules.cancelButton)
  }

  async gotoEdit(baseUrl: string, id: number): Promise<void> {
    await this.goto(`${baseUrl}/alarm-rules/edit/${id}`)
  }

  async verifyEditPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
  }

  async verifyProductDisabled(): Promise<void> {
    await expect(this.productInputDisabled).toBeVisible()
    await expect(this.productInputDisabled).toBeDisabled()
  }

  async verifyTriggerTypeDisabled(): Promise<void> {
    await expect(this.triggerTypeInputDisabled).toBeVisible()
    await expect(this.triggerTypeInputDisabled).toBeDisabled()
  }

  async fillName(name: string): Promise<void> {
    await this.fillField(this.nameInput, name)
  }

  async submit(): Promise<void> {
    await this.smartClick(this.submitButton)
  }

  async cancel(): Promise<void> {
    await this.smartClick(this.cancelButton)
  }
}
