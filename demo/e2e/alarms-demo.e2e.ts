/**
 * Alarm Records List & Acknowledge Demo Tests
 *
 * User stories: US-PA-034 (view/filter alarm records), US-PA-035 (acknowledge alarm),
 *               US-PA-040 (lifecycle status), US-PA-041 (manual clear)
 *
 * Each test is self-contained: creates a rule via API, triggers an alarm via MQTT,
 * verifies the UI, then cleans up the rule in a finally block.
 * Alarm records cannot be deleted, so tests use timestamp-based unique identifiers
 * and verify SPECIFIC alarms by rule name to avoid false positives from prior runs.
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { AlarmRecordsListPage } from './pages/alarm-records-list-page'
import {
  createAlarmRule,
  deleteAlarmRule,
  getAlarmRecords,
  acknowledgeAlarm,
  clearAlarm,
  type AlarmRecordResponse,
} from './helpers/alarm-api'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { findSeedProductId, getProduct, updateProduct } from './helpers/product-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 30_000

test.beforeEach(async ({ page, demoLogger }) => {
  await verifyTestEnvironment(page, { logger: demoLogger })
})

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TriggerAlarmResult {
  ruleId: number
  ruleName: string
  deviceId: string
  alarmId: number
  alarm: AlarmRecordResponse
}

async function triggerAlarmViaMqtt(
  request: Parameters<typeof createAlarmRule>[0],
  config: {
    property_name?: string
    threshold?: number
    value?: number
    level?: string
  } = {},
): Promise<TriggerAlarmResult> {
  const propertyName = config.property_name || 'temperature'
  const threshold = config.threshold ?? 50
  const value = config.value ?? 75
  const level = config.level || 'warning'

  const ts = Date.now()
  const deviceId = `demo-e2e-alarm-${ts}`
  const ruleName = `E2E Alarm Test ${ts}`

  const rule = await createAlarmRule(request, {
    product_id: PRODUCT_ID,
    name: ruleName,
    description: `E2E triggered alarm for ${propertyName}`,
    trigger_type: 'property',
    trigger_config: { property_name: propertyName },
    condition: { operator: '>', value: threshold },
    actions: [{ type: 'alarm', level, message: `${propertyName} exceeded` }],
    throttle_minutes: 0,
  })

  const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
  await device.connect()
  try {
    await device.postProperties({ [propertyName]: value })
  } finally {
    await device.disconnect()
  }

  // Poll for alarm to appear
  const deadline = Date.now() + POLL_TIMEOUT
  let alarm: AlarmRecordResponse | undefined
  while (Date.now() < deadline) {
    const records = await getAlarmRecords(request, {
      product_id: PRODUCT_ID,
      device_id: deviceId,
      page: 1,
      page_size: 10,
    })
    const match = records.data.find(a => a.rule_id === rule.id)
    if (match) {
      alarm = match
      break
    }
    await new Promise(resolve => setTimeout(resolve, 1000))
  }

  if (!alarm) {
    throw new Error(
      `Alarm not found within ${POLL_TIMEOUT}ms for rule ${ruleName} (ruleId=${rule.id}, deviceId=${deviceId})`,
    )
  }

  return { ruleId: rule.id, ruleName, deviceId, alarmId: alarm.id, alarm }
}

/**
 * Temporarily enables auto_provisioning on the seed product so that
 * dynamically-created MQTT devices can connect and trigger alarms.
 * Restores the original value in the finally block.
 */
async function withAutoProvisioning<T>(
  request: Parameters<typeof createAlarmRule>[0],
  body: () => Promise<T>,
): Promise<T> {
  const productId = await findSeedProductId(request)
  const original = await getProduct(request, productId)
  const originalAutoProv = original.auto_provisioning
  try {
    if (!originalAutoProv) {
      await updateProduct(request, productId, {
        name: original.name,
        description: original.description || '',
        auto_provisioning: true,
      })
    }
    return await body()
  } finally {
    if (!originalAutoProv) {
      await updateProduct(request, productId, {
        name: original.name,
        description: original.description || '',
        auto_provisioning: originalAutoProv,
      })
    }
  }
}

