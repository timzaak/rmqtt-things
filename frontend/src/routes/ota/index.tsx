import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { useOtaVersions, useDeleteOtaVersion } from '@/hooks/useOta'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { toast } from '@/components/ui/sonner'
import { formatVersion } from '@/lib/version'
import { formatDatetime } from '@/lib/utils'

export const otaIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ota',
  component: OtaIndexPage,
})

export const Route = otaIndexRoute

interface OtaVersionRow extends Record<string, unknown> {
  id: number
  product_id: string
  key: string
  version: number
  min_version: number
  max_version?: number | null
  file_key: string
  bin_length?: number | null
  bin_md5?: string | null
  status: number
  released_at: string
  created_at: string
  updated_at: string
  device_ids?: Array<string> | null
  log?: unknown
}

function OtaIndexPage() {
  const [productId, setProductId] = useState<string>('')
  const [page, setPage] = useState(1)
  const [deleteTarget, setDeleteTarget] = useState<OtaVersionRow | null>(null)

  const { data: products } = useProducts()
  const { data, isLoading } = useOtaVersions({
    product_id: productId || null,
    page,
    page_size: 10,
  })
  const deleteMutation = useDeleteOtaVersion()

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])

  const items: OtaVersionRow[] = (data?.data ?? []) as OtaVersionRow[]
  const pagination = data?.pagination

  const columns: Column<OtaVersionRow>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Product',
      accessor: (row) => productMap.get(row.product_id) ?? row.product_id,
    },
    { header: 'Key', accessor: 'key' },
    {
      header: 'Version',
      accessor: (row) => formatVersion(row.version),
    },
    {
      header: 'Min Version',
      accessor: (row) => formatVersion(row.min_version),
    },
    {
      header: 'Max Version',
      accessor: (row) => (row.max_version != null ? formatVersion(row.max_version) : '-'),
    },
    {
      header: 'Bin Length',
      accessor: (row) => row.bin_length ?? '-',
    },
    {
      header: 'Bin MD5',
      accessor: (row) => row.bin_md5 ?? '-',
    },
    {
      header: 'Created At',
      accessor: (row) => formatDatetime(row.created_at),
    },
    {
      header: 'Actions',
      accessor: (row) => (
        <div className="flex items-center gap-2">
          <Link
            to="/ota/show/$id"
            params={{ id: String(row.id) }}
            className="text-sm hover:underline"
            style={{ color: 'var(--color-accent)' }}
          >
            Show
          </Link>
          <Link
            to="/ota/edit/$id"
            params={{ id: String(row.id) }}
            className="text-sm hover:underline"
            style={{ color: 'var(--color-accent)' }}
          >
            Edit
          </Link>
          <button
            onClick={() => setDeleteTarget(row)}
            className="text-sm hover:underline"
            style={{ color: '#dc2626' }}
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
        toast.error('Failed to delete OTA version', { description: error.message })
        setDeleteTarget(null)
      },
    })
  }

  return (
    <div>
      <PageHeader
        title="OTA Versions"
        actions={
          <Link
            to="/ota/create"
            className="inline-flex h-9 items-center gap-1.5 rounded-md px-4 text-sm font-medium"
            style={{ background: 'var(--color-accent)', color: '#fff' }}
          >
            <Plus className="h-4 w-4" />
            Create OTA Version
          </Link>
        }
      />
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
      <DataTable
        columns={columns}
        data={items}
        loading={isLoading}
        emptyMessage="No OTA versions found"
        pagination={
          pagination
            ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
            : undefined
        }
        onPageChange={setPage}
      />
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title="Delete OTA Version"
        description={`Are you sure you want to delete OTA version "${deleteTarget?.key}" (v${deleteTarget != null ? formatVersion(deleteTarget.version) : ''})?`}
        onConfirm={handleDelete}
        confirmText="Delete"
        variant="danger"
      />
    </div>
  )
}
