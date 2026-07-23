import { usePropertyLatest } from '@/hooks/useProperties'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading } from './styles'

export function LatestPropertiesSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const { data, isLoading } = usePropertyLatest({ product_id: productId, device_id: deviceId })
  const items = data?.data ?? []

  const columns: Column<Record<string, unknown>>[] = [
    {
      header: 'Properties',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.properties, null, 2)}
        </pre>
      ),
    },
    { header: 'Updated Time', accessor: (row) => formatDatetime(row.updated_time as string) },
  ]

  return (
    <section>
      <h2 style={sectionHeading}>Latest Properties</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No latest properties"
      />
    </section>
  )
}
