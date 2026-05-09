import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useEventValidTemplate } from '@/hooks/useEvents'
import { SchemaDisplay } from '@/components/schema/schema-display'
import type { JSONSchema } from '@/components/schema/schema-editor'
import { PageHeader } from '@/components/ui/page-header'
import { formatDatetime } from '@/lib/utils'

export const validTemplatesShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/valid-templates/show/$id',
  component: ValidTemplatesShowPage,
})

export const Route = validTemplatesShowRoute

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <dt className="mb-1 text-sm font-medium text-slate-700 dark:text-slate-300">{label}</dt>
      <dd className="text-sm text-slate-900 dark:text-slate-100">{children}</dd>
    </div>
  )
}

function ValidTemplatesShowPage() {
  const { id: idStr } = validTemplatesShowRoute.useParams()
  const id = Number(idStr)
  const { data: template, isLoading } = useEventValidTemplate(id)

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  if (!template) {
    return <div className="text-sm text-slate-500">Template not found.</div>
  }

  return (
    <div>
      <PageHeader title="Template Detail" />
      <h3 className="mb-3 text-base font-medium text-slate-800 dark:text-slate-200">Basic Info</h3>
      <dl className="grid grid-cols-3 gap-x-6 gap-y-3">
        <Field label="ID">{template.id}</Field>
        <Field label="Product ID">{template.product_id}</Field>
        <Field label="Event">{template.event}</Field>
        <Field label="Description">{template.description ?? '-'}</Field>
        <Field label="Status">{template.status}</Field>
        <Field label="Created At">{formatDatetime(template.created_at)}</Field>
        <Field label="Updated At">{formatDatetime(template.updated_at)}</Field>
      </dl>
      <hr className="my-6 border-slate-200 dark:border-slate-700" />
      <h3 className="mb-3 text-base font-medium text-slate-800 dark:text-slate-200">Schema</h3>
      <div>
        <SchemaDisplay schema={template.schema as JSONSchema} />
      </div>
      <div className="mt-6 flex gap-2">
        {template.status !== 'Active' && (
          <Link
            to="/valid-templates/edit/$id"
            params={{ id: String(id) }}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
            data-testid="template-show-edit-button"
          >
            Edit
          </Link>
        )}
        <Link
          to="/valid-templates"
          className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          data-testid="template-show-back-link"
        >
          Back to List
        </Link>
      </div>
    </div>
  )
}
