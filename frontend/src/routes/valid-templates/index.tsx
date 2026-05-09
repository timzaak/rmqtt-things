import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useEventValidTemplates, useUpdateEventValidTemplateStatus } from '@/hooks/useEvents'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import type { PaginatedResponseEventValidTemplate, EventValidTemplateStatus } from '@/lib/api-generated/types.gen'
import { formatDatetime } from '@/lib/utils'
import { toast } from '@/components/ui/sonner'

export const validTemplatesIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/valid-templates',
  component: ValidTemplatesIndexPage,
})

export const Route = validTemplatesIndexRoute

type TemplateRow = PaginatedResponseEventValidTemplate['data'][number]

const statusOptions: { value: EventValidTemplateStatus; label: string }[] = [
  { value: 'Draft', label: 'Draft' },
  { value: 'Active', label: 'Active' },
  { value: 'Inactive', label: 'Inactive' },
]

function useColumns() {
  const updateStatus = useUpdateEventValidTemplateStatus()

  const handleStatusChange = (id: number, status: EventValidTemplateStatus) => {
    updateStatus.mutate(
      { id, status },
      { onError: (error) => toast.error('Failed to update status', { description: error.message }) },
    )
  }

  const columns: Column<TemplateRow>[] = [
    { header: 'ID', accessor: 'id' },
    { header: 'Product ID', accessor: 'product_id' },
    { header: 'Event', accessor: 'event' },
    { header: 'Description', accessor: (row) => row.description ?? '-' },
    {
      header: 'Status',
      accessor: (row) => (
        <select
          value={row.status}
          onChange={(e) => handleStatusChange(row.id, e.target.value as EventValidTemplateStatus)}
          disabled={updateStatus.isPending}
          className="rounded-md border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
          data-testid={`template-status-select-${row.id}`}
        >
          {statusOptions.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      ),
    },
    { header: 'Created At', accessor: (row) => formatDatetime(row.created_at) },
    { header: 'Updated At', accessor: (row) => formatDatetime(row.updated_at) },
    {
      header: 'Actions',
      accessor: (row) => (
        <div className="flex gap-3">
          {row.status !== 'Active' && (
            <Link
              to="/valid-templates/edit/$id"
              params={{ id: String(row.id) }}
              className="text-sm text-blue-600 hover:underline dark:text-blue-400"
            >
              Edit
            </Link>
          )}
          <Link
            to="/valid-templates/show/$id"
            params={{ id: String(row.id) }}
            className="text-sm text-blue-600 hover:underline dark:text-blue-400"
          >
            View
          </Link>
        </div>
      ),
    },
  ]

  return columns
}

function ValidTemplatesIndexPage() {
  const [searchProductId, setSearchProductId] = useState<string>('')
  const [searchEvent, setSearchEvent] = useState<string>('')
  const [page, setPage] = useState(1)
  const columns = useColumns()

  const { data: products } = useProducts()
  const { data: result, isLoading } = useEventValidTemplates({
    product_id: searchProductId || null,
    event: searchEvent || null,
    page,
    page_size: 10,
  })

  const templates = result?.data ?? []
  const pagination = result?.pagination

  return (
    <div>
      <PageHeader
        title="Schema Templates"
        description="Manage event validation templates"
        actions={
          <Link
            to="/valid-templates/create"
            className="inline-flex h-9 items-center gap-1.5 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            <Plus className="h-4 w-4" />
            Create Template
          </Link>
        }
      />
      <SearchForm
        fields={[
          {
            name: 'product_id',
            label: 'Product',
            type: 'select',
            options: (products?.data ?? []).map((p) => ({ label: p.name, value: p.model_no })),
          },
          { name: 'event', label: 'Event', placeholder: 'Event Name' },
        ]}
        onSearch={(values) => {
          setSearchProductId(values.product_id)
          setSearchEvent(values.event)
          setPage(1)
        }}
      />
      <DataTable
        columns={columns}
        data={templates}
        loading={isLoading}
        emptyMessage="No templates found"
        pagination={
          pagination
            ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
            : undefined
        }
        onPageChange={setPage}
      />
    </div>
  )
}
