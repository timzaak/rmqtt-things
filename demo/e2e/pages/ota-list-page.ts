/**
 * OTA List Page Object
 *
 * Encapsulates the OTA versions list page (/ota).
 * US-PA-012: View OTA version list
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'

export class OtaListPage extends BasePage {
  readonly heading: Locator
  readonly createLink: Locator
  readonly productFilterLabel: Locator
  readonly searchButton: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'OTA Versions' })
    this.createLink = page.getByRole('link', { name: 'Create OTA Version' })
    this.productFilterLabel = page.locator('form label', { hasText: 'Product' })
    this.searchButton = page.getByRole('button', { name: 'Search' })
  }

  async gotoList(baseUrl: string): Promise<void> {
    await this.goto(`${baseUrl}/ota`)
  }

  async verifyListPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
    await expect(this.createLink).toBeVisible()
  }

  async verifyProductFilter(): Promise<void> {
    await expect(this.productFilterLabel).toBeVisible()
    await expect(this.searchButton).toBeVisible()
  }

  async navigateToCreate(): Promise<void> {
    await this.createLink.click()
  }
}
