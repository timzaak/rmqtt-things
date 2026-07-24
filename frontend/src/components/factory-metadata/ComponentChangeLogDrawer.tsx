import { useEffect, useState } from 'react'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { useComponentChangeLog } from '@/hooks/useFactoryMetadata'
import { extractErrorMessage, formatDatetime } from '@/lib/utils'
import type { FactoryMetadataChangeLog } from '@/lib/api-generated/types.gen'

interface ComponentChangeLogDrawerProps {
  sn: string | null
  onClose: () => void
}

interface ChangeLogRow {
  id: number
  actor: string
  createdAt: string
  before: unknown
  after: unknown
  [key: string]: unknown
}

function toRow(entry: FactoryMetadataChangeLog): ChangeLogRow {
  return {
    id: entry.id,
    actor: entry.actor,
    createdAt: entry.created_at,
    before: entry.before,
    after: entry.after,
  }
}

/**
 * Right-side drawer showing a single component's metadata change log
 * (design §4.4.3). Renders the most recent changes first (backend already
 * returns DESC by `created_at`) with paginated `before`/`after` JSONB snapshots.
 *
 * `before` is null on the initial report — rendered as "Initial report".
 */
export function ComponentChangeLogDrawer({ sn, onClose }: ComponentChangeLogDrawerProps) {
  const [page, setPage] = useState(1)
  const { data, isLoading, isError, error } = useComponentChangeLog(sn ?? '', page)

  // Page resets to 1 on component change via the parent's `key={componentSn}`
  // (remount), so no reset effect is needed here.

  // Surface non-graceful errors. (There is no special-case 404 here — the
  // change-log endpoint returns an empty page rather than 404.)
  useEffect(() => {
    if (isError) {
      toast.error('Failed to load change log', {
        description: extractErrorMessage(error),
      })
    }
  }, [isError, error])

  const isOpen = sn !== null

  const rows = (data?.data ?? []).map(toRow)
  const pagination = data?.pagination

  const columns: Column<ChangeLogRow>[] = [
    { header: 'ID', accessor: 'id' },
    { header: 'Actor', accessor: 'actor' },
    {
      header: 'Time',
      accessor: (row) => formatDatetime(row.createdAt),
    },
    {
      header: 'Before',
      accessor: (row) =>
        row.before === null || row.before === undefined ? (
          <span style={{ color: 'var(--color-text-muted)', fontStyle: 'italic' }}>
            Initial report
          </span>
        ) : (
          <pre
            className="max-w-md overflow-auto text-[11px]"
            style={{ fontFamily: "'JetBrains Mono', monospace" }}
          >
            {JSON.stringify(row.before, null, 2)}
          </pre>
        ),
    },
    {
      header: 'After',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.after, null, 2)}
        </pre>
      ),
    },
  ]

  if (!isOpen) return null

  return (
    <div
      className="fixed inset-0 z-50 flex justify-end"
      style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
      onClick={onClose}
      data-testid="component-change-log-drawer"
    >
      <div
        className="h-full w-full max-w-2xl overflow-auto p-6 shadow-2xl"
        style={{
          background: 'var(--color-surface-1)',
          borderLeft: '1px solid var(--color-border)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3
              className="text-[15px] font-semibold"
              style={{ color: 'var(--color-text-primary)' }}
            >
              Component Change Log
            </h3>
            <p className="text-[12px]" style={{ color: 'var(--color-text-muted)' }}>
              <code style={{ fontFamily: "'JetBrains Mono', monospace" }}>{sn}</code>
            </p>
          </div>
          <button
            data-testid="component-change-log-close"
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
            Close
          </button>
        </div>

        <DataTable
          columns={columns}
          data={rows}
          loading={isLoading}
          emptyMessage="No change history"
          pagination={
            pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
          }
          onPageChange={setPage}
        />
      </div>
    </div>
  )
}
