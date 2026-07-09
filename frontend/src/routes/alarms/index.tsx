import { useState } from 'react'
import { createRoute } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useAlarms, useAckAlarm, useClearAlarm } from '@/hooks/useAlarms'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { Badge } from '@/components/ui/badge'
import { toast } from '@/components/ui/sonner'
import { extractErrorMessage, formatDatetime } from '@/lib/utils'

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
  status: string
  cleared_at?: string | null
  webhook_status?: string | null
  created_at: string
}

function AlarmsIndexPage() {
  const [productId, setProductId] = useState<string>('')
  const [deviceId, setDeviceId] = useState<string>('')
  const [level, setLevel] = useState<string>('')
  const [status, setStatus] = useState<string>('')
  const [page, setPage] = useState(1)

  const { data: products } = useProducts()
  const { data, isLoading } = useAlarms({
    product_id: productId || null,
    device_id: deviceId || null,
    level: level || null,
    status: status || null,
    page,
    page_size: 10,
  })
  const ackMutation = useAckAlarm()
  const clearMutation = useClearAlarm()

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])

  const items: AlarmRecordRow[] = (data?.data ?? []) as AlarmRecordRow[]
  const pagination = data?.pagination

  const levelVariantMap: Record<string, 'info' | 'warning' | 'danger'> = {
    info: 'info',
    warning: 'warning',
    critical: 'danger',
  }

  const statusVariantMap: Record<string, 'danger' | 'warning' | 'success'> = {
    active: 'danger',
    acknowledged: 'warning',
    cleared: 'success',
  }

  const actionButtonStyle: React.CSSProperties = {
    fontSize: '13px',
    color: 'var(--color-accent)',
    textDecoration: 'underline',
    background: 'none',
    border: 'none',
    cursor: 'pointer',
    padding: 0,
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
        <Badge variant={levelVariantMap[row.level] ?? 'default'}>{row.level}</Badge>
      ),
    },
    {
      header: 'Message',
      accessor: (row) => row.message ?? '-',
    },
    {
      header: 'Status',
      accessor: (row) => (
        <Badge
          data-testid={`alarm-status-tag-${row.id}`}
          variant={statusVariantMap[row.status] ?? 'default'}
          style={{ textTransform: 'capitalize' }}
        >
          {row.status}
        </Badge>
      ),
    },
    {
      header: 'Cleared At',
      accessor: (row) =>
        row.cleared_at ? formatDatetime(row.cleared_at) : row.status === 'cleared' ? 'N/A' : '-',
    },
    {
      header: 'Actions',
      accessor: (row) => {
        if (row.status === 'cleared') {
          return <span style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>-</span>
        }
        return (
          <div style={{ display: 'flex', gap: '8px' }}>
            {row.status === 'active' && (
              <button
                data-testid={`ack-alarm-button-${row.id}`}
                disabled={ackMutation.isPending}
                onClick={() => {
                  ackMutation.mutate(row.id, {
                    onError: (error) => {
                      toast.error('Failed to acknowledge alarm', { description: error.message })
                    },
                  })
                }}
                style={actionButtonStyle}
              >
                Acknowledge
              </button>
            )}
            <button
              data-testid={`clear-alarm-button-${row.id}`}
              disabled={clearMutation.isPending}
              onClick={() => {
                clearMutation.mutate(row.id, {
                  onError: (error) => {
                    const msg = extractErrorMessage(error)
                    if (msg.includes('already cleared')) {
                      toast.error('Alarm already cleared')
                    } else {
                      toast.error('Failed to clear alarm', { description: msg })
                    }
                  },
                })
              }}
              style={actionButtonStyle}
            >
              Clear
            </button>
          </div>
        )
      },
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
              name: 'status',
              label: 'Status',
              type: 'select',
              testId: 'status-filter-select',
              options: [
                { label: 'Active', value: 'active' },
                { label: 'Acknowledged', value: 'acknowledged' },
                { label: 'Cleared', value: 'cleared' },
              ],
            },
          ]}
          onSearch={(values) => {
            setPage(1)
            setProductId(values.product_id)
            setDeviceId(values.device_id)
            setLevel(values.level)
            setStatus(values.status)
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
