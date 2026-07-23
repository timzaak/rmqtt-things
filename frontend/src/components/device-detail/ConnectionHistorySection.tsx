import { useState } from 'react'
import { useDeviceStatusHistory } from '@/hooks/useDevices'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading } from './styles'

export function ConnectionHistorySection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useDeviceStatusHistory({
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
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        return (
          <span
            className="text-[12px] font-semibold"
            style={{
              color: status === 'Online' ? '#059669' : 'var(--color-text-muted)',
            }}
          >
            {status}
          </span>
        )
      },
    },
    { header: 'IP Address', accessor: (row) => (row.ip_address as string | null) ?? '-' },
    {
      header: 'Connected At',
      accessor: (row) =>
        (row.connected_at as string | null) ? formatDatetime(row.connected_at as string) : '-',
    },
    {
      header: 'Disconnected At',
      accessor: (row) =>
        (row.disconnected_at as string | null)
          ? formatDatetime(row.disconnected_at as string)
          : '-',
    },
    { header: 'Reason', accessor: (row) => (row.reason as string | null) ?? '-' },
  ]

  return (
    <section>
      <h2 style={sectionHeading}>Connection History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No connection history"
        pagination={
          pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
        }
        onPageChange={setPage}
      />
    </section>
  )
}
