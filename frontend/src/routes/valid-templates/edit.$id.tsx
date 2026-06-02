import { useEffect, useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { Info } from 'lucide-react'
import { rootRoute } from '../__root'
import {
  useEventValidTemplate,
  useUpdateEventValidTemplate,
  useUpdateEventValidTemplateStatus,
} from '@/hooks/useEvents'
import { SchemaEditor } from '@/components/schema/schema-editor'
import type { JSONSchema } from '@/components/schema/schema-editor'
import type { EventValidTemplateStatus, Value } from '@/lib/api-generated/types.gen'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const validTemplatesEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/valid-templates/edit/$id',
  component: ValidTemplatesEditPage,
})

export const Route = validTemplatesEditRoute

const statusOptions: { value: EventValidTemplateStatus; label: string }[] = [
  { value: 'Draft', label: 'Draft' },
  { value: 'Active', label: 'Active' },
  { value: 'Inactive', label: 'Inactive' },
]

function ValidTemplatesEditPage() {
  const { id: idStr } = validTemplatesEditRoute.useParams()
  const id = Number(idStr)
  const navigate = useNavigate()
  const { data: template, isLoading } = useEventValidTemplate(id)
  const updateTemplate = useUpdateEventValidTemplate()
  const updateStatus = useUpdateEventValidTemplateStatus()

  const [description, setDescription] = useState('')
  const [status, setStatus] = useState<EventValidTemplateStatus>('Draft')
  const [schema, setSchema] = useState<JSONSchema>({ type: 'object', properties: {} })
  const [prevDataKey, setPrevDataKey] = useState<string>('')
  const [saved, setSaved] = useState(false)

  const isActive = template?.status === 'Active'

  const dataKey = template
    ? JSON.stringify({ d: template.description, s: template.status, sc: template.schema })
    : ''

  if (template && dataKey !== prevDataKey) {
    setPrevDataKey(dataKey)
    setDescription(template.description ?? '')
    setStatus(template.status)
    setSchema((template.schema as JSONSchema) ?? { type: 'object', properties: {} })
  }

  const isDirty =
    !saved &&
    template !== undefined &&
    dataKey !== '' &&
    (description !== (template.description ?? '') ||
      status !== template.status ||
      JSON.stringify(schema) !== JSON.stringify(template.schema))

  useEffect(() => {
    if (saved) navigate({ to: '/valid-templates' })
  }, [saved, navigate])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const statusChanged = status !== template?.status
    const contentChanged =
      description !== (template?.description ?? '') ||
      JSON.stringify(schema) !== JSON.stringify(template?.schema)

    if (!statusChanged && !contentChanged) {
      setSaved(true)
      return
    }

    const navigateOnSuccess = () => setSaved(true)

    if (statusChanged) {
      updateStatus.mutate(
        { id, status },
        {
          onSuccess: () => {
            if (!contentChanged) navigateOnSuccess()
          },
          onError: (error) => {
            toast.error('Failed to update status', { description: error.message })
          },
        }
      )
    }

    if (contentChanged) {
      updateTemplate.mutate(
        { id, description: description || null, schema: schema as Value },
        {
          onSuccess: navigateOnSuccess,
          onError: (error) => {
            toast.error('Failed to update template', { description: error.message })
          },
        }
      )
    }
  }

  const isPending = updateTemplate.isPending || updateStatus.isPending

  if (isLoading) {
    return <div style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>Loading...</div>
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
  const disabledStyle: React.CSSProperties = {
    width: '100%',
    borderRadius: '6px',
    border: '1px solid var(--color-border)',
    padding: '8px 12px',
    fontSize: '13px',
    background: 'var(--color-surface-2)',
    color: 'var(--color-text-muted)',
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
      <PageHeader title="Edit Template" />
      {isActive && (
        <div
          style={{
            marginBottom: '24px',
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            borderRadius: '6px',
            border: '1px solid #fbbf24',
            background: '#fffbeb',
            padding: '12px 16px',
            fontSize: '13px',
            color: '#92400e',
          }}
          data-testid="template-edit-active-notice"
        >
          <Info className="h-4 w-4 shrink-0" />
          <span>
            This template is currently <strong>Active</strong>. The schema is read-only. You can
            still change the description and status.
          </span>
        </div>
      )}
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
                Product ID
              </label>
              <input
                id="productId"
                type="text"
                disabled
                value={template?.product_id ?? ''}
                style={disabledStyle}
                data-testid="template-edit-product-input"
              />
            </div>
            <div>
              <label htmlFor="event" style={labelStyle}>
                Event
              </label>
              <input
                id="event"
                type="text"
                disabled
                value={template?.event ?? ''}
                style={disabledStyle}
                data-testid="template-edit-event-input"
              />
            </div>
            <div>
              <label htmlFor="status" style={labelStyle}>
                Status
              </label>
              <select
                id="status"
                value={status}
                onChange={(e) => setStatus(e.target.value as EventValidTemplateStatus)}
                disabled={updateStatus.isPending}
                style={inputStyle}
                data-testid="template-edit-status-select"
              >
                {statusOptions.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="col-span-2">
              <label htmlFor="description" style={labelStyle}>
                Description
              </label>
              <textarea
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={3}
                style={inputStyle}
                data-testid="template-edit-description-input"
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
          <SchemaEditor value={schema} onChange={setSchema} disabled={isActive} />
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={isPending}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'var(--color-accent)',
              color: '#fff',
              opacity: isPending ? 0.5 : 1,
            }}
            data-testid="template-edit-submit-button"
          >
            {isPending ? 'Saving...' : 'Save'}
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
