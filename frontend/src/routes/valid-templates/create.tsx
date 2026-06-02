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

  const inputStyle: React.CSSProperties = {
    width: '100%',
    borderRadius: '6px',
    border: '1px solid var(--color-border)',
    padding: '8px 12px',
    fontSize: '13px',
    background: 'var(--color-surface-1)',
    color: 'var(--color-text-primary)',
  }
  const labelStyle: React.CSSProperties = {
    display: 'block',
    marginBottom: '4px',
    fontSize: '13px',
    fontWeight: 500,
    color: 'var(--color-text-secondary)',
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Create Template" />
      <form onSubmit={handleSubmit} className="space-y-6">
        <div>
          <h3
            style={{
              marginBottom: '12px',
              fontSize: '15px',
              fontWeight: 600,
              color: 'var(--color-text-primary)',
            }}
          >
            Basic Info
          </h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-3">
            <div>
              <label htmlFor="productId" style={labelStyle}>
                Product <span style={{ color: '#dc2626' }}>*</span>
              </label>
              <select
                id="productId"
                required
                value={form.productId}
                onChange={(e) => setForm((f) => ({ ...f, productId: e.target.value }))}
                style={inputStyle}
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
              <label htmlFor="event" style={labelStyle}>
                Event <span style={{ color: '#dc2626' }}>*</span>
              </label>
              <input
                id="event"
                type="text"
                required
                value={form.event}
                onChange={(e) => setForm((f) => ({ ...f, event: e.target.value }))}
                style={inputStyle}
                data-testid="template-create-event-input"
              />
            </div>
            <div className="col-span-2">
              <label htmlFor="description" style={labelStyle}>
                Description
              </label>
              <textarea
                id="description"
                value={form.description}
                onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
                rows={3}
                style={inputStyle}
                data-testid="template-create-description-input"
              />
            </div>
          </div>
        </div>
        <hr style={{ borderColor: 'var(--color-border)' }} />
        <div>
          <h3
            style={{
              marginBottom: '12px',
              fontSize: '15px',
              fontWeight: 600,
              color: 'var(--color-text-primary)',
            }}
          >
            Schema
          </h3>
          <SchemaEditor value={schema} onChange={setSchema} />
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={createTemplate.isPending}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'var(--color-accent)',
              color: '#fff',
              opacity: createTemplate.isPending ? 0.5 : 1,
            }}
            data-testid="template-create-submit-button"
          >
            {createTemplate.isPending ? 'Creating...' : 'Create'}
          </button>
          <Link
            to="/valid-templates"
            style={{
              borderRadius: '6px',
              border: '1px solid var(--color-border)',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              color: 'var(--color-text-secondary)',
            }}
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
