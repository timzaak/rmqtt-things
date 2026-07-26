/**
 * Device Detail Experience Demo 测试
 *
 * 对应用户故事（已发布来源；设计文档引用的 draft
 * `.ai/user-stories/core/device-detail-experience.md` 当前不存在，以已发布基线为准）：
 * - US-PA-050 按业务意图理解设备详情
 *   （docs/user-stories/01-platform-admin-user-stories.md:1651）
 * - US-PA-051 区分目标同步、直接属性写入和动作调用
 *   （docs/user-stories/01-platform-admin-user-stories.md:1678）
 *
 * 覆盖映射：
 * - Scenario 1 -> US-PA-050 场景 1/2：七区导航可见 + State & Configuration 的
 *   Current/Target/Sync 对照（In sync / Out of sync / Target not set，设计 §4.4.3）
 * - Scenario 2 -> US-PA-051 场景 2：Set Desired 产生 Target sync 记录，
 *   Direct property write 与 Action invocation 记录可区分，类型筛选生效
 * - Scenario 3 -> US-PA-051 场景 4（核心）：设备回复成功（Operations 中 Success）
 *   但 delta 未收敛时，State 页仍显示 Out of sync，两者同时存在
 * - Scenario 4 -> US-PA-051 场景 3：Direct write 与 Target 冲突时对话框展示固定
 *   警告文案（DirectPropertyWriteDialog.tsx:8-9，FE-A01 handoff）
 *
 * 关键断言均落在持久业务状态（GET /api/admin/property/shadow 的 desired/delta/
 * reported，GET /api/admin/device/operation 的 operationType/status，表格行、
 * 状态徽标、对话框固定文案），不以 sonner/toast 为唯一验收依据。
 *
 * 前置条件：系统中已有产品 "demo_product"。
 * 前置条件：RMQTT broker 运行在 MQTT_URL (默认 mqtt://127.0.0.1:1883)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 * 前置条件：前端运行在 FRONTEND_URL (默认 http://localhost:3000)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { getJson } from './helpers/api'
import { findSeedProductId, getProduct, updateProduct } from './helpers/product-api'
import { verifyTestEnvironment } from './helpers/environment-setup'
import { SELECTORS } from './selectors'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 15_000

/** Direct write 与 Target 冲突时的固定警告文案（DirectPropertyWriteDialog.tsx:8-9）。 */
const TARGET_CONFLICT_WARNING =
  'This one-time write does not change Target and may leave the device out of sync.'

interface ListResponse<T> {
  data?: T[]
}

interface ShadowView {
  desired?: Record<string, unknown>
  reported?: Record<string, unknown>
  delta?: Record<string, unknown>
}

/** 统一操作视图行（设计 §5.1 DeviceOperationView，camelCase）。 */
interface DeviceOperationRow {
  operationId?: string
  operationType?: string
  name?: string
  status?: string
}

/** 导航到设备详情页并切换到 State & Configuration tab。 */
async function openStateConfigurationTab(
  page: import('@playwright/test').Page,
  deviceId: string,
): Promise<void> {
  // waitUntil:'commit' 在收到响应头即返回，避免 Vite dev server 下
  // 'load'/'domcontentloaded' 偶发不触发导致 30s 超时（DE-TR01 复现）。
  await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`, {
    waitUntil: 'commit',
  })
  await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  await page.locator(SELECTORS.deviceTabs.stateConfigurationTab).click()
  // 分区标题是 State & Configuration 面板的持久锚点
  await expect(page.getByRole('heading', { name: 'State & Configuration' })).toBeVisible()
}

/** 导航到设备详情页并切换到 Operations tab。 */
async function openOperationsTab(
  page: import('@playwright/test').Page,
  deviceId: string,
): Promise<void> {
  // waitUntil:'commit'（同 openStateConfigurationTab 说明）。
  await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`, {
    waitUntil: 'commit',
  })
  await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
  await page.locator(SELECTORS.deviceTabs.operationsTab).click()
  await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
}

