import { useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useEventValidTemplate, useDeleteEventValidTemplate } from '@/hooks/useEvents'
import { SchemaDisplay } from '@/components/schema/schema-display'
import type { JSONSchema } from '@/components/schema/schema-editor'
import { PageHeader } from '@/components/ui/page-header'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { toast } from '@/components/ui/sonner'
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
      <dt
        style={{
          marginBottom: '4px',
          fontSize: '13px',
          fontWeight: 500,
          color: 'var(--color-text-secondary)',
        }}
      >
        {label}
      </dt>
      <dd style={{ fontSize: '13px', color: 'var(--color-text-primary)' }}>{children}</dd>
    </div>
  )
}

function ValidTemplatesShowPage() {
  const { id: idStr } = validTemplatesShowRoute.useParams()
  const id = Number(idStr)
  const { data: template, isLoading } = useEventValidTemplate(id)
  const deleteMutation = useDeleteEventValidTemplate()
  const navigate = useNavigate()
  const [confirmDelete, setConfirmDelete] = useState(false)

  function handleDelete() {
    deleteMutation.mutate(id, {
      onSuccess: () => navigate({ to: '/valid-templates' }),
      onError: (error) => {
        toast.error('Failed to delete template', { description: error.message })
        setConfirmDelete(false)
      },
    })
  }

  if (isLoading) {
    return <div style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>Loading...</div>
  }

  if (!template) {
    return (
      <div style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>Template not found.</div>
    )
  }

  return (
    <div>
      <PageHeader title="Template Detail" />
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
      <dl className="grid grid-cols-3 gap-x-6 gap-y-3">
        <Field label="ID">{template.id}</Field>
        <Field label="Product ID">{template.product_id}</Field>
        <Field label="Event">{template.event}</Field>
        <Field label="Description">{template.description ?? '-'}</Field>
        <Field label="Status">{template.status}</Field>
        <Field label="Created At">{formatDatetime(template.created_at)}</Field>
        <Field label="Updated At">{formatDatetime(template.updated_at)}</Field>
      </dl>
      <hr style={{ margin: '24px 0', borderColor: 'var(--color-border)' }} />
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
      <div>
        <SchemaDisplay schema={template.schema as JSONSchema} />
      </div>
      <div className="mt-6 flex gap-2">
        {template.status !== 'Active' && (
          <Link
            to="/valid-templates/edit/$id"
            params={{ id: String(id) }}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'var(--color-accent)',
              color: '#fff',
            }}
            data-testid="template-show-edit-button"
          >
            Edit
          </Link>
        )}
        {template.status !== 'Active' && (
          <button
            onClick={() => setConfirmDelete(true)}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: '#dc2626',
              color: '#fff',
            }}
            data-testid="template-show-delete-button"
          >
            Delete
          </button>
        )}
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
          data-testid="template-show-back-link"
        >
          Back to List
        </Link>
      </div>
      <ConfirmDialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title="Delete Template"
        description={`Are you sure you want to delete template "${template.event}"?`}
        onConfirm={handleDelete}
        confirmText="Delete"
        variant="danger"
      />
    </div>
  )
}
