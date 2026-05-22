import { useState } from 'react'
import { createRoute } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useAlarmRules, useDeleteAlarmRule, useUpdateAlarmRuleStatus } from '@/hooks/useAlarmRules'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { toast } from '@/components/ui/sonner'
import { formatDatetime } from '@/lib/utils'

export const alarmRulesIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/alarm-rules',
  component: AlarmRulesIndexPage,
})

export const Route = alarmRulesIndexRoute

interface AlarmRuleRow extends Record<string, unknown> {
  id: number
  product_id: string
  name: string
  trigger_type: string
  enabled: boolean
  throttle_minutes: number
  created_at: string
  updated_at: string
}

function AlarmRulesIndexPage() {
  const [productId, setProductId] = useState<string>('')
  const [page, setPage] = useState(1)
  const [deleteTarget, setDeleteTarget] = useState<AlarmRuleRow | null>(null)

  const { data: products } = useProducts()
  const { data, isLoading } = useAlarmRules({
    product_id: productId || null,
    page,
    page_size: 10,
  })
  const deleteMutation = useDeleteAlarmRule()
  const statusMutation = useUpdateAlarmRuleStatus()

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])

  const items: AlarmRuleRow[] = (data?.data ?? []) as AlarmRuleRow[]
  const pagination = data?.pagination

  const columns: Column<AlarmRuleRow>[] = [
    { header: 'ID', accessor: 'id' },
    { header: 'Name', accessor: 'name' },
    {
      header: 'Product',
      accessor: (row) => productMap.get(row.product_id) ?? row.product_id,
    },
    { header: 'Trigger Type', accessor: 'trigger_type' },
    {
      header: 'Enabled',
      accessor: (row) => (
        <input
          type="checkbox"
          checked={row.enabled}
          data-testid={`alarm-rule-enabled-switch-${row.id}`}
          onChange={() => {
            statusMutation.mutate(
              { id: row.id, enabled: !row.enabled },
              {
                onError: (error) => {
                  toast.error('Failed to update alarm rule status', { description: error.message })
                },
              }
            )
          }}
          className="h-4 w-4 rounded border-slate-300"
        />
      ),
    },
    { header: 'Throttle (min)', accessor: 'throttle_minutes' },
    {
      header: 'Created At',
      accessor: (row) => formatDatetime(row.created_at),
    },
    {
      header: 'Actions',
      accessor: (row) => (
        <div className="flex items-center gap-2">
          <a
            href={`/alarm-rules/edit/${row.id}`}
            className="text-sm text-blue-600 hover:underline dark:text-blue-400"
          >
            Edit
          </a>
          <button
            onClick={() => setDeleteTarget(row)}
            className="text-sm text-red-600 hover:underline dark:text-red-400"
          >
            Delete
          </button>
        </div>
      ),
    },
  ]

  function handleDelete() {
    if (!deleteTarget) return
    deleteMutation.mutate(deleteTarget.id, {
      onSuccess: () => {
        setDeleteTarget(null)
      },
      onError: (error) => {
        toast.error('Failed to delete alarm rule', { description: error.message })
        setDeleteTarget(null)
      },
    })
  }

  return (
    <div>
      <PageHeader
        title="Alarm Rules"
        actions={
          <a
            href="/alarm-rules/create"
            data-testid="alarm-rule-create-button"
            className="inline-flex h-9 items-center gap-1.5 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            <Plus className="h-4 w-4" />
            Create Alarm Rule
          </a>
        }
      />
      <div data-testid="alarm-rule-search-form">
        <SearchForm
          fields={[
            {
              name: 'product_id',
              label: 'Product',
              type: 'select',
              options: products?.data?.map((p) => ({ label: p.name, value: p.model_no })) ?? [],
            },
          ]}
          onSearch={(values) => {
            setPage(1)
            setProductId(values.product_id)
          }}
        />
      </div>
      <div data-testid="alarm-rule-table">
        <DataTable
          columns={columns}
          data={items}
          loading={isLoading}
          emptyMessage="No alarm rules found"
          pagination={
            pagination
              ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
              : undefined
          }
          onPageChange={setPage}
        />
      </div>
      <div data-testid="delete-confirm-dialog">
        <ConfirmDialog
          open={!!deleteTarget}
          onOpenChange={(open) => {
            if (!open) setDeleteTarget(null)
          }}
          title="Delete Alarm Rule"
          description={`Are you sure you want to delete alarm rule "${deleteTarget?.name}"?`}
          onConfirm={handleDelete}
          confirmText="Delete"
          variant="danger"
        />
      </div>
    </div>
  )
}
