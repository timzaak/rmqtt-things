/**
 * Alarm Rule Create Page Object
 *
 * Encapsulates the alarm rule creation form (/alarm-rules/create).
 * User story: US-PA-029 (create alarm rule)
 */

import { Page, Locator, expect } from '@playwright/test'
import { BasePage } from './base-page'
import { SELECTORS } from '../selectors'

export class AlarmRuleCreatePage extends BasePage {
  readonly heading: Locator
  readonly productSelect: Locator
  readonly nameInput: Locator
  readonly descriptionInput: Locator
  readonly triggerTypeSelect: Locator
  readonly propertyNameInput: Locator
  readonly eventIdentifierInput: Locator
  readonly conditionOperatorSelect: Locator
  readonly conditionValueInput: Locator
  readonly conditionMinInput: Locator
  readonly conditionMaxInput: Locator
  readonly actionsEditor: Locator
  readonly addAlarmActionButton: Locator
  readonly addWebhookActionButton: Locator
  readonly throttleMinutesInput: Locator
  readonly submitButton: Locator
  readonly cancelButton: Locator

  constructor(page: Page) {
    super(page)
    this.heading = page.getByRole('heading', { name: 'Create Alarm Rule' })
    this.productSelect = page.locator(SELECTORS.alarmRules.productSelect)
    this.nameInput = page.locator(SELECTORS.alarmRules.nameInput)
    this.descriptionInput = page.locator(SELECTORS.alarmRules.descriptionInput)
    this.triggerTypeSelect = page.locator(SELECTORS.alarmRules.triggerTypeSelect)
    this.propertyNameInput = page.locator(SELECTORS.alarmRules.propertyNameInput)
    this.eventIdentifierInput = page.locator(SELECTORS.alarmRules.eventIdentifierInput)
    this.conditionOperatorSelect = page.locator(SELECTORS.alarmRules.conditionOperatorSelect)
    this.conditionValueInput = page.locator(SELECTORS.alarmRules.conditionValueInput)
    this.conditionMinInput = page.locator(SELECTORS.alarmRules.conditionMinInput)
    this.conditionMaxInput = page.locator(SELECTORS.alarmRules.conditionMaxInput)
    this.actionsEditor = page.locator(SELECTORS.alarmRules.actionsEditor)
    this.addAlarmActionButton = page.locator(SELECTORS.alarmRules.addAlarmActionButton)
    this.addWebhookActionButton = page.locator(SELECTORS.alarmRules.addWebhookActionButton)
    this.throttleMinutesInput = page.locator(SELECTORS.alarmRules.throttleMinutesInput)
    this.submitButton = page.locator(SELECTORS.alarmRules.submitButton)
    this.cancelButton = page.locator(SELECTORS.alarmRules.cancelButton)
  }

  async gotoCreate(baseUrl: string): Promise<void> {
    await this.goto(`${baseUrl}/alarm-rules/create`)
  }

  async verifyCreatePage(): Promise<void> {
    await expect(this.heading).toBeVisible()
  }

  async selectProduct(productId: string): Promise<void> {
    await this.productSelect.selectOption(productId)
  }

  async fillName(name: string): Promise<void> {
    await this.fillField(this.nameInput, name)
  }

  async fillDescription(desc: string): Promise<void> {
    await this.fillField(this.descriptionInput, desc)
  }

  async selectTriggerType(type: string): Promise<void> {
    await this.triggerTypeSelect.selectOption(type)
  }

  async fillPropertyName(name: string): Promise<void> {
    await this.fillField(this.propertyNameInput, name)
  }

  async fillEventIdentifier(id: string): Promise<void> {
    await this.fillField(this.eventIdentifierInput, id)
  }

  async selectConditionOperator(op: string): Promise<void> {
    await this.conditionOperatorSelect.selectOption(op)
  }

  async fillConditionValue(value: string): Promise<void> {
    await this.fillField(this.conditionValueInput, value)
  }

  async fillConditionMin(min: string): Promise<void> {
    await this.fillField(this.conditionMinInput, min)
  }

  async fillConditionMax(max: string): Promise<void> {
    await this.fillField(this.conditionMaxInput, max)
  }

  async fillActionMessage(index: number, message: string): Promise<void> {
    const input = this.page.locator(SELECTORS.alarmRules.actionMessageInput(index))
    await this.fillField(input, message)
  }

  async selectActionLevel(index: number, level: string): Promise<void> {
    const select = this.page.locator(SELECTORS.alarmRules.actionLevelSelect(index))
    await select.selectOption(level)
  }

  async fillThrottleMinutes(minutes: string): Promise<void> {
    await this.fillField(this.throttleMinutesInput, minutes)
  }

  async submit(): Promise<void> {
    await this.smartClick(this.submitButton)
  }

  async cancel(): Promise<void> {
    await this.smartClick(this.cancelButton)
  }
}
