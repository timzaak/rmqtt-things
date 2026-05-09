/**
 * OTA Detail Page Object
 *
 * Encapsulates the OTA version detail view (/ota/show/:id).
 * US-PA-021: View OTA version details
 */

import { Page, Locator, expect } from '@playwright/test'
import { SELECTORS } from '../selectors'
import { BasePage } from './base-page'

export class OtaDetailPage extends BasePage {
  readonly heading: Locator
  readonly backLink: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'OTA Version Detail' })
    this.backLink = page.getByTestId(SELECTORS.ota.showBackLink.replace('[data-testid="', '').replace('"]', ''))
  }

  async gotoDetail(baseUrl: string, id: number): Promise<void> {
    await this.goto(`${baseUrl}/ota/show/${id}`)
  }

  async verifyDetailPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
    await expect(this.backLink).toBeVisible()
  }

  async verifyFieldValue(text: string): Promise<void> {
    await expect(this.page.getByText(text, { exact: true })).toBeVisible()
  }

  async verifyDashForEmptyFields(): Promise<void> {
    const dashes = this.page.locator('dd', { hasText: /^-$/ })
    await expect(dashes.first()).toBeVisible()
  }
}
