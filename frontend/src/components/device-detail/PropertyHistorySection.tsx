import { useState } from 'react'
import { usePropertyHistory } from '@/hooks/useProperties'
import { DataTable, type Column } from '@/components/ui/data-table'
import { formatDatetime } from '@/lib/utils'
import { sectionHeading, controlBaseStyle } from './styles'
import { PropertyHistoryChartSection } from './PropertyHistoryChartSection'
import { presetRange, type ChartRange } from './property-chart-model'

type HistoryView = 'chart' | 'table'

const viewButtonStyle = (active: boolean): React.CSSProperties => ({
  ...controlBaseStyle,
  padding: '0 12px',
  background: active ? 'var(--color-surface-2, #e2e8f0)' : 'var(--color-surface-1)',
  cursor: 'pointer',
})

export function PropertyHistorySection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  // Chart-view conditions live at the container level so switching to the
  // table view and back never loses them; the table's own condition (page)
  // stays here as before.
  const [view, setView] = useState<HistoryView>('chart')
  const [selectedKeys, setSelectedKeys] = useState<string[]>([])
  const [range, setRange] = useState<ChartRange>(() => presetRange('24h'))
  const [page, setPage] = useState(1)

  // The chart is the default view, so the table's first page is only fetched
  // when the table is actually shown (React Query caches it across switches).
  const { data, isLoading } = usePropertyHistory(
    {
      product_id: productId,
      device_id: deviceId,
      page,
      page_size: 10,
    },
    { enabled: view === 'table' }
  )
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
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
    {
      header: 'Reported Time',
      accessor: (row) =>
        (row.reported_time as string | null) ? formatDatetime(row.reported_time as string) : '-',
    },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
  ]

  return (
    <section>
      <div className="flex items-center justify-between">
        <h2 style={sectionHeading}>Property History</h2>
        <div className="flex gap-2">
          <button
            data-testid="property-history-view-chart"
            onClick={() => setView('chart')}
            style={viewButtonStyle(view === 'chart')}
          >
            Chart
          </button>
          <button
            data-testid="property-history-view-table"
            onClick={() => setView('table')}
            style={viewButtonStyle(view === 'table')}
          >
            Table
          </button>
        </div>
      </div>
      {view === 'chart' ? (
        <PropertyHistoryChartSection
          productId={productId}
          deviceId={deviceId}
          selectedKeys={selectedKeys}
          onSelectedKeysChange={setSelectedKeys}
          range={range}
          onRangeChange={setRange}
          onSwitchToTable={() => setView('table')}
        />
      ) : (
        <DataTable
          columns={columns}
          data={items as unknown as Record<string, unknown>[]}
          loading={isLoading}
          emptyMessage="No property history"
          pagination={
            pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
          }
          onPageChange={setPage}
        />
      )}
    </section>
  )
}
