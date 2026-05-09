/**
 * OTA Demo 测试
 *
 * 对应用户故事：US-PA-011 创建 OTA 版本
 *               US-PA-012 查看 OTA 版本列表
 *               US-PA-013 编辑/删除 OTA 版本
 *               US-PA-021 查看 OTA 版本详情
 *               US-DV-006 OTA 固件升级流程
 *
 * 验证管理员可以通过后台查看 OTA 固件版本列表，并导航到创建和详情页面。
 * 验证 MQTT OTA 固件升级闭环：创建版本、设备上报、推送升级。
 * 验证 Web UI 创建 OTA 版本表单。
 * 前置条件：系统中已有产品。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { APIRequestContext } from '@playwright/test'
import { Buffer } from 'node:buffer'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { OtaListPage } from './pages/ota-list-page'
import { OtaCreatePage } from './pages/ota-create-page'
import { OtaEditPage } from './pages/ota-edit-page'
import { OtaDetailPage } from './pages/ota-detail-page'

const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 15_000

interface OtaVersionListResponse {
  data?: Array<{
    id: number
    product_id: string
    key: string
    version: number
    min_version: number
    max_version: number | null
    file_key: string
    log: unknown
    device_ids: string[] | null
    status: number
  }>
  pagination?: { total: number }
}

interface OtaVersionDetailResponse {
  id: number
  product_id: string
  key: string
  version: number
  min_version: number
  max_version: number | null
  file_key: string
  log: unknown
  device_ids: string[] | null
  status: number
  released_at: string
  created_at: string
  updated_at: string
  bin_length: number | null
  bin_md5: string | null
}

let _s3Available: boolean | undefined

async function isS3Available(request: APIRequestContext): Promise<boolean> {
  if (_s3Available === undefined) {
    const response = await request.post('/api/admin/file/upload', {
      data: {
        fileName: 's3-probe.bin',
        directory: 'ota',
        useOriginName: false,
        fileType: 'application/octet-stream',
      },
    })
    _s3Available = response.status() === 200
  }
  return _s3Available
}

test.describe('OTA demo', () => {
  test('shows OTA versions list page with actions', async ({ page }) => {
    const listPage = new OtaListPage(page)
    await listPage.gotoList(FRONTEND_URL)
    await listPage.verifyListPage()
  })

  test('shows product filter on list page', async ({ page }) => {
    const listPage = new OtaListPage(page)
    await listPage.gotoList(FRONTEND_URL)
    await listPage.verifyProductFilter()
  })

  test('navigates to create OTA version page', async ({ page }) => {
    const listPage = new OtaListPage(page)
    await listPage.gotoList(FRONTEND_URL)
    await listPage.navigateToCreate()
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/ota/create`))

    const createPage = new OtaCreatePage(page)
    await expect(createPage.heading).toBeVisible()
  })

  test('create OTA version form has required fields', async ({ page }) => {
    const createPage = new OtaCreatePage(page)
    await createPage.gotoCreate(FRONTEND_URL)
    await createPage.verifyFormFields()
  })
})

test.describe('OTA MQTT flow demo', () => {
  test('[US-DV-006] admin creates OTA version, device reports old version and receives upgrade', async ({ request }) => {
    const ts = Date.now()
    const deviceId = `demo-e2e-ota-upgrade-${ts}`
    const versionSuffix = `${ts}`.slice(-6)
    const fileKey = `ota/firmware_2.0.0_${versionSuffix}.bin`

    const createResponse = await request.post('/api/admin/ota/version', {
      data: {
        product_id: PRODUCT_ID,
        key: 'firmware',
        version: '2.0.0',
        min_version: '1.0.0',
        max_version: null,
        file_key: fileKey,
        log: { notes: `E2E test upgrade ${ts}` },
        device_ids: null,
        bin_length: 1024,
        bin_md5: 'd41d8cd98f00b204e9800998ecf8427e',
      },
    })
    expect(createResponse.status()).toBe(201)

    let otaVersionId: number | undefined
    try {
      const listBody = await getJson<OtaVersionListResponse>(
        request,
        `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=10`,
      )
      const created = listBody.data?.find(v => v.file_key === fileKey)
      expect(created).toBeDefined()
      otaVersionId = created!.id

      const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
      await device.connect()
      try {
        await device.subscribeOtaUpgrade()

        await device.publishOtaVersionReport([{ key: 'firmware', version: 100000 }])

        if (await isS3Available(request)) {
          const upgrade = await device.waitForOtaUpgrade(POLL_TIMEOUT)
          expect(upgrade.params.length).toBeGreaterThan(0)
          expect(upgrade.params[0].key).toBe('firmware')
          expect(upgrade.params[0].version).toBe(200000)
        } else {
          const upgradeResult = await Promise.race([
            device.waitForOtaUpgrade(3000).catch(() => null),
            new Promise<null>((resolve) => setTimeout(() => resolve(null), 3000)),
          ])
          expect(upgradeResult).toBeNull()
        }
      } finally {
        await device.disconnect()
      }
    } finally {
      // Clean up: delete the OTA version
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-DV-006] admin creates targeted OTA version for specific devices', async ({ request }) => {
    const ts = Date.now()
    const targetedDeviceId = `demo-e2e-ota-target-${ts}`
    const otherDeviceId = `demo-e2e-ota-other-${ts}`
    const versionSuffix = `${ts}`.slice(-6)
    const targetedFileKey = `ota/firmware_targeted_${versionSuffix}.bin`
    const broadcastFileKey = `ota/firmware_broadcast_${versionSuffix}.bin`

    const createTargeted = await request.post('/api/admin/ota/version', {
      data: {
        product_id: PRODUCT_ID,
        key: 'firmware',
        version: '3.0.0',
        min_version: '1.0.0',
        max_version: null,
        file_key: targetedFileKey,
        log: { notes: `E2E targeted test ${ts}` },
        device_ids: [targetedDeviceId],
        bin_length: 2048,
        bin_md5: 'd41d8cd98f00b204e9800998ecf8427e',
      },
    })
    expect(createTargeted.status()).toBe(201)

    let targetedOtaId: number | undefined
    let broadcastOtaId: number | undefined
    try {
      // Find the targeted OTA version ID
      const listBody = await getJson<OtaVersionListResponse>(
        request,
        `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=20`,
      )
      const targeted = listBody.data?.find(v => v.file_key === targetedFileKey)
      expect(targeted).toBeDefined()
      targetedOtaId = targeted!.id
      expect(targeted!.device_ids).toContain(targetedDeviceId)

      // Broadcast version (v2.5.0) must be lower than targeted (v3.0.0) so targeted device gets v3.0.0
      const createBroadcast = await request.post('/api/admin/ota/version', {
        data: {
          product_id: PRODUCT_ID,
          key: 'firmware',
          version: '2.5.0',
          min_version: '1.0.0',
          max_version: null,
          file_key: broadcastFileKey,
          log: { notes: `E2E broadcast test ${ts}` },
          device_ids: null,
          bin_length: 2048,
          bin_md5: 'd41d8cd98f00b204e9800998ecf8427e',
        },
      })
      expect(createBroadcast.status()).toBe(201)

      // Find the broadcast OTA version ID
      const listBody2 = await getJson<OtaVersionListResponse>(
        request,
        `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=20`,
      )
      const broadcast = listBody2.data?.find(v => v.file_key === broadcastFileKey)
      expect(broadcast).toBeDefined()
      broadcastOtaId = broadcast!.id

      const targetedDevice = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId: targetedDeviceId })
      await targetedDevice.connect()
      try {
        await targetedDevice.subscribeOtaUpgrade()

        await targetedDevice.publishOtaVersionReport([{ key: 'firmware', version: 100000 }])

        if (await isS3Available(request)) {
          const upgrade = await targetedDevice.waitForOtaUpgrade(POLL_TIMEOUT)
          expect(upgrade.params.length).toBeGreaterThan(0)
          expect(upgrade.params[0].version).toBe(300000) // 3.0.0
        }
      } finally {
        await targetedDevice.disconnect()
      }

      const otherDevice = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId: otherDeviceId })
      await otherDevice.connect()
      try {
        await otherDevice.subscribeOtaUpgrade()

        await otherDevice.publishOtaVersionReport([{ key: 'firmware', version: 100000 }])

        if (await isS3Available(request)) {
          const upgrade = await otherDevice.waitForOtaUpgrade(POLL_TIMEOUT)
          expect(upgrade.params.length).toBeGreaterThan(0)
          expect(upgrade.params[0].version).toBe(250000) // 2.5.0
        }
      } finally {
        await otherDevice.disconnect()
      }
    } finally {
      if (targetedOtaId !== undefined) {
        await request.delete(`/api/admin/ota/version/${targetedOtaId}`)
      }
      if (broadcastOtaId !== undefined) {
        await request.delete(`/api/admin/ota/version/${broadcastOtaId}`)
      }
    }
  })

  test('[US-DV-006] device reports version already at latest, no upgrade pushed', async ({ request }) => {
    const ts = Date.now()
    const deviceId = `demo-e2e-ota-latest-${ts}`
    const versionSuffix = `${ts}`.slice(-6)
    const fileKey = `ota/firmware_noupgrade_${versionSuffix}.bin`

    const createResponse = await request.post('/api/admin/ota/version', {
      data: {
        product_id: PRODUCT_ID,
        key: 'firmware',
        version: '2.0.0',
        min_version: '1.0.0',
        max_version: null,
        file_key: fileKey,
        log: { notes: `E2E no-upgrade test ${ts}` },
        device_ids: null,
        bin_length: 1024,
        bin_md5: 'd41d8cd98f00b204e9800998ecf8427e',
      },
    })
    expect(createResponse.status()).toBe(201)

    let otaVersionId: number | undefined
    try {
      const listBody = await getJson<OtaVersionListResponse>(
        request,
        `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=10`,
      )
      const created = listBody.data?.find(v => v.file_key === fileKey)
      expect(created).toBeDefined()
      otaVersionId = created!.id

      // max_version (1.5.0) < device version (2.0.0) → no match
      const updateResponse = await request.put(`/api/admin/ota/version/${otaVersionId}`, {
        data: { max_version: '1.5.0' },
      })
      expect(updateResponse.status()).toBe(200)

      const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })
      await device.connect()
      try {
        await device.subscribeOtaUpgrade()

        await device.publishOtaVersionReport([{ key: 'firmware', version: 200000 }])

        const upgradeResult = await Promise.race([
          device.waitForOtaUpgrade(3000).catch(() => null),
          new Promise<null>((resolve) => setTimeout(() => resolve(null), 3000)),
        ])
        expect(upgradeResult).toBeNull()
      } finally {
        await device.disconnect()
      }
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })
})

test.describe('OTA CRUD (US-PA-011, US-PA-013, US-PA-021)', () => {
  test('[US-PA-011] create OTA version via API and verify in list', async ({ request }) => {
    const ts = Date.now()
    const fileKey = `ota/create_test_${ts}.bin`
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: fileKey,
        device_ids: ['dev-a', 'dev-b'],
        log: { notes: `E2E create test ${ts}` },
      })
      otaVersionId = created.id
      expect(created.device_ids).toContain('dev-a')
      expect(created.device_ids).toContain('dev-b')
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-011] file upload API returns presigned URL or error when S3 unavailable', async ({ request }) => {
    const response = await request.post('/api/admin/file/upload', {
      data: {
        fileName: 'test_firmware.bin',
        directory: 'ota',
        useOriginName: false,
        fileType: 'application/octet-stream',
      },
    })
    // 200: S3 available and directory allowed
    // 400: S3 available but directory not in allowed list
    // 500: S3 not configured
    if (response.status() === 200) {
      const body = await response.json()
      expect(body.url).toBeDefined()
      expect(body.fields).toBeDefined()
    } else {
      expect([400, 500]).toContain(response.status())
    }
  })

  test('[US-PA-013] update OTA version via PUT API', async ({ request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/update_test_${ts}.bin`,
      })
      otaVersionId = created.id

      const updateResponse = await request.put(`/api/admin/ota/version/${otaVersionId}`, {
        data: { min_version: '1.1.0', max_version: '2.0.0' },
      })
      expect(updateResponse.status()).toBe(200)

      const detail = await getJson<OtaVersionDetailResponse>(
        request,
        `/api/admin/ota/version/${otaVersionId}`,
      )
      expect(detail.min_version).toBe(101000) // 1.1.0
      expect(detail.max_version).toBe(200000) // 2.0.0
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-013] update OTA version device_ids via PUT API', async ({ request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/update_deviceids_${ts}.bin`,
      })
      otaVersionId = created.id

      const updateResponse = await request.put(`/api/admin/ota/version/${otaVersionId}`, {
        data: { device_ids: ['dev-x'] },
      })
      expect(updateResponse.status()).toBe(200)

      const detail = await getJson<OtaVersionDetailResponse>(
        request,
        `/api/admin/ota/version/${otaVersionId}`,
      )
      expect(detail.device_ids).toContain('dev-x')
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-013] delete OTA version via API and verify removal', async ({ request }) => {
    const ts = Date.now()
    const created = await createOtaVersion(request, {
      file_key: `ota/delete_test_${ts}.bin`,
    })

    const deleteResponse = await request.delete(`/api/admin/ota/version/${created.id}`)
    expect(deleteResponse.status()).toBe(200)

    const getResponse = await request.get(`/api/admin/ota/version/${created.id}`)
    expect(getResponse.status()).toBe(404)
  })

  test('[US-PA-013] edit page shows disabled Product/Key/Version and editable fields', async ({ page, request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/edit_page_${ts}.bin`,
      })
      otaVersionId = created.id

      const editPage = new OtaEditPage(page)
      await editPage.gotoEdit(FRONTEND_URL, otaVersionId)
      await editPage.verifyEditPage()
      await editPage.verifyDisabledFields(3)
      await editPage.verifyMinVersionEditable()
      await expect(editPage.saveButton).toBeVisible()
      await expect(editPage.cancelButton).toBeVisible()
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-021] GET /api/admin/ota/version/{id} returns complete version data', async ({ request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/detail_api_${ts}.bin`,
        log: { notes: 'detail test' },
        device_ids: ['dev-1'],
      })
      otaVersionId = created.id

      const detail = await getJson<OtaVersionDetailResponse>(
        request,
        `/api/admin/ota/version/${otaVersionId}`,
      )
      expect(detail.id).toBe(otaVersionId)
      expect(detail.product_id).toBe(PRODUCT_ID)
      expect(detail.key).toBe('firmware')
      expect(detail.version).toBe(200000)
      expect(detail.min_version).toBe(100000)
      expect(detail.file_key).toContain('detail_api_')
      expect(detail.device_ids).toContain('dev-1')
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-021] GET /api/admin/ota/version/{id} returns 404 for nonexistent ID', async ({ request }) => {
    const response = await request.get('/api/admin/ota/version/999999')
    expect(response.status()).toBe(404)
  })

  test('[US-PA-021] OTA version detail page shows all fields', async ({ page, request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/detail_page_${ts}.bin`,
        log: 'release notes',
        device_ids: ['dev-1', 'dev-2'],
      })
      otaVersionId = created.id

      const detailPage = new OtaDetailPage(page)
      await detailPage.gotoDetail(FRONTEND_URL, otaVersionId)
      await detailPage.verifyDetailPage()
      await detailPage.verifyFieldValue('firmware')
      await detailPage.verifyFieldValue('2.0.0')
      await detailPage.verifyFieldValue('1.0.0')
      await detailPage.verifyFieldValue('dev-1')
      await detailPage.verifyFieldValue('dev-2')
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })

  test('[US-PA-021] OTA version detail page shows dash for empty fields', async ({ page, request }) => {
    const ts = Date.now()
    let otaVersionId: number | undefined
    try {
      const created = await createOtaVersion(request, {
        file_key: `ota/detail_empty_${ts}.bin`,
        max_version: null,
        log: null,
        device_ids: null,
      })
      otaVersionId = created.id

      const detailPage = new OtaDetailPage(page)
      await detailPage.gotoDetail(FRONTEND_URL, otaVersionId)
      await detailPage.verifyDetailPage()
      await detailPage.verifyDashForEmptyFields()
    } finally {
      if (otaVersionId !== undefined) {
        await request.delete(`/api/admin/ota/version/${otaVersionId}`)
      }
    }
  })
})

test.describe('OTA web UI creation', () => {
  test('[US-PA-011] OTA create form renders and accepts input', async ({ page }) => {
    const createPage = new OtaCreatePage(page)
    await createPage.gotoCreate(FRONTEND_URL)
    await expect(createPage.heading).toBeVisible()

    await expect(createPage.productSelect).toBeVisible()
    await expect(createPage.keyInput).toBeVisible()
    await expect(createPage.versionInput).toBeVisible()
    await expect(createPage.minVersionInput).toBeVisible()
    await expect(createPage.maxVersionInput).toBeVisible()
    await expect(createPage.firmwareFileLabel).toBeVisible()
    await expect(createPage.createButton).toBeVisible()
    await expect(createPage.cancelButton).toBeVisible()

    await createPage.productSelect.selectOption('demo_product')

    await createPage.keyInput.fill('firmware')
    await createPage.versionInput.fill('2.0.0')
    await createPage.minVersionInput.fill('1.0.0')
    await createPage.maxVersionInput.fill('1.9.0')

    await createPage.logTextarea.fill('E2E web UI test release')

    await createPage.deviceInput.fill('device-001')
    await createPage.deviceInput.press('Enter')
    await expect(page.getByText('device-001')).toBeVisible()

    await createPage.addDeviceId('device-002')

    await createPage.submit()
    await createPage.verifyToastVisible()
  })

  test('[US-PA-011] create OTA version via web form with file upload', async ({ page, request }) => {
    if (!(await isS3Available(request))) {
      test.skip()
      return
    }

    const uniqueSuffix = Date.now()

    const createPage = new OtaCreatePage(page)
    await createPage.gotoCreate(FRONTEND_URL)

    await createPage.productSelect.selectOption('demo_product')
    await createPage.keyInput.fill('firmware')
    await createPage.versionInput.fill('3.0.0')
    await createPage.minVersionInput.fill('1.0.0')
    await createPage.logTextarea.fill(`E2E web creation test ${uniqueSuffix}`)

    const fileContent = Buffer.alloc(256, 0xAB)
    await createPage.fileInput.setInputFiles({
      name: `firmware_web_${uniqueSuffix}.bin`,
      mimeType: 'application/octet-stream',
      buffer: fileContent,
    })

    await expect(page.getByText('File uploaded')).toBeVisible({ timeout: 10_000 })

    await createPage.submit()

    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/ota$`))

    await expect(page.getByText('OTA version created')).toBeVisible()

    const listBody = await getJson<OtaVersionListResponse>(
      request,
      `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=10`,
    )
    const created = listBody.data?.find(v => v.version === 300000 && v.key === 'firmware')
    if (created) {
      await request.delete(`/api/admin/ota/version/${created.id}`)
    }
  })
})

async function createOtaVersion(
  request: APIRequestContext,
  overrides: Record<string, unknown> = {},
) {
  const ts = Date.now()
  const defaults = {
    product_id: PRODUCT_ID,
    key: 'firmware',
    version: '2.0.0',
    min_version: '1.0.0',
    max_version: null,
    file_key: `ota/test_${ts}.bin`,
    log: null,
    device_ids: null,
    bin_length: 1024,
    bin_md5: 'd41d8cd98f00b204e9800998ecf8427e',
  }
  const data = { ...defaults, ...overrides }
  const createResponse = await request.post('/api/admin/ota/version', { data })
  if (createResponse.status() !== 201) {
    const text = await createResponse.text()
    throw new Error(`Create OTA version failed: ${createResponse.status()} ${text}`)
  }

  const listBody = await getJson<OtaVersionListResponse>(
    request,
    `/api/admin/ota/version?product_id=${PRODUCT_ID}&page=1&page_size=20`,
  )
  const created = listBody.data?.find(v => v.file_key === data.file_key)
  if (!created) {
    throw new Error(`Created OTA version not found in list (file_key: ${data.file_key})`)
  }
  return created
}

async function getJson<T>(request: APIRequestContext, path: string): Promise<T> {
  const response = await request.get(path)
  if (!response.ok()) {
    const text = await response.text()
    throw new Error(`GET ${path} returned ${response.status()}: ${text}`)
  }
  return response.json()
}
