import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { Plus, Download } from 'lucide-react'
import { rootRoute } from '../__root'
import { useCerts, useUpdateCertStatus, useCaCert } from '@/hooks/useCerts'
import { useProducts } from '@/hooks/useProducts'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import type { CertIssue, CertStatus } from '@/lib/api-generated/types.gen'
import { formatDatetime } from '@/lib/utils'

export const certsIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/certs',
  component: CertsIndexPage,
})

export const Route = certsIndexRoute

const statusLabel: Record<CertStatus, string> = {
  Normal: 'Active',
  InValid: 'Invalid',
  Revoked: 'Revoked',
}

const statusBadgeClass: Record<CertStatus, string> = {
  Normal: 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300',
  InValid: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
  Revoked: 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300',
}

function StatusActions({ row }: { row: CertIssue }) {
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [pendingAction, setPendingAction] = useState<'revoke' | 'invalidate' | null>(null)
  const updateStatus = useUpdateCertStatus()

  function handleConfirm() {
    if (!pendingAction) return
    const status = pendingAction === 'revoke' ? 2 : 1
    updateStatus.mutate({
      product_id: row.product_id,
      device_id: row.device_id,
      status,
    })
  }

  function openConfirm(action: 'revoke' | 'invalidate') {
    setPendingAction(action)
    setConfirmOpen(true)
  }

  return (
    <>
      <div className="flex gap-2">
        <Link
          to="/certs/show/$id"
          params={{ id: String(row.id) }}
          className="text-sm text-blue-600 hover:underline dark:text-blue-400"
        >
          Show
        </Link>
        {row.status === 'Normal' && (
          <>
          <button
            onClick={() => openConfirm('revoke')}
            className="text-sm text-orange-600 hover:underline dark:text-orange-400"
          >
            Revoke
          </button>
          <button
            onClick={() => openConfirm('invalidate')}
            className="text-sm text-red-600 hover:underline dark:text-red-400"
          >
            Invalidate
          </button>
          </>
        )}
      </div>
      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title={pendingAction === 'revoke' ? 'Revoke Certificate' : 'Invalidate Certificate'}
        description={`Are you sure you want to ${pendingAction} the certificate for device ${row.device_id}?`}
        onConfirm={handleConfirm}
        confirmText={pendingAction === 'revoke' ? 'Revoke' : 'Invalidate'}
        variant="danger"
      />
    </>
  )
}

const columns: Column<CertIssue>[] = [
  { header: 'ID', accessor: 'id' },
  { header: 'Product', accessor: 'product_id' },
  { header: 'Device', accessor: 'device_id' },
  { header: 'Creation Time', accessor: (row) => formatDatetime(row.created_at) },
  { header: 'End Time', accessor: (row) => formatDatetime(row.end_at) },
  {
    header: 'Status',
    accessor: (row) => (
      <span
        className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${statusBadgeClass[row.status] ?? ''}`}
      >
        {statusLabel[row.status] ?? row.status}
      </span>
    ),
  },
  {
    header: 'Actions',
    accessor: (row) => <StatusActions row={row} />,
  },
]

function CertsIndexPage() {
  const [searchParams, setSearchParams] = useState<{
    product_id: string
    device_id: string
  }>({ product_id: '', device_id: '' })
  const [page, setPage] = useState(1)

  const { data: products } = useProducts()
  const { data: caCertData } = useCaCert()
  const { data: certPage, isLoading } = useCerts({
    product_id: searchParams.product_id || null,
    device_id: searchParams.device_id || null,
    page,
    page_size: 10,
  })
  const certData = certPage?.data ?? []
  const certPagination = certPage?.pagination
    ? {
        page: certPage.pagination.page,
        pageSize: certPage.pagination.page_size,
        hasMore: certData.length === certPage.pagination.page_size,
      }
    : undefined

  const productOptions = (products?.data ?? []).map((p) => ({
    label: p.name,
    value: p.model_no,
  }))

  return (
    <div>
      <PageHeader
        title="Certificates"
        actions={
          <div className="flex gap-2">
            <button
              type="button"
              disabled={!caCertData?.ca_pem}
              onClick={() => {
                if (!caCertData?.ca_pem) return
                const blob = new Blob([caCertData.ca_pem], { type: 'application/x-pem-file' })
                const url = URL.createObjectURL(blob)
                const a = document.createElement('a')
                a.href = url
                a.download = 'ca.pem'
                a.click()
                URL.revokeObjectURL(url)
              }}
              className="inline-flex h-9 items-center gap-1.5 rounded-md border border-slate-300 px-4 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
            >
              <Download className="h-4 w-4" />
              Download CA Certificate
            </button>
            <Link
              to="/certs/create"
              className="inline-flex h-9 items-center gap-1.5 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
            >
              <Plus className="h-4 w-4" />
              Issue Certificate
            </Link>
          </div>
        }
      />
      <SearchForm
        fields={[
          { name: 'product_id', label: 'Product', type: 'select', options: productOptions },
          { name: 'device_id', label: 'Device ID', placeholder: 'Search by Device ID' },
        ]}
        onSearch={(values) => {
          setPage(1)
          setSearchParams({ product_id: values.product_id, device_id: values.device_id })
        }}
      />
      <DataTable
        columns={columns}
        data={certData}
        loading={isLoading}
        emptyMessage="No certificates found"
        pagination={certPagination}
        onPageChange={setPage}
      />
    </div>
  )
}
