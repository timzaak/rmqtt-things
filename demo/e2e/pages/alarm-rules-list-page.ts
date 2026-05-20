/**
 * Alarm Rules List Page Object
 *
 * Encapsulates the alarm rules list page (/alarm-rules).
 * User stories: US-PA-030 (list), US-PA-032 (toggle), US-PA-033 (delete)
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'
import { SELECTORS } from '../selectors'

export class AlarmRulesListPage extends BasePage {
  readonly heading: Locator
  readonly createButton: Locator
  readonly searchForm: Locator
  readonly table: Locator
  readonly deleteConfirmDialog: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Alarm Rules' })
    this.createButton = page.locator(SELECTORS.alarmRules.createButton)
    this.searchForm = page.locator(SELECTORS.alarmRules.searchForm)
    this.table = page.locator(SELECTORS.alarmRules.table)
    this.deleteConfirmDialog = page.locator(SELECTORS.alarmRules.deleteConfirmDialog)
  }

  async gotoList(baseUrl: string): Promise<void> {
    await this.goto(`${baseUrl}/alarm-rules`)
  }

  async verifyListPage(): Promise<void> {
    await expect(this.heading).toBeVisible()
    await expect(this.createButton).toBeVisible()
  }

  async verifyProductFilter(): Promise<void> {
    await expect(this.searchForm).toBeVisible()
  }

  async selectProductFilter(productId: string): Promise<void> {
    const productSelect = this.searchForm.getByLabel('Product')
    await productSelect.selectOption(productId)
  }

  async clickSearch(): Promise<void> {
    const searchButton = this.searchForm.getByRole('button', { name: 'Search' })
    await this.smartClick(searchButton)
  }

  async navigateToCreate(): Promise<void> {
    await this.smartClick(this.createButton)
  }

  getEnabledSwitch(ruleId: number): Locator {
    return this.page.locator(SELECTORS.alarmRules.enabledSwitch(ruleId))
  }

  async toggleEnabled(ruleId: number): Promise<void> {
    const switchEl = this.getEnabledSwitch(ruleId)
    await this.smartClick(switchEl)
  }

  async clickEdit(ruleId: number): Promise<void> {
    const editLink = this.table.locator(`a[href="/alarm-rules/edit/${ruleId}"]`)
    await this.smartClick(editLink)
  }

  async clickDelete(ruleName: string): Promise<void> {
    // Find the row containing the rule name, then click its Delete button
    const row = this.table.locator('tr', { hasText: ruleName })
    const deleteButton = row.getByRole('button', { name: 'Delete' })
    await this.smartClick(deleteButton)
  }

  async confirmDelete(): Promise<void> {
    await expect(this.page.getByRole('heading', { name: 'Delete Alarm Rule' })).toBeVisible()
    const buttons = this.page.locator('[data-testid="delete-confirm-dialog"] button')
    const confirmButton = buttons.filter({ hasText: 'Delete' }).last()
    await this.smartClick(confirmButton)
  }

  async cancelDelete(): Promise<void> {
    await expect(this.page.getByRole('heading', { name: 'Delete Alarm Rule' })).toBeVisible()
    const cancelButton = this.page.getByRole('button', { name: 'Cancel' })
    await this.smartClick(cancelButton)
  }

  async waitForRuleInList(ruleName: string): Promise<void> {
    const row = this.table.locator('tr', { hasText: ruleName })
    await expect(row).toBeVisible({ timeout: 10000 })
  }

  async waitForRuleNotInList(ruleName: string): Promise<void> {
    const row = this.table.locator('tr', { hasText: ruleName })
    await expect(row).toBeHidden({ timeout: 10000 })
  }
}