/**
 * Wrapper that triggers an alarm via MQTT and handles cleanup + ECONNREFUSED skip.
 * Enables auto_provisioning around the MQTT connection so the dynamic device can register.
 * Runs the test body with the TriggerAlarmResult, then deletes the rule in finally.
 * Skips the test if the MQTT broker is unreachable.
 */
async function withTriggeredAlarm(
  request: Parameters<typeof createAlarmRule>[0],
  config: Parameters<typeof triggerAlarmViaMqtt>[1],
  body: (result: TriggerAlarmResult) => Promise<void>,
): Promise<void> {
  let ruleId: number | undefined
  try {
    const result = await withAutoProvisioning(request, () => triggerAlarmViaMqtt(request, config))
    ruleId = result.ruleId
    await body(result)
  } catch (err) {
    if (err instanceof Error && (err.message.includes('ECONNREFUSED') || err.message.includes('Bad username or password'))) {
      test.skip()
    }
    throw err
  } finally {
    if (ruleId !== undefined) {
      await deleteAlarmRule(request, ruleId).catch(() => {})
    }
  }
}

// ===========================================================================
// US-PA-034: View alarm records
// ===========================================================================

test.describe('Alarm records list (US-PA-034)', () => {
  test('[US-PA-034] shows alarm records list page with search filters', async ({ page }) => {
    const recordsPage = new AlarmRecordsListPage(page)
    await recordsPage.gotoList(FRONTEND_URL)
    await recordsPage.verifyListPage()
    await recordsPage.verifySearchFilters()
    await expect(recordsPage.table).toBeVisible()
  })

  test('[US-PA-034] alarm records display after rule triggers', async ({ request, page }) => {
    await withTriggeredAlarm(request, {}, async (result) => {
      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)
      await recordsPage.verifyListPage()
      await recordsPage.waitForAlarmInList(result.ruleName)
    })
  })

  test('[US-PA-034] filter alarm records by product', async ({ request, page }) => {
    await withTriggeredAlarm(request, {}, async (result) => {
      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)
      await recordsPage.selectProductFilter(PRODUCT_ID)
      await recordsPage.clickSearch()
      await recordsPage.waitForAlarmInList(result.ruleName)
    })
  })

  test('[US-PA-034] filter alarm records by acknowledged status', async ({ request, page }) => {
    await withTriggeredAlarm(request, {}, async (result) => {
      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)
      await recordsPage.selectAcknowledgedFilter('false')
      await recordsPage.clickSearch()
      await recordsPage.waitForAlarmInList(result.ruleName)
    })
  })

  test('[US-PA-034] filter alarm records by level', async ({ request, page }) => {
    await withTriggeredAlarm(request, { level: 'warning' }, async (result) => {
      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)
      await recordsPage.selectLevelFilter('warning')
      await recordsPage.clickSearch()
      await recordsPage.waitForAlarmInList(result.ruleName)
    })
  })
})

// ===========================================================================
// US-PA-035: Acknowledge alarm
// ===========================================================================