test.describe('Device Detail Experience (US-PA-050/051)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  // ---------------------------------------------------------------------------
  // Scenario 1 — US-PA-050 七区导航 + Current/Target/Sync 状态映射
  // ---------------------------------------------------------------------------
  test('[Scenario 1] US-PA-050 seven-zone navigation and Current/Target/Sync state mapping', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    // 设备为全新、未注册的 deviceId，需要先开启 auto_provisioning 才能通过认证
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-dde-nav-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 造数（覆盖设计 §4.4.3 三种 Sync 映射）：
      // power       -> desired=reported       -> In sync
      // brightness  -> desired 存在但设备未上报 -> Out of sync
      // temperature -> 仅 reported            -> Target not set
      const syncDesired = { power: true }
      const syncCommandPromise = device.waitForCommand()
      const putSync = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: syncDesired },
      })
      expect(putSync.status()).toBe(200)
      const syncCommand = await syncCommandPromise
      await device.replyCommand(syncCommand)
      await device.postProperties(syncDesired)

      const driftDesired = { brightness: 80 }
      const driftCommandPromise = device.waitForCommand()
      const putDrift = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: driftDesired },
      })
      expect(putDrift.status()).toBe(200)
      // 设备收到 delta 命令但不回复、不上报，使 brightness 持续偏离
      await driftCommandPromise

      await device.postProperties({ temperature: 22 })

      // 主断言（持久状态）：shadow 视图三种映射的输入条件已就绪
      await expect.poll(
        async () => {
          const body = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return {
            powerInDelta: 'power' in (body.delta ?? {}),
            brightnessInDelta: 'brightness' in (body.delta ?? {}),
            temperatureReported: 'temperature' in (body.reported ?? {}),
          }
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual({ powerInDelta: false, brightnessInDelta: true, temperatureReported: true })

      // US-PA-050 场景 1：页面按七个业务区域组织，导航全部可见
      // （waitUntil:'commit'，原因同 openStateConfigurationTab）
      await page.goto(`${FRONTEND_URL}/devices/show/${deviceId}`, {
        waitUntil: 'commit',
      })
      await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()
      await expect(page.getByRole('tab', { name: SELECTORS.deviceTabs.overviewTab })).toBeVisible()
      await expect(page.locator(SELECTORS.deviceTabs.stateConfigurationTab)).toBeVisible()
      await expect(page.locator(SELECTORS.deviceTabs.operationsTab)).toBeVisible()
      await expect(page.locator(SELECTORS.deviceTabs.reportedDataTab)).toBeVisible()
      await expect(page.getByRole('tab', { name: SELECTORS.deviceTabs.eventsTab })).toBeVisible()
      await expect(
        page.getByRole('tab', { name: SELECTORS.deviceTabs.connectivityTab }),
      ).toBeVisible()
      await expect(page.getByRole('tab', { name: SELECTORS.deviceTabs.metadataTab })).toBeVisible()

      // US-PA-050 场景 1/2：State & Configuration 逐属性展示 Current/Target/Sync，
      // 并说明区域用途（设计 §4.4.2 副标题）
      await page.locator(SELECTORS.deviceTabs.stateConfigurationTab).click()
      await expect(page.getByRole('heading', { name: 'State & Configuration' })).toBeVisible()
      await expect(
        page.getByText('Persistent target; changes are applied once and are not auto-retried.'),
      ).toBeVisible()

      const table = page.locator(SELECTORS.stateConfiguration.table)
      await expect(table).toBeVisible({ timeout: POLL_TIMEOUT })
      await expect(table.getByRole('columnheader', { name: 'Current' })).toBeVisible()
      await expect(table.getByRole('columnheader', { name: 'Target' })).toBeVisible()
      await expect(table.getByRole('columnheader', { name: 'Sync' })).toBeVisible()

      // 三种 Sync 映射同时可见（行内断言，非 toast）
      const powerRow = table.locator('tr', { hasText: 'power' })
      await expect(powerRow.getByText('In sync', { exact: true })).toBeVisible()
      const brightnessRow = table.locator('tr', { hasText: 'brightness' })
      await expect(brightnessRow.getByText('Out of sync', { exact: true })).toBeVisible()
      const temperatureRow = table.locator('tr', { hasText: 'temperature' })
      // temperature 行 Target 列与 Sync 列均为 "Target not set"，取首个即可
      await expect(temperatureRow.getByText('Target not set', { exact: true }).first()).toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 2 — US-PA-051 三类操作可区分且类型筛选生效
  // ---------------------------------------------------------------------------
  test('[Scenario 2] US-PA-051 target sync, direct write and action are distinguishable and filterable', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-dde-ops-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()
      await device.subscribeAction('reboot')

      // 1) 经 UI Set Desired（Update Target）产生 targetSync 记录
      await openStateConfigurationTab(page, deviceId)
      await page.locator(SELECTORS.stateConfiguration.targetUpdateButton).click()
      const targetDialog = page.locator(SELECTORS.stateConfiguration.updateDialog)
      await expect(targetDialog).toBeVisible()
      const desiredPayload = { brightness: 80 }
      await targetDialog
        .locator(SELECTORS.stateConfiguration.targetJsonInput)
        .fill(JSON.stringify(desiredPayload))
      // 在 submit 前挂 waiter，避免竞态丢失 delta 命令
      const syncCommandPromise = device.waitForCommand()
      await targetDialog.locator(SELECTORS.stateConfiguration.dialogSubmitButton).click()
      await expect(targetDialog).not.toBeVisible({ timeout: POLL_TIMEOUT })
      const syncCommand = await syncCommandPromise
      expect(syncCommand.params).toMatchObject(desiredPayload)
      await device.replyCommand(syncCommand)

      // 2) 经 UI Direct property write 产生 directPropertyWrite 记录。
      // 已在设备详情页（上一步 openStateConfigurationTab），不再整页 reload；
      // 直接切 Operations tab（DE-TR01 修复：连续 page.goto 在同一用例内偶发卡死）。
      await page.locator(SELECTORS.deviceTabs.operationsTab).click()
      await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
      await page.locator(SELECTORS.operations.moreActionsButton).click()
      await page.locator(SELECTORS.operations.directPropertyWriteButton).click()
      const directDialog = page.locator(SELECTORS.operations.directWriteDialog)
      await expect(directDialog).toBeVisible()
      const directPayload = { brightness: 50 }
      await directDialog
        .locator(SELECTORS.operations.directWriteJsonInput)
        .fill(JSON.stringify(directPayload))
      const directCommandPromise = device.waitForCommand()
      await directDialog.locator(SELECTORS.operations.dialogSubmitButton).click()
      await expect(directDialog).not.toBeVisible({ timeout: POLL_TIMEOUT })
      const directCommand = await directCommandPromise
      expect(directCommand.params).toMatchObject(directPayload)
      await device.replyCommand(directCommand)

      // 3) 经 UI Run Action 产生 actionInvocation 记录
      await page.locator(SELECTORS.operations.runActionButton).click()
      const actionDialog = page.locator(SELECTORS.operations.actionDialog)
      await expect(actionDialog).toBeVisible()
      const actionParams = { delaySeconds: 5 }
      await actionDialog.locator(SELECTORS.operations.actionServiceTypeInput).fill('reboot')
      await actionDialog
        .locator(SELECTORS.operations.actionParamsInput)
        .fill(JSON.stringify(actionParams))
      // 在 submit 前挂 waitForAction，避免 eager publish 竞态
      const actionPromise = device.waitForAction()
      await actionDialog.locator(SELECTORS.operations.dialogSubmitButton).click()
      await expect(actionDialog).not.toBeVisible({ timeout: POLL_TIMEOUT })
      const action = await actionPromise
      expect(action.params).toMatchObject(actionParams)
      await device.replyAction(action, 200)

      // 主断言（持久状态）：三类操作在统一视图中并存且类型各异
      await expect.poll(
        async () => {
          const body = await getJson<ListResponse<DeviceOperationRow>>(
            request,
            `/api/admin/device/operation?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
          )
          return (body.data ?? []).map((row) => row.operationType).sort()
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual(['actionInvocation', 'directPropertyWrite', 'targetSync'])

      // US-PA-051 场景 2：全部类型下三种 Type 文案同时可区分
      const table = page.locator(SELECTORS.operations.table)
      await expect(table.getByText('Target sync', { exact: true })).toBeVisible({
        timeout: POLL_TIMEOUT,
      })
      await expect(table.getByText('Direct write', { exact: true })).toBeVisible()
      await expect(table.getByText('Action', { exact: true })).toBeVisible()
      await expect(table.getByText('reboot', { exact: true })).toBeVisible()

      // 类型筛选生效：targetSync 仅见 Target sync 行
      await page.locator(SELECTORS.operations.typeFilter).selectOption('targetSync')
      await expect(table.getByText('Direct write', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Action', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Target sync', { exact: true })).toBeVisible()

      // directPropertyWrite 仅见 Direct write 行
      await page.locator(SELECTORS.operations.typeFilter).selectOption('directPropertyWrite')
      await expect(table.getByText('Target sync', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Action', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Direct write', { exact: true })).toBeVisible()

      // actionInvocation 仅见 Action 行（reboot 为动作名）
      await page.locator(SELECTORS.operations.typeFilter).selectOption('actionInvocation')
      await expect(table.getByText('Target sync', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Direct write', { exact: true })).toHaveCount(0)
      await expect(table.getByText('Action', { exact: true })).toBeVisible()
      await expect(table.getByText('reboot', { exact: true })).toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 3 — US-PA-051 核心：投递成功与 Out of sync 同时存在
  // ---------------------------------------------------------------------------
  test('[Scenario 3] US-PA-051 delivery success and out-of-sync state coexist', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-dde-coexist-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 设 desired { brightness: 80 }，设备回复成功但不上报新值 -> delta 不收敛
      const desiredPayload = { brightness: 80 }
      const commandPromise = device.waitForCommand()
      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)
      const command = await commandPromise
      expect(command.params).toMatchObject(desiredPayload)
      await device.replyCommand(command, 200)
      // 关键：不调用 postProperties，设备虽回复成功但 reported 仍为旧值（缺失）

      // 主断言（持久状态）：targetSync 记录已 Success，而 delta 仍含 brightness
      await expect.poll(
        async () => {
          const opsBody = await getJson<ListResponse<DeviceOperationRow>>(
            request,
            `/api/admin/device/operation?product_id=${PRODUCT_ID}&device_id=${deviceId}&operation_type=targetSync&page=1&page_size=10`,
          )
          const shadowBody = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return {
            syncStatus: opsBody.data?.[0]?.status ?? '',
            brightnessInDelta: 'brightness' in (shadowBody.delta ?? {}),
          }
        },
        { timeout: POLL_TIMEOUT },
      ).toEqual({ syncStatus: 'Success', brightnessInDelta: true })

      // UI 共存断言 1：Operations 中该 targetSync 记录显示投递成功（Success 徽标）
      await openOperationsTab(page, deviceId)
      await page.locator(SELECTORS.operations.typeFilter).selectOption('targetSync')
      const opsTable = page.locator(SELECTORS.operations.table)
      const syncRow = opsTable.locator('tr', { hasText: 'Target sync' })
      await expect(syncRow.getByText('Success', { exact: true })).toBeVisible({
        timeout: POLL_TIMEOUT,
      })

      // UI 共存断言 2：State & Configuration 中同一属性仍显示 Out of sync
      await page.locator(SELECTORS.deviceTabs.stateConfigurationTab).click()
      const stateTable = page.locator(SELECTORS.stateConfiguration.table)
      await expect(stateTable).toBeVisible({ timeout: POLL_TIMEOUT })
      const brightnessRow = stateTable.locator('tr', { hasText: 'brightness' })
      await expect(brightnessRow.getByText('Out of sync', { exact: true })).toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  // ---------------------------------------------------------------------------
  // Scenario 4 — US-PA-051 Direct write 与 Target 冲突时的固定警告文案
  // ---------------------------------------------------------------------------
  test('[Scenario 4] US-PA-051 direct write conflicting with target shows fixed warning copy', async ({
    page,
    request,
    demoLogger: _demoLogger,
  }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    const deviceId = `e2e-dde-warning-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    try {
      await updateProduct(request, productId, { auto_provisioning: true })
      await device.connect()

      // 建立 Target { brightness: 80 } 并使命令收敛（对话框冲突判断只读 shadow.desired）
      const desiredPayload = { brightness: 80 }
      const commandPromise = device.waitForCommand()
      const putResponse = await request.put('/api/admin/property/shadow/desired', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, desired: desiredPayload },
      })
      expect(putResponse.status()).toBe(200)
      const command = await commandPromise
      await device.replyCommand(command)

      // 主断言（持久状态）：desired 已落库，对话框冲突判断的数据前提成立
      await expect.poll(
        async () => {
          const body = await getJson<ShadowView>(
            request,
            `/api/admin/property/shadow?product_id=${PRODUCT_ID}&device_id=${deviceId}`,
          )
          return (body.desired ?? {}).brightness
        },
        { timeout: POLL_TIMEOUT },
      ).toBe(80)

      // 打开 Direct property write 对话框
      await openOperationsTab(page, deviceId)
      await page.locator(SELECTORS.operations.moreActionsButton).click()
      await page.locator(SELECTORS.operations.directPropertyWriteButton).click()
      const dialog = page.locator(SELECTORS.operations.directWriteDialog)
      await expect(dialog).toBeVisible()

      // 写入与 Target 冲突的值 -> 展示固定警告文案（持久对话框区域，非 toast）
      await dialog
        .locator(SELECTORS.operations.directWriteJsonInput)
        .fill(JSON.stringify({ brightness: 50 }))
      const warning = dialog.locator(SELECTORS.operations.targetConflictWarning)
      await expect(warning).toBeVisible()
      await expect(warning).toHaveText(TARGET_CONFLICT_WARNING)

      // 写入与 Target 一致的值 -> 警告消失（冲突判断基于值差异）
      await dialog
        .locator(SELECTORS.operations.directWriteJsonInput)
        .fill(JSON.stringify(desiredPayload))
      await expect(warning).not.toBeVisible()

      // 不提交，仅取消（本场景只验收提示行为，不产生写操作）
      await dialog.locator(SELECTORS.operations.dialogCancelButton).click()
      await expect(dialog).not.toBeVisible()
    } finally {
      await device.disconnect()
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
