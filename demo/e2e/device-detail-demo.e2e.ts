/**
 * Device Detail Demo 测试
 *
 * 对应用户故事：US-PA-020（设备详情页面）/ US-PA-015（设备列表与详情）
 *
 * 验证管理员可以查看设备详情页面的各区域，且每个区域都有数据展示。
 * 前置条件：系统中已有产品和设备连接记录（通过 seed_demo_data 初始化）。
 *
 * DE-D02 适配（device-detail-experience 七区信息架构）：旧的单页分区
 * （Device Info / Latest Properties / Property History / Event History /
 * Property Commands / Connection History）被重组为七区 Tab 结构
 * （show.$id.tsx TABS）：
 * - Overview（默认激活）= Device Info + Latest Properties + failed-ops 摘要
 * - State & Configuration / Operations / Reported Data / Events /
 *   Connectivity / Metadata
 * 因此 region 断言需先切到对应 Tab。Tab 文案集中在 SELECTORS.deviceTabs
 * （禁止硬编码选择器）。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { SELECTORS } from './selectors'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const DEVICE_ID = 'demo-device'

/**
 * 导航到设备详情页。
 *
 * waitUntil:'commit' 在收到响应头即返回（最早可靠点），避免 Vite dev server 下
 * 'load'/'domcontentloaded' 偶发不触发导致 30s 超时（DE-TR01 复现：业务状态已
 * 就绪但页面 load 卡死）。后续 toBeVisible 断言自带重试，足以保证 DOM 就绪。
 */
async function gotoDeviceDetail(page: import('@playwright/test').Page): Promise<void> {
  await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`, {
    waitUntil: 'commit',
  })
}

test.describe('Device detail page (US-PA-020)', () => {
  test('shows device detail page with all region headings', async ({ page }) => {
    await gotoDeviceDetail(page)

    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    // Overview（默认激活）= Device Info + Latest Properties（DeviceOverviewSection.tsx）
    await expect(page.getByRole('heading', { name: 'Device Info' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Latest Properties' })).toBeVisible()

    // Reported Data tab = Latest Properties + Property History（ReportedDataSection.tsx）
    await page.locator(SELECTORS.deviceTabs.reportedDataTab).click()
    await expect(page.getByRole('heading', { name: 'Property History' })).toBeVisible()

    // Events tab（无 testid，按稳定文案定位）
    await page.getByRole('tab', { name: SELECTORS.deviceTabs.eventsTab }).click()
    await expect(page.getByRole('heading', { name: 'Event History' })).toBeVisible()

    // Connectivity tab（无 testid，按稳定文案定位）
    await page.getByRole('tab', { name: SELECTORS.deviceTabs.connectivityTab }).click()
    await expect(page.getByRole('heading', { name: 'Connection History' })).toBeVisible()

    // Operations tab：seeded Pending 属性命令汇入统一操作表
    // （DeviceOperationsSection.tsx，device-operations-table testid）
    await page.locator(SELECTORS.deviceTabs.operationsTab).click()
    await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()
  })

  test('shows Back to Devices link on detail page', async ({ page }) => {
    await gotoDeviceDetail(page)

    await expect(page.getByRole('heading', { name: 'Device Detail' })).toBeVisible()

    const backLink = page.getByRole('link', { name: /Back to Devices/ })
    await expect(backLink).toBeVisible()
  })

  test('shows device info with seeded data', async ({ page }) => {
    await gotoDeviceDetail(page)

    // Device Info 在默认 Overview tab 上（DeviceOverviewSection.tsx）
    await expect(page.getByRole('heading', { name: 'Device Info' })).toBeVisible()

    // Device ID and Product ID should show seeded values
    await expect(page.getByText('demo-device')).toBeVisible()
    await expect(page.getByText('demo_product')).toBeVisible()

    // Status 字段应渲染（Online / Offline）。
    // DE-TR01 修复：seed demo-device 的在线状态由 demo 启动时的 MQTT 心跳决定，
    // demo-stop/restart 后心跳断开即变 Offline，是运行时瞬态而非 seeded 数据。
    // 本用例的意图是"Device Info 字段以 seeded 数据渲染"，因此断言 Status
    // 字段存在而非具体取值；连接态由专门的连接场景覆盖。
    const deviceInfoSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Device Info' }) })
    await expect(deviceInfoSection.getByText('Status')).toBeVisible()
    await expect(
      deviceInfoSection.getByText('Online', { exact: true }).or(
        deviceInfoSection.getByText('Offline', { exact: true }),
      ),
    ).toBeVisible()
  })

  test('shows latest properties with data rows', async ({ page }) => {
    await gotoDeviceDetail(page)

    // Latest Properties 在默认 Overview tab 上
    await expect(page.getByRole('heading', { name: 'Latest Properties' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No latest properties')).not.toBeVisible()

    // Should show seeded property values (temperature, humidity, power)
    // Scope to the Latest Properties section to avoid matching Property History
    const latestSection = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Latest Properties' }) })
    await expect(latestSection.getByText(/temperature/)).toBeVisible()
  })

  test('shows property history with data rows', async ({ page }) => {
    await gotoDeviceDetail(page)

    // Property History 在 Reported Data tab（ReportedDataSection.tsx 组合）
    await page.locator(SELECTORS.deviceTabs.reportedDataTab).click()
    await expect(page.getByRole('heading', { name: 'Property History' })).toBeVisible()

    await expect(page.getByText('No property history')).not.toBeVisible()
  })

  test('shows event history with data rows', async ({ page }) => {
    await gotoDeviceDetail(page)

    // Event History 在 Events tab（无 testid，按稳定文案定位）
    await page.getByRole('tab', { name: SELECTORS.deviceTabs.eventsTab }).click()
    await expect(page.getByRole('heading', { name: 'Event History' })).toBeVisible()

    await expect(page.getByText('No event history')).not.toBeVisible()
  })

  test('shows property command via unified Operations region', async ({ page }) => {
    await gotoDeviceDetail(page)

    // DE-D02：旧的 Property Commands 分区已被统一 Operations 表接管
    // （DeviceOperationsSection.tsx）。seeded property_command 行（status=0
    // = Pending）汇入 device-operations-table，以 Pending 状态展示。
    await page.locator(SELECTORS.deviceTabs.operationsTab).click()
    await expect(page.getByRole('heading', { name: 'Operations' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No operations')).not.toBeVisible()

    // Should show seeded command status (seed_demo_data: status=0 => Pending)
    await expect(page.locator(SELECTORS.operations.table).getByText('Pending')).toBeVisible()

    // The Direct property write entry exists via More menu
    // （operations.region 的入口按设计 §4.4.2 收敛到 More ▾ 下拉）
    await page.locator(SELECTORS.operations.moreActionsButton).click()
    await expect(page.locator(SELECTORS.operations.directPropertyWriteButton)).toBeVisible()
  })

  test('shows connection history with data rows', async ({ page }) => {
    await gotoDeviceDetail(page)

    // Connection History 在 Connectivity tab（无 testid，按稳定文案定位）
    await page.getByRole('tab', { name: SELECTORS.deviceTabs.connectivityTab }).click()
    await expect(page.getByRole('heading', { name: 'Connection History' })).toBeVisible()

    // Should NOT show the empty message
    await expect(page.getByText('No connection history')).not.toBeVisible()
  })
})
