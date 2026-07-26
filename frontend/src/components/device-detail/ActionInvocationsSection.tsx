import { useState } from 'react'
import type { CreateActionCommandRequest } from '@/lib/api-generated/types.gen'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading } from './styles'
import { StatusBadge } from './StatusBadge'
import {
  useActionInvocations,
  useCreateActionInvocation,
  useDeleteActionInvocations,
} from '@/hooks/useActionInvocations'
import { ActionInvokeDialog } from './ActionInvokeDialog'

export function ActionInvocationsSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const [dialogOpen, setDialogOpen] = useState(false)

  const { data, isLoading } = useActionInvocations({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const createInvocation = useCreateActionInvocation()
  const deleteInvocations = useDeleteActionInvocations()

  const handleDelete = (invocationId: number) => {
    deleteInvocations.mutate([invocationId])
  }

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    { header: 'Service Type', accessor: 'serviceType' },
    {
      header: 'Params',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.params, null, 2)}
        </pre>
      ),
    },
    {
      header: 'Status',
      accessor: (row) => <StatusBadge status={row.status as string} />,
    },
    {
      header: 'Created Time',
      accessor: (row) => formatDatetime(row.createdTime as string),
    },
    {
      header: 'Updated Time',
      accessor: (row) => formatDatetime(row.updatedTime as string),
    },
    {
      header: 'Actions',
      accessor: (row) =>
        row.status === 'Pending' ? (
          <button
            onClick={() => handleDelete(row.id as number)}
            disabled={deleteInvocations.isPending}
            className="text-[12px] font-medium hover:underline disabled:opacity-50"
            style={{ color: '#dc2626' }}
          >
            Delete
          </button>
        ) : null,
    },
  ]

  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <h2 style={sectionHeading}>Action Invocations</h2>
        <button
          data-testid="action-invoke-button"
          onClick={() => setDialogOpen(true)}
          className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          style={{ background: 'var(--color-accent)' }}
        >
          Invoke Action
        </button>
      </div>
      <div data-testid="action-invocation-table">
        <DataTable
          columns={columns}
          data={items as unknown as Record<string, unknown>[]}
          loading={isLoading}
          emptyMessage="No action invocations"
          pagination={
            pagination
              ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
              : undefined
          }
          onPageChange={setPage}
        />
      </div>
      {dialogOpen && (
        <ActionInvokeDialog
          productId={productId}
          deviceId={deviceId}
          onClose={() => setDialogOpen(false)}
          onSubmit={(request: CreateActionCommandRequest) => {
            createInvocation.mutate(request, {
              onSuccess: () => {
                setDialogOpen(false)
              },
            })
          }}
          isSubmitting={createInvocation.isPending}
        />
      )}
    </section>
  )
}
