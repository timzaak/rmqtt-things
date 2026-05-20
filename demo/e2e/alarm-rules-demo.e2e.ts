/**
 * Alarm Rules CRUD Demo Tests
 *
 * User stories: US-PA-029 (create), US-PA-030 (list), US-PA-031 (edit),
 *               US-PA-032 (toggle status), US-PA-033 (delete)
 *
 * Each test is self-contained: creates data via API, verifies UI, cleans up
 * via API in finally blocks. No test depends on data from another test.
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { AlarmRulesListPage } from './pages/alarm-rules-list-page'
import { AlarmRuleCreatePage } from './pages/alarm-rule-create-page'
import { AlarmRuleEditPage } from './pages/alarm-rule-edit-page'
import { createAlarmRule, deleteAlarmRule, getAlarmRules } from './helpers/alarm-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'

test.beforeEach(async ({ page, demoLogger }) => {
  await verifyTestEnvironment(page, { logger: demoLogger })
})

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Create a property threshold alarm rule via API with sensible defaults. */
async function createPropertyRule(
  request: Parameters<typeof createAlarmRule>[0],
  name: string,
  overrides: Record<string, unknown> = {},
) {
  return createAlarmRule(request, {
    product_id: PRODUCT_ID,
    name,
    description: `E2E test rule: ${name}`,
    trigger_type: 'property',
    trigger_config: { property_name: 'temperature' },
    condition: { operator: '>', value: 50 },
    actions: [{ type: 'alarm', level: 'warning', message: 'temperature exceeded threshold' }],
    throttle_minutes: 0,
    ...overrides,
  })
}

// ===========================================================================
// US-PA-030: View alarm rules list
// ===========================================================================