test.describe('Acknowledge alarm (US-PA-035)', () => {
  test('[US-PA-035] acknowledge alarm from list page', async ({ request, page }) => {
    await withTriggeredAlarm(request, {}, async (result) => {
      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)

      const ackButton = recordsPage.getAckButton(result.alarmId)
      await expect(ackButton).toBeVisible({ timeout: 10_000 })

      await recordsPage.acknowledgeAlarm(result.alarmId)

      const ackTag = recordsPage.getAcknowledgedTag(result.alarmId)
      await expect(ackTag).toBeVisible({ timeout: 5_000 })
      await expect(ackButton).toBeHidden({ timeout: 5_000 })
    })
  })

  test('[US-PA-035] already-acknowledged alarm hides acknowledge button', async ({ request, page }) => {
    await withTriggeredAlarm(request, {}, async (result) => {
      await acknowledgeAlarm(request, result.alarmId)

      const recordsPage = new AlarmRecordsListPage(page)
      await recordsPage.gotoList(FRONTEND_URL)

      const ackTag = recordsPage.getAcknowledgedTag(result.alarmId)
      await expect(ackTag).toBeVisible({ timeout: 10_000 })

      const ackButton = recordsPage.getAckButton(result.alarmId)
      await expect(ackButton).toBeHidden({ timeout: 5_000 })
    })
  })

  // ===========================================================================
  // US-PA-040: Lifecycle status (Active/Acknowledged/Cleared)
  // ===========================================================================

  test.describe('Alarm lifecycle status (US-PA-040)', () => {
    test('[US-PA-040] active alarm shows Active status tag', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)
        await recordsPage.waitForAlarmInList(result.ruleName)

        const statusTag = recordsPage.getStatusTag(result.alarmId)
        await expect(statusTag).toBeVisible({ timeout: 10_000 })
        await expect(statusTag).toHaveText(/active/i, { timeout: 5_000 })
      })
    })

    test('[US-PA-040] acknowledged alarm shows Acknowledged status tag', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        await acknowledgeAlarm(request, result.alarmId)

        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)
        await recordsPage.waitForAlarmInList(result.ruleName)

        const statusTag = recordsPage.getStatusTag(result.alarmId)
        await expect(statusTag).toBeVisible({ timeout: 10_000 })
        await expect(statusTag).toHaveText(/acknowledged/i, { timeout: 5_000 })

        // Ack button should be hidden for acknowledged alarms
        const ackButton = recordsPage.getAckButton(result.alarmId)
        await expect(ackButton).toBeHidden({ timeout: 5_000 })

        // Clear button should still be visible
        const clearButton = recordsPage.getClearButton(result.alarmId)
        await expect(clearButton).toBeVisible({ timeout: 5_000 })
      })
    })

    test('[US-PA-040] filter alarm records by status', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)

        // Filter by active status
        await recordsPage.selectStatusFilter('active')
        await recordsPage.clickSearch()
        await recordsPage.waitForAlarmInList(result.ruleName)
      })
    })
  })

  // ===========================================================================
  // US-PA-041: Manual clear alarm
  // ===========================================================================

  test.describe('Manual clear alarm (US-PA-041)', () => {
    test('[US-PA-041] clear active alarm from list page', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)
        await recordsPage.waitForAlarmInList(result.ruleName)

        const clearButton = recordsPage.getClearButton(result.alarmId)
        await expect(clearButton).toBeVisible({ timeout: 10_000 })

        await recordsPage.clearAlarm(result.alarmId)

        // Verify status changed to Cleared
        const statusTag = recordsPage.getStatusTag(result.alarmId)
        await expect(statusTag).toHaveText(/cleared/i, { timeout: 5_000 })

        // Cleared alarm should not have ack or clear buttons
        await expect(recordsPage.getAckButton(result.alarmId)).toBeHidden({ timeout: 5_000 })
        await expect(recordsPage.getClearButton(result.alarmId)).toBeHidden({ timeout: 5_000 })
      })
    })

    test('[US-PA-041] clear acknowledged alarm from list page', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        // First acknowledge, then clear
        await acknowledgeAlarm(request, result.alarmId)

        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)
        await recordsPage.waitForAlarmInList(result.ruleName)

        const clearButton = recordsPage.getClearButton(result.alarmId)
        await expect(clearButton).toBeVisible({ timeout: 10_000 })

        await recordsPage.clearAlarm(result.alarmId)

        // Verify status changed to Cleared
        const statusTag = recordsPage.getStatusTag(result.alarmId)
        await expect(statusTag).toHaveText(/cleared/i, { timeout: 5_000 })
      })
    })

    test('[US-PA-041] clear alarm via API and verify in UI', async ({ request, page }) => {
      await withTriggeredAlarm(request, {}, async (result) => {
        await clearAlarm(request, result.alarmId)

        const recordsPage = new AlarmRecordsListPage(page)
        await recordsPage.gotoList(FRONTEND_URL)
        await recordsPage.waitForAlarmInList(result.ruleName)

        const statusTag = recordsPage.getStatusTag(result.alarmId)
        await expect(statusTag).toBeVisible({ timeout: 10_000 })
        await expect(statusTag).toHaveText(/cleared/i, { timeout: 5_000 })

        // No action buttons for cleared alarms
        await expect(recordsPage.getAckButton(result.alarmId)).toBeHidden({ timeout: 5_000 })
        await expect(recordsPage.getClearButton(result.alarmId)).toBeHidden({ timeout: 5_000 })
      })
    })
  })
})
