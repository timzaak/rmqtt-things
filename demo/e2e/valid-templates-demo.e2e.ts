/**
 * Valid Templates (Schema) Demo CRUD 测试
 *
 * 对应用户故事：
 * - US-PA-008 查看校验模板列表
 * - 创建校验模板
 * - 编辑校验模板
 * - 查看校验模板详情
 * - 状态管理（Draft/Active/Inactive）
 *
 * 验证管理员可以通过后台进行 Schema Templates 的完整 CRUD 操作。
 * 前置条件：系统中已有产品（种子数据中有 demo_product）。
 *
 * 注意：后端没有 delete 端点，因此通过状态变更为 Inactive 来替代删除测试。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { APIRequestContext } from '@playwright/test'
import { SELECTORS } from './selectors'

const VT = SELECTORS.validTemplates
const FRONTEND_URL = process.env.FRONTEND_URL || 'http://localhost:3000'
const PRODUCT_ID = 'demo_product'

interface TemplateListResponse {
  data: Array<{
    id: number
    product_id: string
    event: string
    description: string | null
    schema: unknown
    status: 'Draft' | 'Active' | 'Inactive'
    created_at: string
    updated_at: string
  }>
  pagination: { page: number; page_size: number; total: number }
}

interface TemplateDetailResponse {
  id: number
  product_id: string
  event: string
  description: string | null
  schema: unknown
  status: 'Draft' | 'Active' | 'Inactive'
  created_at: string
  updated_at: string
}

async function getJson<T>(request: APIRequestContext, path: string): Promise<T> {
  const response = await request.get(path)
  if (!response.ok()) {
    const text = await response.text()
    throw new Error(`GET ${path} returned ${response.status()}: ${text}`)
  }
  return response.json()
}

const defaultSchema = { type: 'object', properties: { temperature: { type: 'number' } } }

async function createTemplateViaApi(
  request: APIRequestContext,
  overrides: { event?: string; description?: string | null; schema?: object } = {},
) {
  const ts = Date.now()
  const data = {
    product_id: PRODUCT_ID,
    event: `e2e_event_${ts}`,
    description: `E2E test template ${ts}`,
    schema: defaultSchema,
    ...overrides,
  }
  const response = await request.post('/api/admin/valid/event', { data })
  if (response.status() !== 201) {
    const text = await response.text()
    throw new Error(`Create template failed: ${response.status()} ${text}`)
  }

  const listBody = await getJson<TemplateListResponse>(
    request,
    `/api/admin/valid/event?product_id=${PRODUCT_ID}&page=1&page_size=50`,
  )
  const created = listBody.data.find((t) => t.event === data.event)
  if (!created) {
    throw new Error(`Created template not found in list (event: ${data.event})`)
  }
  return created
}

async function setTemplateStatus(
  request: APIRequestContext,
  id: number,
  status: 'Draft' | 'Active' | 'Inactive',
) {
  const response = await request.patch(`/api/admin/valid/event/${id}/status`, {
    data: { status },
  })
  if (response.status() !== 200) {
    const text = await response.text()
    throw new Error(`Update status failed: ${response.status()} ${text}`)
  }
}

test.describe('Valid Templates smoke', () => {
  test('shows templates list page with actions', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates`)

    await expect(page.getByRole('heading', { name: 'Schema Templates' })).toBeVisible()
    await expect(page.getByText('Manage event validation templates')).toBeVisible()
    await expect(page.getByRole('link', { name: 'Create Template' })).toBeVisible()
  })

  test('shows search form with product and event filters', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates`)

    const form = page.locator('form')
    await expect(form.getByText('Product', { exact: true })).toBeVisible()
    await expect(form.getByText('Event', { exact: true })).toBeVisible()
    await expect(form.getByRole('button', { name: 'Search' })).toBeVisible()
  })

  test('navigates to create template page', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates`)

    await page.getByRole('link', { name: 'Create Template' }).click()
    await expect(page).toHaveURL(new RegExp(`${FRONTEND_URL}/valid-templates/create`))
    await expect(page.getByRole('heading', { name: 'Create Template' })).toBeVisible()
  })

  test('create template form has required fields', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates/create`)

    const form = page.locator('form')
    await expect(form.locator('label', { hasText: /^Product/ })).toBeVisible()
    await expect(form.locator('label', { hasText: /^Event/ })).toBeVisible()
    await expect(form.locator('label', { hasText: /^Description/ })).toBeVisible()
    await expect(form.getByRole('button', { name: 'Create' })).toBeVisible()
  })
})

test.describe('Create template', () => {
  test('creates a new template via web form', async ({ page, request }) => {
    const uniqueSuffix = Date.now()
    const eventName = `web_create_${uniqueSuffix}`
    const description = `E2E web create test ${uniqueSuffix}`
    let templateId: number | undefined

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates/create`)
      await expect(page.getByRole('heading', { name: 'Create Template' })).toBeVisible()

      await page.locator(VT.createProductSelect).selectOption(PRODUCT_ID)
      await page.locator(VT.createEventInput).fill(eventName)
      await page.locator(VT.createDescriptionInput).fill(description)
      await page.locator(VT.createSubmitButton).click()

      await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/valid-templates$`))
      await expect(page.getByText(eventName)).toBeVisible()

      const listBody = await getJson<TemplateListResponse>(
        request,
        `/api/admin/valid/event?product_id=${PRODUCT_ID}&page=1&page_size=50`,
      )
      const created = listBody.data.find((t) => t.event === eventName)
      templateId = created?.id
    } finally {
      if (templateId !== undefined) {
        await setTemplateStatus(request, templateId, 'Inactive').catch(() => {})
      }
    }
  })

  test('rejects creation with missing required fields', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates/create`)
    await expect(page.getByRole('heading', { name: 'Create Template' })).toBeVisible()

    // Click Create without filling anything -- browser HTML5 validation should block
    await page.locator(VT.createSubmitButton).click()

    await expect(page).toHaveURL(new RegExp('/valid-templates/create'))
  })
})

test.describe('List templates', () => {
  test('shows template data in table rows', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `list_test_${Date.now()}`,
      description: 'List display test',
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)
      await expect(page.getByRole('heading', { name: 'Schema Templates' })).toBeVisible()

      const row = page.getByRole('row').filter({ hasText: template.event })
      await expect(row).toBeVisible()
      await expect(row.getByText(PRODUCT_ID)).toBeVisible()
      await expect(row.getByText('List display test')).toBeVisible()
      await expect(row.locator(`select[data-testid="template-status-select-${template.id}"]`)).toHaveValue('Draft')
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('shows empty message when no templates match', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/valid-templates`)
    await expect(page.getByRole('heading', { name: 'Schema Templates' })).toBeVisible()

    await page.getByLabel('Event').fill('nonexistent_event_xyz_99999')
    await page.getByRole('button', { name: 'Search' }).click()

    await expect(page.getByText('No templates found')).toBeVisible()
  })

  test('shows View and Edit links for Draft templates', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `link_test_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      const row = page.getByRole('row').filter({ hasText: template.event })
      await expect(row.getByRole('link', { name: 'Edit' })).toBeVisible()
      await expect(row.getByRole('link', { name: 'View' })).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('hides Edit link for Active templates', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `active_link_${Date.now()}`,
    })

    try {
      await setTemplateStatus(request, template.id, 'Active')

      await page.goto(`${FRONTEND_URL}/valid-templates`)

      const row = page.getByRole('row').filter({ hasText: template.event })
      await expect(row).toBeVisible()
      await expect(row.getByRole('link', { name: 'Edit' })).not.toBeVisible()
      await expect(row.getByRole('link', { name: 'View' })).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })
})

test.describe('Search and filter templates', () => {
  test('filters by product select', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `filter_prod_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      await page.locator('form select').first().selectOption(PRODUCT_ID)
      await page.getByRole('button', { name: 'Search' }).click()

      await expect(page.getByText(template.event)).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('filters by event name', async ({ page, request }) => {
    const ts = Date.now()
    const template = await createTemplateViaApi(request, {
      event: `filter_evt_${ts}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      await page.getByLabel('Event').fill(template.event)
      await page.getByRole('button', { name: 'Search' }).click()

      await expect(page.getByText(template.event)).toBeVisible()

      await page.getByLabel('Event').clear()
      await page.getByRole('button', { name: 'Search' }).click()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('clearing filters restores list', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `filter_clear_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      await page.getByLabel('Event').fill('nonexistent_xyz_99999')
      await page.getByRole('button', { name: 'Search' }).click()
      await expect(page.getByText('No templates found')).toBeVisible()

      await page.getByLabel('Event').clear()
      await page.getByRole('button', { name: 'Search' }).click()

      await expect(page.getByText(template.event)).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })
})

test.describe('Update template', () => {
  test('edits description via web form', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `edit_desc_${Date.now()}`,
      description: 'Original description',
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)
      const row = page.getByRole('row').filter({ hasText: template.event })
      await row.getByRole('link', { name: 'Edit' }).click()

      await expect(page.getByRole('heading', { name: 'Edit Template' })).toBeVisible()

      await expect(page.locator(VT.editProductInput)).toBeDisabled()
      await expect(page.locator(VT.editEventInput)).toBeDisabled()
      await expect(page.locator(VT.editDescriptionInput)).toHaveValue('Original description')

      const updatedDescription = `Updated ${Date.now()}`
      await page.locator(VT.editDescriptionInput).clear()
      await page.locator(VT.editDescriptionInput).fill(updatedDescription)

      await page.locator(VT.editSubmitButton).click()

      await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/valid-templates$`))
      await expect(page.getByText(updatedDescription)).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('edits template via API and verifies changes', async ({ request }) => {
    const template = await createTemplateViaApi(request, {
      event: `edit_api_${Date.now()}`,
      description: 'Before update',
    })

    try {
      const updatedSchema = { type: 'object', properties: { humidity: { type: 'string' } } }
      const updateResponse = await request.patch(`/api/admin/valid/event/${template.id}`, {
        data: {
          description: 'After update',
          schema: updatedSchema,
        },
      })
      expect(updateResponse.status()).toBe(200)

      const detail = await getJson<TemplateDetailResponse>(
        request,
        `/api/admin/valid/event/${template.id}`,
      )
      expect(detail.description).toBe('After update')
      expect(detail.schema).toEqual(updatedSchema)
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('status dropdown changes template status in list', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `status_change_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      const statusSelect = page.locator(`select[data-testid="template-status-select-${template.id}"]`)
      await statusSelect.selectOption('Active')

      // Wait for the mutation to complete by verifying the Edit link disappears (Active templates hide Edit)
      const row = page.getByRole('row').filter({ hasText: template.event })
      await expect(row.getByRole('link', { name: 'Edit' })).not.toBeVisible()

      const detail = await getJson<TemplateDetailResponse>(
        request,
        `/api/admin/valid/event/${template.id}`,
      )
      expect(detail.status).toBe('Active')
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })
})

test.describe('View template detail', () => {
  test('shows template detail page with all fields', async ({ page, request }) => {
    const ts = Date.now()
    const template = await createTemplateViaApi(request, {
      event: `detail_view_${ts}`,
      description: 'Detail page test',
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)
      const row = page.getByRole('row').filter({ hasText: template.event })
      await row.getByRole('link', { name: 'View' }).click()

      await expect(page.getByRole('heading', { name: 'Template Detail' })).toBeVisible()

      await expect(page.getByText(PRODUCT_ID)).toBeVisible()
      await expect(page.getByText(template.event)).toBeVisible()
      await expect(page.getByText('Detail page test')).toBeVisible()
      await expect(page.getByText('Draft')).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('shows Edit button for Draft templates on detail page', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `detail_edit_btn_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates/show/${template.id}`)
      await expect(page.getByRole('heading', { name: 'Template Detail' })).toBeVisible()

      await expect(page.locator(VT.showEditButton)).toBeVisible()
      await expect(page.locator(VT.showBackLink)).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('hides Edit button for Active templates on detail page', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `detail_active_${Date.now()}`,
    })

    try {
      await setTemplateStatus(request, template.id, 'Active')

      await page.goto(`${FRONTEND_URL}/valid-templates/show/${template.id}`)
      await expect(page.getByRole('heading', { name: 'Template Detail' })).toBeVisible()

      await expect(page.locator(VT.showEditButton)).not.toBeVisible()
      await expect(page.locator(VT.showBackLink)).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('navigates from detail to edit page', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `detail_to_edit_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates/show/${template.id}`)
      await expect(page.getByRole('heading', { name: 'Template Detail' })).toBeVisible()

      await page.locator(VT.showEditButton).click()
      await expect(page).toHaveURL(new RegExp('/valid-templates/edit/'))
      await expect(page.getByRole('heading', { name: 'Edit Template' })).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('navigates back to list from detail page', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `detail_back_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates/show/${template.id}`)
      await expect(page.getByRole('heading', { name: 'Template Detail' })).toBeVisible()

      await page.locator(VT.showBackLink).click()
      await expect(page).toHaveURL(new RegExp(`^${FRONTEND_URL}/valid-templates$`))
      await expect(page.getByRole('heading', { name: 'Schema Templates' })).toBeVisible()
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })
})

test.describe('Deactivate template', () => {
  test('sets template to Inactive via status dropdown', async ({ page, request }) => {
    const template = await createTemplateViaApi(request, {
      event: `deactivate_${Date.now()}`,
    })

    try {
      await page.goto(`${FRONTEND_URL}/valid-templates`)

      await expect(page.getByText(template.event)).toBeVisible()

      const statusSelect = page.locator(`select[data-testid="template-status-select-${template.id}"]`)
      await statusSelect.selectOption('Inactive')

      // Wait for the async status mutation to complete by polling the API
      await expect(async () => {
        const detail = await getJson<TemplateDetailResponse>(
          request,
          `/api/admin/valid/event/${template.id}`,
        )
        expect(detail.status).toBe('Inactive')
      }).toPass({ timeout: 5000 })
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })

  test('API: setting Inactive then reverting to Draft works', async ({ request }) => {
    const template = await createTemplateViaApi(request, {
      event: `status_cycle_${Date.now()}`,
    })

    try {
      await setTemplateStatus(request, template.id, 'Active')
      let detail = await getJson<TemplateDetailResponse>(
        request,
        `/api/admin/valid/event/${template.id}`,
      )
      expect(detail.status).toBe('Active')

      await setTemplateStatus(request, template.id, 'Inactive')
      detail = await getJson<TemplateDetailResponse>(
        request,
        `/api/admin/valid/event/${template.id}`,
      )
      expect(detail.status).toBe('Inactive')

      await setTemplateStatus(request, template.id, 'Draft')
      detail = await getJson<TemplateDetailResponse>(
        request,
        `/api/admin/valid/event/${template.id}`,
      )
      expect(detail.status).toBe('Draft')
    } finally {
      await setTemplateStatus(request, template.id, 'Inactive').catch(() => {})
    }
  })
})
