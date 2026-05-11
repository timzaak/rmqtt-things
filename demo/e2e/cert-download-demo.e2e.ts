/**
 * Certificate Download Demo 测试
 *
 * 对应用户故事：
 *   US-PA-024 下载已签发证书和私钥
 *   US-PA-025 下载 CA 证书
 *
 * 验证场景：
 * 1. 签发成功后展示证书内容、私钥内容和下载按钮 (US-PA-024 Scenario 1)
 * 2. 点击下载证书按钮，浏览器下载证书 PEM 文件 (US-PA-024 Scenario 2)
 * 3. 点击下载私钥按钮，浏览器下载私钥 PEM 文件 (US-PA-024 Scenario 3)
 * 4. 在证书管理区域下载 CA 证书 PEM 文件 (US-PA-025 Scenario 1)
 *
 * 前置条件：系统中已有产品 "Demo Smart Light" (model_no: demo_product)。
 * 前置条件：后端 API 运行在 BASE_URL (默认 http://localhost:8080)。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { Page, Download } from '@playwright/test'
import { verifyTestEnvironment } from './helpers/environment-setup'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe.configure({ mode: 'serial' })

async function issueCertViaUi(page: Page, deviceId: string): Promise<void> {
  await page.goto(`${FRONTEND_URL}/certs/create`)
  await expect(page.getByRole('heading', { name: 'Issue Certificate' })).toBeVisible()
  await page.getByLabel('Product').selectOption({ label: 'Demo Smart Light' })
  await page.getByLabel('Device ID').fill(deviceId)
  await expect(page.getByLabel('Start At')).not.toHaveValue('')
  await expect(page.getByLabel('End At')).not.toHaveValue('')
  await page.getByRole('button', { name: 'Issue' }).click()
  await expect(page.getByText('Certificate Issued Successfully')).toBeVisible()
}

async function readDownloadContent(download: Download): Promise<string | null> {
  const downloadPath = await download.path()
  if (!downloadPath) return null
  const fs = await import('fs')
  return fs.readFileSync(downloadPath, 'utf-8')
}

test.describe('Certificate download demo (US-PA-024)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null as any)
  })

  test('shows certificate and private key content with download buttons after issue succeeds (Scenario 1)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `dl-device-${Date.now()}`

    await issueCertViaUi(page, deviceId)

    // Warning about private key shown only once
    await expect(
      page.getByText('Private key is shown only once')
    ).toBeVisible()

    // Download buttons are visible
    await expect(
      page.getByRole('button', { name: 'Download Certificate' })
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Download Private Key' })
    ).toBeVisible()

    await expect(page.getByText(/BEGIN CERTIFICATE/)).toBeVisible()
    await expect(page.getByText(/BEGIN (RSA )?PRIVATE KEY/)).toBeVisible()
  })

  test('downloads certificate PEM file when clicking download certificate button (Scenario 2)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `cert-dl-${Date.now()}`
    await issueCertViaUi(page, deviceId)

    const downloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Download Certificate' }).click()
    const download = await downloadPromise

    expect(download.suggestedFilename()).toBe(`${deviceId}.pem`)

    const content = await readDownloadContent(download)
    expect(content).toContain('-----BEGIN CERTIFICATE-----')
    expect(content).toContain('-----END CERTIFICATE-----')
  })

  test('downloads private key PEM file when clicking download private key button (Scenario 3)', async ({ page, demoLogger: _demoLogger }) => {
    const deviceId = `key-dl-${Date.now()}`
    await issueCertViaUi(page, deviceId)

    const downloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Download Private Key' }).click()
    const download = await downloadPromise

    expect(download.suggestedFilename()).toBe(`${deviceId}.key`)

    const content = await readDownloadContent(download)
    expect(content).toMatch(/-----BEGIN (RSA )?PRIVATE KEY-----/)
    expect(content).toMatch(/-----END (RSA )?PRIVATE KEY-----/)
  })
})

test.describe('CA certificate download demo (US-PA-025)', () => {
  test.beforeAll(async () => {
    await verifyTestEnvironment(null as any)
  })

  test('downloads CA certificate PEM file from certificates management page (Scenario 1)', async ({ page, demoLogger: _demoLogger }) => {
    // Navigate to the certificates list page
    await page.goto(`${FRONTEND_URL}/certs`)
    await expect(page.getByRole('heading', { name: 'Certificates' })).toBeVisible()

    // Look for the "Download CA Certificate" button on the certs management page
    // This tests the expected UI: a button labeled "Download CA Certificate"
    const downloadCaButton = page.getByRole('button', { name: 'Download CA Certificate' })

    // If the button exists, test the download flow
    await expect(downloadCaButton).toBeVisible()

    const downloadPromise = page.waitForEvent('download')
    await downloadCaButton.click()
    const download = await downloadPromise

    // Verify filename
    expect(download.suggestedFilename()).toMatch(/\.pem$/)

    // Read downloaded content and verify it is a valid CA certificate
    const content = await readDownloadContent(download)
    expect(content).toContain('-----BEGIN CERTIFICATE-----')
    expect(content).toContain('-----END CERTIFICATE-----')
  })
})
