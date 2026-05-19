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

  const levelColorMap: Record<string, string> = {
    info: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300',
    warning: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300',
    critical: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300',
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
          className={`inline-flex rounded px-2 py-0.5 text-xs font-medium ${levelColorMap[row.level] ?? 'bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-slate-300'}`}
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
          className={`inline-flex rounded px-2 py-0.5 text-xs font-medium ${
            row.acknowledged
              ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300'
              : 'bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-slate-300'
          }`}
        >
          {row.acknowledged ? 'Yes' : 'No'}
        </span>
      ),
    },
    {
      header: 'Actions',
      accessor: (row) =>
        row.acknowledged ? (
          <span className="text-sm text-slate-400">Acknowledged</span>
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
            className="text-sm text-blue-600 hover:underline dark:text-blue-400"
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
