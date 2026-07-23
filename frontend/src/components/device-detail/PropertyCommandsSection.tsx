import { useState } from 'react'
import {
  usePropertyCommands,
  useCreatePropertyCommand,
  useDeletePropertyCommands,
} from '@/hooks/useProperties'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading } from './styles'

export function PropertyCommandsSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const [dialogOpen, setDialogOpen] = useState(false)

  const { data, isLoading } = usePropertyCommands({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const createCommand = useCreatePropertyCommand()
  const deleteCommands = useDeletePropertyCommands()

  const handleDelete = (commandId: number) => {
    deleteCommands.mutate([commandId])
  }

  const statusColors: Record<string, string> = {
    Pending: '#d97706',
    Sent: 'var(--color-accent)',
    Success: '#059669',
    Failed: '#dc2626',
    Deleted: 'var(--color-text-muted)',
  }

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Command',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.command, null, 2)}
        </pre>
      ),
    },
    {
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        return (
          <span
            className="text-[12px] font-semibold"
            style={{ color: statusColors[status] ?? 'var(--color-text-secondary)' }}
          >
            {status}
          </span>
        )
      },
    },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
    { header: 'Updated Time', accessor: (row) => formatDatetime(row.updated_time as string) },
    {
      header: 'Actions',
      accessor: (row) =>
        row.status === 'Pending' ? (
          <button
            onClick={() => handleDelete(row.id as number)}
            disabled={deleteCommands.isPending}
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
        <h2 style={sectionHeading}>Property Commands</h2>
        <button
          onClick={() => setDialogOpen(true)}
          className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          style={{ background: 'var(--color-accent)' }}
        >
          Send Command
        </button>
      </div>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No commands"
        pagination={
          pagination
            ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
            : undefined
        }
        onPageChange={setPage}
      />
      {dialogOpen && (
        <CommandDialog
          productId={productId}
          deviceId={deviceId}
          onClose={() => setDialogOpen(false)}
          onSubmit={(command) => {
            createCommand.mutate(command, {
              onSuccess: () => {
                setDialogOpen(false)
              },
            })
          }}
          isSubmitting={createCommand.isPending}
        />
      )}
    </section>
  )
}

function CommandDialog({
  productId,
  deviceId,
  onClose,
  onSubmit,
  isSubmitting,
}: {
  productId: string
  deviceId: string
  onClose: () => void
  onSubmit: (command: { product_id: string; device_id: string; command: unknown }) => void
  isSubmitting: boolean
}) {
  const [jsonInput, setJsonInput] = useState('')
  const [parseError, setParseError] = useState<string | null>(null)

  const handleSubmit = () => {
    setParseError(null)
    try {
      const parsed = JSON.parse(jsonInput)
      onSubmit({ product_id: productId, device_id: deviceId, command: parsed })
    } catch {
      setParseError('Invalid JSON')
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-xl p-6 shadow-2xl"
        style={{ background: 'var(--color-surface-1)', border: '1px solid var(--color-border)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-[15px] font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          Send Command
        </h3>
        <textarea
          value={jsonInput}
          onChange={(e) => {
            setJsonInput(e.target.value)
            setParseError(null)
          }}
          placeholder='{"key": "value"}'
          rows={8}
          className="mt-4 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            border: '1px solid var(--color-border)',
            background: 'var(--color-surface-2)',
            color: 'var(--color-text-primary)',
          }}
        />
        {parseError && (
          <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
            {parseError}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors"
            style={{
              color: 'var(--color-text-secondary)',
              border: '1px solid var(--color-border)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--color-surface-2)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent'
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSubmitting || !jsonInput.trim()}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {isSubmitting ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}