test.describe('Alarm rules list (US-PA-030)', () => {
  test('[US-PA-030] shows alarm rules list page with product filter', async ({ page }) => {
    const listPage = new AlarmRulesListPage(page)
    await listPage.gotoList(FRONTEND_URL)
    await listPage.verifyListPage()
    await listPage.verifyProductFilter()
  })

  test('[US-PA-030] alarm rules list displays created rules', async ({ request, page }) => {
    const ruleName = `E2E List Test Rule ${Date.now()}`
    const rule = await createPropertyRule(request, ruleName)
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.waitForRuleInList(ruleName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })

  test('[US-PA-030] filter rules by product', async ({ request, page }) => {
    const ruleName = `E2E Filter Test Rule ${Date.now()}`
    const rule = await createPropertyRule(request, ruleName)
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.selectProductFilter(PRODUCT_ID)
      await listPage.clickSearch()
      await listPage.waitForRuleInList(ruleName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })
})

// ===========================================================================
// US-PA-029: Create alarm rule
// ===========================================================================

test.describe('Create alarm rule (US-PA-029)', () => {
  test('[US-PA-029] create property threshold rule via form', async ({ request, page }) => {
    const ruleName = `E2E Property Threshold Rule ${Date.now()}`
    const ruleId: number[] = []
    try {
      const createPage = new AlarmRuleCreatePage(page)
      await createPage.gotoCreate(FRONTEND_URL)
      await createPage.verifyCreatePage()

      await createPage.selectProduct(PRODUCT_ID)
      await createPage.fillName(ruleName)
      await createPage.selectTriggerType('property')

      // Wait for conditional fields to appear after trigger type selection
      await expect(createPage.propertyNameInput).toBeVisible({ timeout: 5000 })
      await createPage.fillPropertyName('temperature')

      await createPage.selectConditionOperator('>')

      // Wait for condition value input to appear
      await expect(createPage.conditionValueInput).toBeVisible({ timeout: 5000 })
      await createPage.fillConditionValue('50')

      await createPage.fillActionMessage(0, 'temperature exceeded threshold')
      await createPage.selectActionLevel(0, 'warning')

      // Intercept the create API response to capture the rule ID
      const responsePromise = page.waitForResponse(
        resp => resp.url().includes('/api/admin/alarm-rule') && resp.request().method() === 'POST' && resp.status() === 201,
      )
      await createPage.submit()
      const response = await responsePromise
      const created = (await response.json()).data
      ruleId.push(created.id)

      // Wait for navigation back to list page
      await page.waitForURL('**/alarm-rules')

      // Verify the rule appears in the list
      const listPage = new AlarmRulesListPage(page)
      await listPage.waitForRuleInList(ruleName)
    } finally {
      for (const id of ruleId) {
        await deleteAlarmRule(request, id).catch(() => {})
      }
    }
  })

  test('[US-PA-029] create event trigger rule via API and verify in list', async ({ request, page }) => {
    const ruleName = `E2E Event Trigger Rule ${Date.now()}`
    const rule = await createAlarmRule(request, {
      product_id: PRODUCT_ID,
      name: ruleName,
      description: 'E2E event trigger test',
      trigger_type: 'event',
      trigger_config: { event_identifier: 'error_event' },
      condition: { operator: 'always' },
      actions: [{ type: 'alarm', level: 'critical', message: 'error event triggered' }],
      throttle_minutes: 0,
    })
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.waitForRuleInList(ruleName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })

  test('[US-PA-029] create device offline rule via API and verify', async ({ request, page }) => {
    const ruleName = `E2E Device Offline Rule ${Date.now()}`
    const rule = await createAlarmRule(request, {
      product_id: PRODUCT_ID,
      name: ruleName,
      description: 'E2E device offline test',
      trigger_type: 'device_offline',
      trigger_config: {},
      condition: { operator: 'always' },
      actions: [{ type: 'alarm', level: 'info', message: 'device went offline' }],
      throttle_minutes: 0,
    })
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.waitForRuleInList(ruleName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })
})

// ===========================================================================
// US-PA-031: Edit alarm rule
// ===========================================================================

test.describe('Edit alarm rule (US-PA-031)', () => {
  test('[US-PA-031] edit alarm rule -- product field read-only', async ({ request, page }) => {
    const originalName = `E2E Edit Test Rule ${Date.now()}`
    const editedName = `E2E Edited Rule Name ${Date.now()}`
    const rule = await createPropertyRule(request, originalName)
    try {
      const editPage = new AlarmRuleEditPage(page)
      await editPage.gotoEdit(FRONTEND_URL, rule.id)
      await editPage.verifyEditPage()

      // Product and trigger type must be read-only
      await editPage.verifyProductDisabled()
      await editPage.verifyTriggerTypeDisabled()

      // Name is editable
      await editPage.fillName(editedName)
      await editPage.submit()

      // Wait for redirect to list page
      await page.waitForURL('**/alarm-rules')

      // Verify updated name appears in the list
      const listPage = new AlarmRulesListPage(page)
      await listPage.waitForRuleInList(editedName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })
})

// ===========================================================================
// US-PA-032: Toggle alarm rule enabled status
// ===========================================================================

test.describe('Toggle alarm rule status (US-PA-032)', () => {
  test('[US-PA-032] toggle alarm rule enabled status', async ({ request, page }) => {
    const ruleName = `E2E Toggle Test Rule ${Date.now()}`
    const rule = await createPropertyRule(request, ruleName)
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.waitForRuleInList(ruleName)

      // Toggle to disable
      const switchEl = listPage.getEnabledSwitch(rule.id)
      await expect(switchEl).toBeVisible()
      await listPage.toggleEnabled(rule.id)

      // Wait for the switch state to update (checkbox unchecked)
      await expect(switchEl).not.toBeChecked({ timeout: 5000 })

      // Toggle back to enable
      await listPage.toggleEnabled(rule.id)

      // Wait for the switch state to update (checkbox checked)
      await expect(switchEl).toBeChecked({ timeout: 5000 })
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })
})

// ===========================================================================
// US-PA-033: Delete alarm rule
// ===========================================================================

test.describe('Delete alarm rule (US-PA-033)', () => {
  test('[US-PA-033] delete alarm rule with confirmation dialog', async ({ request, page }) => {
    const ruleName = `E2E Delete Test Rule ${Date.now()}`
    const rule = await createPropertyRule(request, ruleName)

    const listPage = new AlarmRulesListPage(page)
    await listPage.gotoList(FRONTEND_URL)
    await listPage.waitForRuleInList(ruleName)

    await listPage.clickDelete(ruleName)
    await listPage.confirmDelete()

    // Verify the rule no longer appears in the list
    await listPage.waitForRuleNotInList(ruleName)

    // Verify via API that the rule is gone
    const result = await getAlarmRules(request, { product_id: PRODUCT_ID })
    const stillExists = result.data.some(r => r.id === rule.id)
    expect(stillExists).toBe(false)
  })

  test('[US-PA-033] cancel delete preserves the rule', async ({ request, page }) => {
    const ruleName = `E2E Cancel Delete Rule ${Date.now()}`
    const rule = await createPropertyRule(request, ruleName)
    try {
      const listPage = new AlarmRulesListPage(page)
      await listPage.gotoList(FRONTEND_URL)
      await listPage.waitForRuleInList(ruleName)

      await listPage.clickDelete(ruleName)
      await listPage.cancelDelete()

      // Verify the rule still appears in the list
      await listPage.waitForRuleInList(ruleName)
    } finally {
      await deleteAlarmRule(request, rule.id)
    }
  })
})
