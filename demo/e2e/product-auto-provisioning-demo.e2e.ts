/**
 * Product Auto-Provisioning Toggle Demo Tests
 *
 * User story: US-PA-036 - Configure product auto-provisioning
 *
 * Verifies that an admin can enable/disable the auto-provisioning toggle
 * on a product's edit page, that new products default to OFF, and that
 * toggling does not affect already-registered devices.
 *
 * Prerequisites:
 * - Seed product "Demo Smart Light" with model_no `demo_product` exists.
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { SELECTORS } from './selectors'
import { createProduct, findSeedProductId, getProduct, updateProduct, SEED_PRODUCT_MODEL_NO } from './helpers/product-api'
import { issueCert } from './helpers/cert-api'

const P = SELECTORS.products
const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

async function gotoSeedProductEdit(page: import('@playwright/test').Page, request: import('@playwright/test').APIRequestContext): Promise<number> {
  const productId = await findSeedProductId(request)
  await page.goto(`${FRONTEND_URL}/products/edit/${productId}`)
  await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()
  return productId
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('US-PA-036: Product auto-provisioning toggle', () => {

  test('US-PA-036 S1: auto-provisioning toggle is visible on product edit page', async ({ page, request, demoLogger: _demoLogger }) => {
    await gotoSeedProductEdit(page, request)

    await expect(page.getByText(P.autoProvisioningLabel, { exact: true })).toBeVisible()
    await expect(page.getByText(P.autoProvisioningText)).toBeVisible()
  })

  test('US-PA-036 S2: new product defaults to auto-provisioning OFF', async ({ page, request, demoLogger: _demoLogger }) => {
    const uniqueSuffix = Date.now()
    const productName = `AutoProv Test ${uniqueSuffix}`
    const modelNo = `autoprov-model-${uniqueSuffix}`

    let productId: number | undefined
    try {
      const created = await createProduct(request, { name: productName, model_no: modelNo })
      productId = created.id

      await page.goto(`${FRONTEND_URL}/products/edit/${productId}`)
      await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()

      const checkbox = page.getByRole('checkbox')
      await expect(checkbox).not.toBeChecked()
    } finally {
      if (productId !== undefined) {
        await updateProduct(request, productId, { name: `[E2E-cleaned] ${productName}`, description: 'Cleaned up by auto-prov E2E' })
      }
    }
  })

  test('US-PA-036 S3: enable auto-provisioning via UI and verify via API', async ({ page, request, demoLogger: _demoLogger }) => {
    const productId = await gotoSeedProductEdit(page, request)

    let product = await getProduct(request, productId)
    if (product.auto_provisioning) {
      await updateProduct(request, productId, { auto_provisioning: false })
      await page.reload()
      await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()
    }

    const checkbox = page.getByRole('checkbox')
    await expect(checkbox).not.toBeChecked()
    await checkbox.check()
    await expect(checkbox).toBeChecked()

    await page.getByRole('button', { name: P.saveButton }).click()
    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/products$`))

    product = await getProduct(request, productId)
    expect(product.auto_provisioning).toBe(true)

    await updateProduct(request, productId, { auto_provisioning: false })
  })

  test('US-PA-036 S4: disable auto-provisioning via UI and verify via API', async ({ page, request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)

    await updateProduct(request, productId, { auto_provisioning: true })

    await page.goto(`${FRONTEND_URL}/products/edit/${productId}`)
    await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()

    const checkbox = page.getByRole('checkbox')
    await expect(checkbox).toBeChecked()

    await checkbox.uncheck()
    await expect(checkbox).not.toBeChecked()

    await page.getByRole('button', { name: P.saveButton }).click()
    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/products$`))

    const product = await getProduct(request, productId)
    expect(product.auto_provisioning).toBe(false)
  })

  test('US-PA-036 S5: toggling does not affect already-registered devices', async ({ page: _page, request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)

    await updateProduct(request, productId, { auto_provisioning: false })

    const deviceId = `autoprov-device-${Date.now()}`
    await issueCert(request, deviceId)

    const deviceStatusResponse = await request.get(
      `/api/admin/device/status?product_id=${SEED_PRODUCT_MODEL_NO}&device_id=${deviceId}&page=1&page_size=10`,
    )
    expect(deviceStatusResponse.ok(), 'Device status API should succeed').toBeTruthy()
    const deviceStatusBody = await deviceStatusResponse.json()
    const deviceRecord = deviceStatusBody.data?.[0]
    expect(deviceRecord, 'Device should be registered after cert issuance').toBeDefined()
    expect(deviceRecord.registration_source).toBe('Manual')

    await updateProduct(request, productId, { auto_provisioning: true })

    const afterToggleResponse = await request.get(
      `/api/admin/device/status?product_id=${SEED_PRODUCT_MODEL_NO}&device_id=${deviceId}&page=1&page_size=10`,
    )
    expect(afterToggleResponse.ok(), 'Device status API should succeed after toggle').toBeTruthy()
    const afterToggleBody = await afterToggleResponse.json()
    const afterToggleDevice = afterToggleBody.data?.[0]
    expect(afterToggleDevice.registration_source).toBe('Manual')

    await updateProduct(request, productId, { auto_provisioning: false })
  })
})
