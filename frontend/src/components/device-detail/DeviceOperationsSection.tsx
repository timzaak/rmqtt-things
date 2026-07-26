import { useEffect, useState } from 'react'
import type { CommandStatus, DeviceOperationType } from '@/lib/api-generated/types.gen'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { extractErrorMessage, formatDatetime } from '@/lib/utils'
import { useDeviceOperations } from '@/hooks/useDeviceOperations'
import { useCreateActionInvocation } from '@/hooks/useActionInvocations'
import { sectionHeading } from './styles'
import { StatusBadge } from './StatusBadge'
import { ActionInvokeDialog } from './ActionInvokeDialog'
import { DirectPropertyWriteDialog } from './DirectPropertyWriteDialog'

// Deterministic UI summary rules: the type label is fixed per operation type,
// never inferred from the payload.
const TYPE_OPTIONS: { value: DeviceOperationType; label: string }[] = [
  { value: 'targetSync', label: 'Target sync' },
  { value: 'directPropertyWrite', label: 'Direct write' },
  { value: 'actionInvocation', label: 'Action' },
]

const TYPE_LABELS = Object.fromEntries(TYPE_OPTIONS.map((opt) => [opt.value, opt.label])) as Record<
  DeviceOperationType,
  string
>

const STATUS_OPTIONS: CommandStatus[] = ['Pending', 'Sent', 'Success', 'Failed', 'Deleted']

const filterStyle: React.CSSProperties = {
  height: '30px',
  borderRadius: '8px',
  border: '1px solid var(--color-border)',
  background: 'var(--color-surface-1)',
  color: 'var(--color-text-primary)',
  padding: '0 10px',
  fontSize: '13px',
  outline: 'none',
}

export function DeviceOperationsSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const [typeFilter, setTypeFilter] = useState<DeviceOperationType | null>(null)
  const [statusFilter, setStatusFilter] = useState<CommandStatus | null>(null)
  const [actionDialogOpen, setActionDialogOpen] = useState(false)
  const [moreMenuOpen, setMoreMenuOpen] = useState(false)
  const [directWriteOpen, setDirectWriteOpen] = useState(false)

  const { data, isLoading, isError, error } = useDeviceOperations({
    product_id: productId,
    device_id: deviceId,
    operation_type: typeFilter,
    status: statusFilter,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const createInvocation = useCreateActionInvocation()

  useEffect(() => {
    if (isError) {
      toast.error('Failed to load device operations', {
        description: extractErrorMessage(error),
      })
    }
  }, [isError, error])

  const columns: Column<Record<string, unknown>>[] = [
    {
      header: 'Time',
      accessor: (row) => formatDatetime(row.updatedTime as string),
    },
    { header: 'Operation', accessor: 'name' },
    {
      header: 'Type',
      accessor: (row) => TYPE_LABELS[row.operationType as DeviceOperationType],
    },
    {
      header: 'Status',
      accessor: (row) => <StatusBadge status={row.status as string} />,
    },
    {
      header: 'Details',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.payload, null, 2)}
        </pre>
      ),
    },
  ]

  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <h2 style={sectionHeading}>Operations</h2>
        <div className="flex items-center gap-2">
          <select
            data-testid="operation-type-filter"
            aria-label="Operation type"
            value={typeFilter ?? ''}
            onChange={(e) => {
              setTypeFilter((e.target.value || null) as DeviceOperationType | null)
              setPage(1)
            }}
            style={filterStyle}
          >
            <option value="">All types</option>
            {TYPE_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <select
            data-testid="operation-status-filter"
            aria-label="Operation status"
            value={statusFilter ?? ''}
            onChange={(e) => {
              setStatusFilter((e.target.value || null) as CommandStatus | null)
              setPage(1)
            }}
            style={filterStyle}
          >
            <option value="">All statuses</option>
            {STATUS_OPTIONS.map((status) => (
              <option key={status} value={status}>
                {status}
              </option>
            ))}
          </select>
          <button
            data-testid="run-action-button"
            onClick={() => setActionDialogOpen(true)}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
            style={{ background: 'var(--color-accent)' }}
          >
            Run Action
          </button>
          <div className="relative">
            <button
              data-testid="more-actions-button"
              onClick={() => setMoreMenuOpen((open) => !open)}
              className="rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors"
              style={{
                color: 'var(--color-text-secondary)',
                border: '1px solid var(--color-border)',
              }}
            >
              More ▾
            </button>
            {moreMenuOpen && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setMoreMenuOpen(false)} />
                <div
                  className="absolute right-0 z-50 mt-1 min-w-[180px] rounded-lg py-1 shadow-lg"
                  style={{
                    background: 'var(--color-surface-1)',
                    border: '1px solid var(--color-border)',
                  }}
                >
                  <button
                    data-testid="direct-property-write-button"
                    onClick={() => {
                      setMoreMenuOpen(false)
                      setDirectWriteOpen(true)
                    }}
                    className="w-full px-3 py-1.5 text-left text-[13px] transition-colors"
                    style={{ color: 'var(--color-text-primary)' }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = 'var(--color-surface-2)'
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = 'transparent'
                    }}
                  >
                    Direct property write
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
      {isError ? (
        <p
          data-testid="device-operations-error"
          className="rounded-xl border px-4 py-6 text-center text-[13px]"
          style={{
            background: 'var(--color-surface-1)',
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-muted)',
          }}
        >
          Failed to load device operations
        </p>
      ) : (
        <div data-testid="device-operations-table">
          <DataTable
            columns={columns}
            data={items as unknown as Record<string, unknown>[]}
            loading={isLoading}
            emptyMessage="No operations"
            pagination={
              pagination
                ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
                : undefined
            }
            onPageChange={setPage}
          />
        </div>
      )}
      {actionDialogOpen && (
        <ActionInvokeDialog
          productId={productId}
          deviceId={deviceId}
          onClose={() => setActionDialogOpen(false)}
          onSubmit={(request) => {
            createInvocation.mutate(request, {
              onSuccess: () => {
                setActionDialogOpen(false)
              },
            })
          }}
          isSubmitting={createInvocation.isPending}
        />
      )}
      {directWriteOpen && (
        <DirectPropertyWriteDialog
          productId={productId}
          deviceId={deviceId}
          onClose={() => setDirectWriteOpen(false)}
        />
      )}
    </section>
  )
}
