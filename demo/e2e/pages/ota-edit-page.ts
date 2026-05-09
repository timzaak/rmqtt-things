/**
 * OTA Edit Page Object
 *
 * Encapsulates the OTA version edit form (/ota/edit/:id).
 * US-PA-013: Edit OTA version
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'

export class OtaEditPage extends BasePage {
  readonly heading: Locator
  readonly minVersionInput: Locator
  readonly saveButton: Locator
  readonly cancelButton: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Edit OTA Version' })
    this.minVersionInput = page.getByRole('textbox', { name: 'Min Version *', exact: true })
    this.saveButton = page.getByRole('button', { name: 'Save' })
    this.cancelButton = page.getByRole('link', { name: 'Cancel' })
  }

  async gotoEdit(baseUrl: string, id: number): Promise<void> {
    await this.goto(`${baseUrl}/ota/edit/${id}`)
  }

  async verifyEditPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
  }

  async verifyDisabledFields(count: number): Promise<void> {
    const disabledInputs = this.page.locator('input[disabled]')
    await expect(disabledInputs).toHaveCount(count)
  }

  async verifyMinVersionEditable(): Promise<void> {
    await expect(this.minVersionInput).toBeVisible()
    await expect(this.minVersionInput).toBeEditable()
  }
}
