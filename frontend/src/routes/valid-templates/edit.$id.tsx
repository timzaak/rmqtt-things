import { useEffect, useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useEventValidTemplate, useUpdateEventValidTemplate } from '@/hooks/useEvents'
import { SchemaEditor } from '@/components/schema/schema-editor'
import type { JSONSchema } from '@/components/schema/schema-editor'
import type { Value } from '@/lib/api-generated/types.gen'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const validTemplatesEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/valid-templates/edit/$id',
  component: ValidTemplatesEditPage,
})

export const Route = validTemplatesEditRoute

function ValidTemplatesEditPage() {
  const { id: idStr } = validTemplatesEditRoute.useParams()
  const id = Number(idStr)
  const navigate = useNavigate()
  const { data: template, isLoading } = useEventValidTemplate(id)
  const updateTemplate = useUpdateEventValidTemplate()

  const [description, setDescription] = useState('')
  const [schema, setSchema] = useState<JSONSchema>({ type: 'object', properties: {} })
  const [prevTemplate, setPrevTemplate] = useState<typeof template>(undefined)
  const [saved, setSaved] = useState(false)

  if (template && template !== prevTemplate) {
    setPrevTemplate(template)
    setDescription(template.description ?? '')
    setSchema((template.schema as JSONSchema) ?? { type: 'object', properties: {} })
  }

  const isDirty =
    !saved &&
    prevTemplate !== undefined &&
    template !== undefined &&
    (description !== (template.description ?? '') ||
      JSON.stringify(schema) !== JSON.stringify(template.schema))

  useEffect(() => {
    if (saved) navigate({ to: '/valid-templates' })
  }, [saved, navigate])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    updateTemplate.mutate(
      { id, description: description || null, schema: schema as Value },
      {
        onSuccess: () => setSaved(true),
        onError: (error) => {
          toast.error('Failed to update template', { description: error.message })
        },
      },
    )
  }

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  const inputClass =
    'w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100'
  const disabledInputClass =
    'w-full rounded-md border border-slate-300 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-400'

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit Template" />
      <form onSubmit={handleSubmit} className="space-y-6">
        <div>
          <h3 className="mb-3 text-base font-medium text-slate-800 dark:text-slate-200">Basic Info</h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-3">
            <div>
              <label htmlFor="productId" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
                Product ID
              </label>
              <input
                id="productId"
                type="text"
                disabled
                value={template?.product_id ?? ''}
                className={disabledInputClass}
                data-testid="template-edit-product-input"
              />
            </div>
            <div>
              <label htmlFor="event" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
                Event
              </label>
              <input
                id="event"
                type="text"
                disabled
                value={template?.event ?? ''}
                className={disabledInputClass}
                data-testid="template-edit-event-input"
              />
            </div>
            <div className="col-span-2">
              <label htmlFor="description" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
                Description
              </label>
              <textarea
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={3}
                className={inputClass}
                data-testid="template-edit-description-input"
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
            disabled={updateTemplate.isPending}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
            data-testid="template-edit-submit-button"
          >
            {updateTemplate.isPending ? 'Saving...' : 'Save'}
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
