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
    <div className="overflow-x-auto rounded-lg border border-slate-200 dark:border-slate-800">
      <table className="w-full text-left text-sm">
        <thead className="border-b border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-900">
          <tr>
            {columns.map((col, i) => (
              <th
                key={i}
                className={`px-4 py-3 font-medium text-slate-600 dark:text-slate-400 ${col.className ?? ''}`}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
          {loading ? (
            <tr>
              <td colSpan={columns.length} className="px-4 py-12 text-center">
                <Loader2 className="mx-auto h-5 w-5 animate-spin text-slate-400" />
              </td>
            </tr>
          ) : data.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="px-4 py-12 text-center text-slate-500">
                {emptyMessage}
              </td>
            </tr>
          ) : (
            data.map((row, rowIndex) => (
              <tr key={rowIndex} className="bg-white dark:bg-slate-950">
                {columns.map((col, colIndex) => (
                  <td
                    key={colIndex}
                    className={`px-4 py-3 text-slate-700 dark:text-slate-300 ${col.className ?? ''}`}
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
        <div className="flex items-center justify-between border-t border-slate-200 px-4 py-3 dark:border-slate-800">
          <span className="text-sm text-slate-500">
            Page {pagination!.page}
            {totalPages ? ` of ${totalPages}` : ''}
          </span>
          <div className="flex gap-1">
            <button
              onClick={() => onPageChange?.(pagination!.page - 1)}
              disabled={!showPrev}
              className="rounded p-1 text-slate-500 hover:bg-slate-100 disabled:opacity-40 dark:hover:bg-slate-800"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            <button
              onClick={() => onPageChange?.(pagination!.page + 1)}
              disabled={!showNext}
              className="rounded p-1 text-slate-500 hover:bg-slate-100 disabled:opacity-40 dark:hover:bg-slate-800"
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
