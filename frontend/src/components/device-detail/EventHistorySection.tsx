import { useState } from 'react'
import { useEventHistory } from '@/hooks/useEvents'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading } from './styles'

export function EventHistorySection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useEventHistory({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Events',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.events, null, 2)}
        </pre>
      ),
    },
    {
      header: 'Reported Time',
      accessor: (row) =>
        (row.reported_time as string | null) ? formatDatetime(row.reported_time as string) : '-',
    },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
  ]

  return (
    <section>
      <h2 style={sectionHeading}>Event History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No event history"
        pagination={
          pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
        }
        onPageChange={setPage}
      />
    </section>
  )
}
