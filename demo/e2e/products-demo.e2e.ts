/**
 * Products Demo 测试
 *
 * 对应用户故事：
 * - US-PA-001 创建产品
 * - US-PA-002 查看产品列表
 * - US-PA-003 编辑产品
 *
 * 前置条件：后端已启动且有种子数据。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { SELECTORS } from './selectors'

const P = SELECTORS.products
const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'

test.describe('Products demo', () => {
  test('shows seeded products', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/products`)

    await expect(page.getByRole('heading', { name: 'Products' })).toBeVisible()
    await expect(page.getByRole('link', { name: P.createLink })).toBeVisible()

    await expect(page.getByText('Demo Smart Light')).toBeVisible()
    await expect(page.getByText('demo_product')).toBeVisible()
    await expect(page.getByText('Default product for RMQTT Things demo')).toBeVisible()
  })

  test('creates a new product', async ({ page }) => {
    const uniqueSuffix = Date.now()
    const productName = `Test Light ${uniqueSuffix}`
    const modelNo = `test-model-${uniqueSuffix}`

    await page.goto(`${FRONTEND_URL}/products`)

    await page.getByRole('link', { name: P.createLink }).click()
    await expect(page.getByRole('heading', { name: 'Create Product' })).toBeVisible()

    await page.getByLabel(P.nameInput).fill(productName)
    await page.getByLabel(P.modelNoInput).fill(modelNo)
    await page.getByLabel(P.descriptionInput).fill(`E2E test product ${uniqueSuffix}`)

    await page.getByRole('button', { name: P.createButton }).click()

    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/products$`))
    await expect(page.getByText(productName)).toBeVisible()
  })

  test('rejects duplicate model number', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/products/create`)

    await expect(page.getByRole('heading', { name: 'Create Product' })).toBeVisible()

    await page.getByLabel(P.nameInput).fill('Duplicate Product')
    await page.getByLabel(P.modelNoInput).fill('demo_product')
    await page.getByLabel(P.descriptionInput).fill('Should fail')

    await page.getByRole('button', { name: P.createButton }).click()

    await expect(page.locator(SELECTORS.common.errorMessage)).toBeVisible()
    await expect(page).toHaveURL(new RegExp('/products/create'))
  })

  test('edits an existing product', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/products`)

    await expect(page.getByText('Demo Smart Light')).toBeVisible()

    const seedRow = page.getByRole('row').filter({ hasText: 'Demo Smart Light' })
    await seedRow.getByRole('link', { name: P.editLink }).click()

    await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()
    await expect(page.getByLabel(P.modelNoInput)).toBeDisabled()

    const updatedName = 'Demo Smart Light Updated'
    await page.getByLabel(P.nameInput).clear()
    await page.getByLabel(P.nameInput).fill(updatedName)

    await page.getByRole('button', { name: P.saveButton }).click()

    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/products$`))
    await expect(page.getByText(updatedName)).toBeVisible()

    // Restore original name to avoid polluting seed data for other tests
    await page.getByRole('row').filter({ hasText: updatedName }).getByRole('link', { name: P.editLink }).click()
    await expect(page.getByRole('heading', { name: 'Edit Product' })).toBeVisible()
    await page.getByLabel(P.nameInput).clear()
    await page.getByLabel(P.nameInput).fill('Demo Smart Light')
    await page.getByRole('button', { name: P.saveButton }).click()
    await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/products$`))
    await expect(page.getByText('Demo Smart Light')).toBeVisible()
  })

  test('filters products by search keyword', async ({ page, request }) => {
    const uniqueSuffix = Date.now()
    const productName = `Search Test ${uniqueSuffix}`
    const modelNo = `search-model-${uniqueSuffix}`

    let productId: number | undefined
    try {
      const createResp = await request.post('/api/admin/product', {
        data: { name: productName, model_no: modelNo, description: 'For search test' },
      })
      expect(createResp.status()).toBe(201)
      const created = await createResp.json()
      productId = created.id

      await page.goto(`${FRONTEND_URL}/products`)
      await expect(page.getByText(productName)).toBeVisible()

      await page.getByLabel(P.searchInput).fill(productName)
      await page.getByRole('button', { name: P.searchButton }).click()

      await expect(page.getByText(productName)).toBeVisible()
      await expect(page.getByText('Demo Smart Light')).not.toBeVisible()

      await page.getByLabel(P.searchInput).clear()
      await page.getByRole('button', { name: P.searchButton }).click()
      await expect(page.getByText('Demo Smart Light')).toBeVisible()
      await expect(page.getByText(productName)).toBeVisible()
    } finally {
      // No DELETE endpoint, rename to mark as cleaned
      if (productId !== undefined) {
        await request.patch(`/api/admin/product/${productId}`, {
          data: { name: `[E2E-cleaned] ${productName}`, description: 'Cleaned up by E2E test' },
        })
      }
    }
  })
})
