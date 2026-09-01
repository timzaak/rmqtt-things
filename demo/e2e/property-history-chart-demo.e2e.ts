/**
 * Property History Chart Demo 测试
 *
 * 对应用户故事：US-PA-052（以折线图查看设备属性历史趋势）
 *
 * 覆盖 US-PA-052 的四个验收场景
 * （docs/user-stories/01-platform-admin-user-stories.md）：
 * 1. 选属性 + 时间档（含自定义）看到折线，悬停图表页面保持可用
 * 2. 空时间范围显示空态（property-chart-empty），不是错误
 * 3. 非数值属性（mode/power）不出现在标签区，但表格视图可见
 * 4. 7 天档触发降采样标注（property-chart-downsample-note-*）且页面可用
 *
 * 数据前提：demo_env.seed_demo_data 注入 2000 行图表种子（近 7 天、每 5 分钟、
 * temperature/humidity 数值 + mode/power 非数值随行）。默认 24h 档约 288 点
 * （不降采样），7d 档 2000 点（触发 1000 点上限 → downsampled 标注）。
 *
 * Chart.js 在 canvas 内绘制（含 tooltip），DOM 断言覆盖容器/标签/标注/三态；
 * 悬停交互以"hover 后页面仍可操作"断言页面可用性，曲线像素与 tooltip 文案
 * 属人工核验范围（多轴可读性同理）。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { SELECTORS } from './selectors'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const DEVICE_ID = 'demo-device'
const CHART = SELECTORS.propertyHistoryChart

async function gotoReportedDataChart(page: import('@playwright/test').Page): Promise<void> {
  await page.goto(`${FRONTEND_URL}/devices/show/${DEVICE_ID}`, {
    waitUntil: 'commit',
  })
  await page.locator(SELECTORS.deviceTabs.reportedDataTab).click()
  await expect(page.getByRole('heading', { name: 'Property History' })).toBeVisible()
  // 图表为默认视图（与表格并存的加法式双视图）
  await expect(page.locator(CHART.viewChartButton)).toBeVisible()
}

/** datetime-local 本地时间串（与前端 toDatetimeLocal 一致的粒度）。 */
function toDatetimeLocal(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

test.describe('Property history chart (US-PA-052)', () => {
  test('scenario 1: select keys and ranges (incl. custom) to plot and hover the chart', async ({ page }) => {
    await gotoReportedDataChart(page)

    // 默认预选最活跃数值属性 temperature（keys 按 sampleCount 降序）
    await expect(page.locator(CHART.keyToggle('temperature'))).toBeVisible()
    await expect(page.locator(CHART.container)).toBeVisible()
    await expect(page.locator(CHART.container).locator('canvas')).toBeVisible()

    // 默认 24h 档 ~288 点，不应出现降采样标注
    await expect(page.locator(CHART.downsampleNote('temperature'))).not.toBeVisible()

    // 多选第二个数值属性：曲线同图可辨识（chips 即图例，选中着色）
    await page.locator(CHART.keyToggle('humidity')).click()
    await expect(page.locator(CHART.container)).toBeVisible()

    // 时间档切换自动刷新（FR2：无查询按钮）
    await page.locator(CHART.rangeSelect).selectOption('1h')
    await expect(page.locator(CHART.container)).toBeVisible()

    // 自定义起止：预填上一档位，填入合法近 2 小时窗口后自动刷新
    await page.locator(CHART.rangeSelect).selectOption('custom')
    await expect(page.locator(CHART.startInput)).toBeVisible()
    await expect(page.locator(CHART.endInput)).toBeVisible()
    const now = Date.now()
    await page.locator(CHART.startInput).fill(toDatetimeLocal(new Date(now - 2 * 3600 * 1000)))
    await page.locator(CHART.endInput).fill(toDatetimeLocal(new Date(now - 3600 * 1000)))
    await expect(page.locator(CHART.container)).toBeVisible()

    // 悬停图表（FR4 悬停明细入口）：hover 后页面仍可操作
    const box = await page.locator(CHART.container).boundingBox()
    if (box) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    }
    await expect(page.locator(CHART.container)).toBeVisible()
    await page.locator(CHART.keyToggle('temperature')).click() // 取消选择 → 仍可交互
    await expect(page.locator(CHART.container)).toBeVisible()
  })

  test('scenario 2: empty custom range shows the empty state, not an error', async ({ page }) => {
    await gotoReportedDataChart(page)

    await page.locator(CHART.rangeSelect).selectOption('custom')
    const now = Date.now()
    await page.locator(CHART.startInput).fill(toDatetimeLocal(new Date(now + 10 * 24 * 3600 * 1000)))
    await page.locator(CHART.endInput).fill(toDatetimeLocal(new Date(now + 11 * 24 * 3600 * 1000)))

    // 未来窗口无数据：空态可见、错误态缺席（空态与错误态必须可区分）
    await expect(page.locator(CHART.empty)).toBeVisible()
    await expect(page.locator('[data-testid="property-chart-error"]')).not.toBeVisible()
  })

  test('scenario 3: non-numeric keys stay out of the chart but visible in the table', async ({ page }) => {
    await gotoReportedDataChart(page)

    // mode（字符串）/ power（布尔）永不入图（R1：仅全数值键入选）
    await expect(page.locator(CHART.keyToggle('temperature'))).toBeVisible()
    await expect(page.locator(CHART.keyToggle('mode'))).toHaveCount(0)
    await expect(page.locator(CHART.keyToggle('power'))).toHaveCount(0)

    // 表格视图仍可审计原始上报（表格零改动并存）
    await page.locator(CHART.viewTableButton).click()
    await expect(page.getByRole('columnheader', { name: 'Reported Time' })).toBeVisible()
    await expect(page.getByText('"eco"').first()).toBeVisible()
    await expect(page.getByText('"power": true').first()).toBeVisible()
  })

  test('scenario 4: 7d range triggers the downsample note and stays usable', async ({ page }) => {
    await gotoReportedDataChart(page)

    // 加入第二个数值属性：两键各自产生降采样标注（逐键 stride 可能不同）
    await page.locator(CHART.keyToggle('humidity')).click()

    // 7 天窗口 2000 点 > 1000 点上限 → 逐键降采样标注（降精度必须显式披露）
    await page.locator(CHART.rangeSelect).selectOption('7d')
    await expect(page.locator(CHART.downsampleNote('temperature'))).toBeVisible()
    await expect(page.locator(CHART.downsampleNote('humidity'))).toBeVisible()
    // 标注必须披露真实总数、抽样点数与步长，而非把抽样点伪装成全量。
    // 总数随 seed/查询时钟差在 2000 上下浮动（stride 相应为 2 或 3），钉格式不钉值。
    const temperatureNote = page.locator(CHART.downsampleNote('temperature'))
    await expect(temperatureNote).toContainText(/共 \d+ 条记录/)
    await expect(temperatureNote).toContainText(/显示 \d+ 个抽样点/)
    await expect(temperatureNote).toContainText(/每 \d+ 条取 1 条/)

    // 降采样状态下页面保持可用：悬停、增删属性、切换视图（US-PA-052 场景 4）
    const box = await page.locator(CHART.container).boundingBox()
    if (box) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    }
    await page.locator(CHART.keyToggle('humidity')).click()
    await expect(page.locator(CHART.container)).toBeVisible()
    await page.locator(CHART.viewTableButton).click()
    await expect(page.getByRole('columnheader', { name: 'Reported Time' })).toBeVisible()
  })
})
