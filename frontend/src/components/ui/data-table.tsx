import { ChevronLeft, ChevronRight, Loader2 } from 'lucide-react'

export interface Column<T> {
  header: string
  accessor: keyof T | ((row: T) => React.ReactNode)
  className?: string
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  pagination?: {
    page: number
    pageSize: number
    total?: number
    hasMore?: boolean
  }
  onPageChange?: (page: number) => void
  loading?: boolean
  emptyMessage?: string
}

export function DataTable<T extends Record<string, unknown>>({
  columns,
  data,
  pagination,
  onPageChange,
  loading,
  emptyMessage = 'No data',
}: DataTableProps<T>) {
  const totalPages = pagination?.total
    ? Math.ceil(pagination.total / pagination.pageSize)
    : undefined
  const showPrev = pagination && pagination.page > 1
  const showNext = pagination && (totalPages ? pagination.page < totalPages : pagination.hasMore)
  const showPagination = pagination && (showPrev || showNext)

  return (
    <div
      className="overflow-x-auto rounded-xl border"
      style={{
        background: 'var(--color-surface-1)',
        borderColor: 'var(--color-border)',
      }}
    >
      <table className="w-full text-left text-[13px]">
        <thead>
          <tr
            style={{
              borderBottom: '1px solid var(--color-border)',
              background: 'var(--color-surface-2)',
            }}
          >
            {columns.map((col, i) => (
              <th
                key={i}
                className={`px-4 py-2.5 text-[11px] font-semibold uppercase tracking-wider ${col.className ?? ''}`}
                style={{ color: 'var(--color-text-muted)' }}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <tr>
              <td colSpan={columns.length} className="px-4 py-14 text-center">
                <Loader2
                  className="mx-auto h-5 w-5 animate-spin"
                  style={{ color: 'var(--color-accent)' }}
                />
              </td>
            </tr>
          ) : data.length === 0 ? (
            <tr>
              <td
                colSpan={columns.length}
                className="px-4 py-14 text-center text-[13px]"
                style={{ color: 'var(--color-text-muted)' }}
              >
                {emptyMessage}
              </td>
            </tr>
          ) : (
            data.map((row, rowIndex) => (
              <tr
                key={rowIndex}
                className="transition-colors duration-100"
                style={{
                  background: 'var(--color-surface-1)',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--color-accent-soft)'
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'var(--color-surface-1)'
                }}
              >
                {columns.map((col, colIndex) => (
                  <td
                    key={colIndex}
                    className={`px-4 py-3 ${col.className ?? ''}`}
                    style={{
                      color: 'var(--color-text-primary)',
                      borderBottom: '1px solid var(--color-border)',
                      fontFamily: colIndex === 0 ? "'JetBrains Mono', monospace" : undefined,
                      fontSize: colIndex === 0 ? '12px' : undefined,
                    }}
                  >
                    {typeof col.accessor === 'function'
                      ? col.accessor(row)
                      : (row[col.accessor] as React.ReactNode)}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
      {showPagination && (
        <div
          className="flex items-center justify-between px-4 py-3"
          style={{ borderTop: '1px solid var(--color-border)' }}
        >
          <span className="text-[12px]" style={{ color: 'var(--color-text-muted)' }}>
            Page {pagination!.page}
            {totalPages ? ` of ${totalPages}` : ''}
          </span>
          <div className="flex gap-1">
            <button
              onClick={() => onPageChange?.(pagination!.page - 1)}
              disabled={!showPrev}
              className="flex h-7 w-7 items-center justify-center rounded-md transition-colors duration-100 disabled:opacity-30"
              style={{ color: 'var(--color-text-secondary)' }}
              onMouseEnter={(e) => {
                if (!e.currentTarget.disabled)
                  e.currentTarget.style.background = 'var(--color-surface-2)'
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'transparent'
              }}
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            <button
              onClick={() => onPageChange?.(pagination!.page + 1)}
              disabled={!showNext}
              className="flex h-7 w-7 items-center justify-center rounded-md transition-colors duration-100 disabled:opacity-30"
              style={{ color: 'var(--color-text-secondary)' }}
              onMouseEnter={(e) => {
                if (!e.currentTarget.disabled)
                  e.currentTarget.style.background = 'var(--color-surface-2)'
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'transparent'
              }}
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
