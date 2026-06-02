import { useState } from 'react'
import { createRoute } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useAlarms, useAckAlarm } from '@/hooks/useAlarms'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { toast } from '@/components/ui/sonner'
import { formatDatetime } from '@/lib/utils'

export const alarmsIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/alarms',
  component: AlarmsIndexPage,
})

export const Route = alarmsIndexRoute

interface AlarmRecordRow extends Record<string, unknown> {
  id: number
  rule_id: number
  rule_name: string
  product_id: string
  device_id: string
  level: string
  message?: string | null
  acknowledged: boolean
  webhook_status?: string | null
  created_at: string
}

function AlarmsIndexPage() {
  const [productId, setProductId] = useState<string>('')
  const [deviceId, setDeviceId] = useState<string>('')
  const [level, setLevel] = useState<string>('')
  const [acknowledged, setAcknowledged] = useState<string>('')
  const [page, setPage] = useState(1)

  const { data: products } = useProducts()
  const { data, isLoading } = useAlarms({
    product_id: productId || null,
    device_id: deviceId || null,
    level: level || null,
    acknowledged: acknowledged === 'true' ? true : acknowledged === 'false' ? false : null,
    page,
    page_size: 10,
  })
  const ackMutation = useAckAlarm()

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])

  const items: AlarmRecordRow[] = (data?.data ?? []) as AlarmRecordRow[]
  const pagination = data?.pagination

  const levelStyleMap: Record<string, React.CSSProperties> = {
    info: { background: '#dbeafe', color: '#1e40af' },
    warning: { background: '#fef3c7', color: '#92400e' },
    critical: { background: '#fee2e2', color: '#991b1b' },
  }

  const columns: Column<AlarmRecordRow>[] = [
    {
      header: 'Created At',
      accessor: (row) => formatDatetime(row.created_at),
    },
    { header: 'Rule Name', accessor: 'rule_name' },
    {
      header: 'Product',
      accessor: (row) => productMap.get(row.product_id) ?? row.product_id,
    },
    { header: 'Device', accessor: 'device_id' },
    {
      header: 'Level',
      accessor: (row) => (
        <span
          style={{
            display: 'inline-flex',
            borderRadius: '4px',
            padding: '2px 8px',
            fontSize: '12px',
            fontWeight: 500,
            ...(levelStyleMap[row.level] ?? {
              background: 'var(--color-surface-2)',
              color: 'var(--color-text-secondary)',
            }),
          }}
        >
          {row.level}
        </span>
      ),
    },
    {
      header: 'Message',
      accessor: (row) => row.message ?? '-',
    },
    {
      header: 'Acknowledged',
      accessor: (row) => (
        <span
          data-testid={`alarm-acknowledged-tag-${row.id}`}
          style={{
            display: 'inline-flex',
            borderRadius: '4px',
            padding: '2px 8px',
            fontSize: '12px',
            fontWeight: 500,
            ...(row.acknowledged
              ? { background: '#dcfce7', color: '#166534' }
              : { background: 'var(--color-surface-2)', color: 'var(--color-text-secondary)' }),
          }}
        >
          {row.acknowledged ? 'Yes' : 'No'}
        </span>
      ),
    },
    {
      header: 'Actions',
      accessor: (row) =>
        row.acknowledged ? (
          <span style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>Acknowledged</span>
        ) : (
          <button
            data-testid={`ack-alarm-button-${row.id}`}
            onClick={() => {
              ackMutation.mutate(row.id, {
                onError: (error) => {
                  toast.error('Failed to acknowledge alarm', { description: error.message })
                },
              })
            }}
            style={{
              fontSize: '13px',
              color: 'var(--color-accent)',
              textDecoration: 'underline',
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              padding: 0,
            }}
          >
            Acknowledge
          </button>
        ),
    },
  ]

  return (
    <div>
      <PageHeader title="Alarm Records" />
      <div data-testid="alarm-search-form">
        <SearchForm
          fields={[
            {
              name: 'product_id',
              label: 'Product',
              type: 'select',
              options: products?.data?.map((p) => ({ label: p.name, value: p.model_no })) ?? [],
            },
            {
              name: 'device_id',
              label: 'Device',
              type: 'text',
              placeholder: 'Device ID',
            },
            {
              name: 'level',
              label: 'Level',
              type: 'select',
              options: [
                { label: 'Info', value: 'info' },
                { label: 'Warning', value: 'warning' },
                { label: 'Critical', value: 'critical' },
              ],
            },
            {
              name: 'acknowledged',
              label: 'Acknowledged',
              type: 'select',
              options: [
                { label: 'Acknowledged', value: 'true' },
                { label: 'Unacknowledged', value: 'false' },
              ],
            },
          ]}
          onSearch={(values) => {
            setPage(1)
            setProductId(values.product_id)
            setDeviceId(values.device_id)
            setLevel(values.level)
            setAcknowledged(values.acknowledged)
          }}
        />
      </div>
      <div data-testid="alarm-table">
        <DataTable
          columns={columns}
          data={items}
          loading={isLoading}
          emptyMessage="No alarm records found"
          pagination={
            pagination
              ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
              : undefined
          }
          onPageChange={setPage}
        />
      </div>
    </div>
  )
}
