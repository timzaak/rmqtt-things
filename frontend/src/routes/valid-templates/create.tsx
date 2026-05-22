import { useEffect, useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useCreateEventValidTemplate } from '@/hooks/useEvents'
import { SchemaEditor } from '@/components/schema/schema-editor'
import type { Value } from '@/lib/api-generated/types.gen'
import type { JSONSchema } from '@/components/schema/schema-editor'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const validTemplatesCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/valid-templates/create',
  component: ValidTemplatesCreatePage,
})

export const Route = validTemplatesCreateRoute

const defaultSchema: JSONSchema = { type: 'object', properties: {} }

const initialForm = {
  productId: '',
  event: '',
  description: '',
}

function ValidTemplatesCreatePage() {
  const navigate = useNavigate()
  const { data: products } = useProducts()
  const createTemplate = useCreateEventValidTemplate()

  const [form, setForm] = useState(initialForm)
  const [schema, setSchema] = useState<JSONSchema>(defaultSchema)
  const [saved, setSaved] = useState(false)

  const isDirty =
    !saved &&
    (form.productId !== '' ||
      form.event !== '' ||
      form.description !== '' ||
      schema !== defaultSchema)

  useEffect(() => {
    if (saved) navigate({ to: '/valid-templates' })
  }, [saved, navigate])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    createTemplate.mutate(
      {
        product_id: form.productId,
        event: form.event,
        description: form.description || null,
        schema: schema as Value,
      },
      {
        onSuccess: () => setSaved(true),
        onError: (error) => {
          toast.error('Failed to create template', { description: error.message })
        },
      }
    )
  }

  const inputClass =
    'w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100'

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Create Template" />
      <form onSubmit={handleSubmit} className="space-y-6">
        <div>
          <h3 className="mb-3 text-base font-medium text-slate-800 dark:text-slate-200">
            Basic Info
          </h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-3">
            <div>
              <label
                htmlFor="productId"
                className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
              >
                Product <span className="text-red-500">*</span>
              </label>
              <select
                id="productId"
                required
                value={form.productId}
                onChange={(e) => setForm((f) => ({ ...f, productId: e.target.value }))}
                className={inputClass}
                data-testid="template-create-product-select"
              >
                <option value="">Select a product</option>
                {(products?.data ?? []).map((p) => (
                  <option key={p.id} value={p.model_no}>
                    {p.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label
                htmlFor="event"
                className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
              >
                Event <span className="text-red-500">*</span>
              </label>
              <input
                id="event"
                type="text"
                required
                value={form.event}
                onChange={(e) => setForm((f) => ({ ...f, event: e.target.value }))}
                className={inputClass}
                data-testid="template-create-event-input"
              />
            </div>
            <div className="col-span-2">
              <label
                htmlFor="description"
                className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
              >
                Description
              </label>
              <textarea
                id="description"
                value={form.description}
                onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
                rows={3}
                className={inputClass}
                data-testid="template-create-description-input"
              />
            </div>
          </div>
        </div>
        <hr className="border-slate-200 dark:border-slate-700" />
        <div>
          <h3 className="mb-3 text-base font-medium text-slate-800 dark:text-slate-200">Schema</h3>
          <SchemaEditor value={schema} onChange={setSchema} />
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={createTemplate.isPending}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
            data-testid="template-create-submit-button"
          >
            {createTemplate.isPending ? 'Creating...' : 'Create'}
          </button>
          <Link
            to="/valid-templates"
            className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
