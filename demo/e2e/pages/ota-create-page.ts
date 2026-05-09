/**
 * OTA Create Page Object
 *
 * Encapsulates the OTA version creation form (/ota/create).
 * US-PA-011: Create OTA version
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'

export class OtaCreatePage extends BasePage {
  readonly heading: Locator
  readonly productSelect: Locator
  readonly keyInput: Locator
  readonly versionInput: Locator
  readonly minVersionInput: Locator
  readonly maxVersionInput: Locator
  readonly logTextarea: Locator
  readonly fileInput: Locator
  readonly deviceInput: Locator
  readonly addDeviceButton: Locator
  readonly createButton: Locator
  readonly cancelButton: Locator
  readonly firmwareFileLabel: Locator
  readonly toast: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Create OTA Version' })
    this.productSelect = page.getByRole('combobox', { name: 'Product *' })
    this.keyInput = page.getByRole('textbox', { name: 'Key *', exact: true })
    this.versionInput = page.getByRole('textbox', { name: 'Version *', exact: true })
    this.minVersionInput = page.getByRole('textbox', { name: 'Min Version *', exact: true })
    this.maxVersionInput = page.getByRole('textbox', { name: 'Max Version', exact: true })
    this.logTextarea = page.getByRole('textbox', { name: 'Log' })
    this.fileInput = page.locator('input[type="file"]')
    this.deviceInput = page.getByPlaceholder('Enter device ID and press Enter')
    this.addDeviceButton = page.getByRole('button', { name: 'Add', exact: true })
    this.createButton = page.getByRole('button', { name: 'Create' })
    this.cancelButton = page.getByRole('link', { name: 'Cancel' })
    this.firmwareFileLabel = page.getByText('Firmware File')
    this.toast = page.locator('[data-sonner-toast]').first()
  }

  async gotoCreate(baseUrl: string): Promise<void> {
    await this.goto(`${baseUrl}/ota/create`)
  }

  async verifyFormFields(): Promise<void> {
    await expect(this.heading).toBeVisible()
    await expect(this.productSelect).toBeVisible()
    await expect(this.keyInput).toBeVisible()
    await expect(this.versionInput).toBeVisible()
    await expect(this.minVersionInput).toBeVisible()
    await expect(this.firmwareFileLabel).toBeVisible()
    await expect(this.createButton).toBeVisible()
  }

  async addDeviceId(deviceId: string): Promise<void> {
    await this.deviceInput.fill(deviceId)
    await this.addDeviceButton.click()
    await expect(this.page.getByText(deviceId)).toBeVisible()
  }

  async submit(): Promise<void> {
    await this.createButton.click()
  }

  async verifyToastVisible(): Promise<void> {
    await expect(this.toast).toBeVisible()
  }
}
