/**
 * Alarm Records List Page Object
 *
 * Encapsulates the alarm records list page (/alarms).
 * User stories: US-PA-034 (list/filter), US-PA-035 (acknowledge)
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'
import { SELECTORS } from '../selectors'

export class AlarmRecordsListPage extends BasePage {
  readonly heading: Locator
  readonly searchForm: Locator
  readonly table: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Alarm Records' })
    this.searchForm = page.locator(SELECTORS.alarms.searchForm)
    this.table = page.locator(SELECTORS.alarms.table)
  }

  async gotoList(baseUrl: string): Promise<void> {
    await this.goto(`${baseUrl}/alarms`)
  }

  async verifyListPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
  }

  async verifySearchFilters(): Promise<void> {
    await expect(this.searchForm).toBeVisible()
  }

  async selectProductFilter(productId: string): Promise<void> {
    const productSelect = this.searchForm.getByLabel('Product')
    await productSelect.selectOption(productId)
  }

  async selectLevelFilter(level: string): Promise<void> {
    const levelSelect = this.searchForm.getByLabel('Level')
    await levelSelect.selectOption(level)
  }

  async selectAcknowledgedFilter(value: string): Promise<void> {
    const acknowledgedSelect = this.searchForm.getByLabel('Acknowledged')
    await acknowledgedSelect.selectOption(value)
  }

  async clickSearch(): Promise<void> {
    const searchButton = this.searchForm.getByRole('button', { name: 'Search' })
    await this.smartClick(searchButton)
  }

  getAckButton(alarmId: number): Locator {
    return this.page.locator(SELECTORS.alarms.ackButton(alarmId))
  }

  getAcknowledgedTag(alarmId: number): Locator {
    return this.page.locator(SELECTORS.alarms.acknowledgedTag(alarmId))
  }

  async acknowledgeAlarm(alarmId: number): Promise<void> {
    const ackButton = this.getAckButton(alarmId)
    await this.smartClick(ackButton)
  }

  async waitForAlarmInList(ruleName: string): Promise<void> {
    const row = this.table.locator('tr', { hasText: ruleName })
    await expect(row).toBeVisible({ timeout: 10000 })
  }
}
