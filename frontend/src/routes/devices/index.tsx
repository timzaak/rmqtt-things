import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useDevices, type DeviceRow } from '@/hooks/useDevices'
import { useProducts } from '@/hooks/useProducts'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import { Badge } from '@/components/ui/badge'
import type { DeviceConnectionStatus, RegistrationSource } from '@/lib/api-generated/types.gen'
import { formatDatetime } from '@/lib/utils'

export const devicesIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/devices',
  component: DevicesIndexPage,
})

export const Route = devicesIndexRoute

const columns: Column<DeviceRow>[] = [
  {
    header: 'Device ID',
    accessor: (row) => (
      <Link
        to="/devices/show/$id"
        params={{ id: row.device_id }}
        className="text-sm text-blue-600 hover:underline dark:text-blue-400"
      >
        {row.device_id}
      </Link>
    ),
  },
  { header: 'Product ID', accessor: 'product_id' },
  {
    header: 'Status',
    accessor: (row) => (
      <span className={row.status === 'Online' ? 'text-green-600 dark:text-green-400' : 'text-slate-400 dark:text-slate-500'}>
        {row.status}
      </span>
    ),
  },
  { header: 'IP Address', accessor: (row) => row.ip_address ?? '-' },
  { header: 'Last Online', accessor: (row) => row.last_online_at ? formatDatetime(row.last_online_at) : '-' },
  { header: 'Last Offline', accessor: (row) => row.last_offline_at ? formatDatetime(row.last_offline_at) : '-' },
  {
    header: 'Registration',
    accessor: (row) => (
      <Badge variant={row.registration_source === 'Auto' ? 'info' : 'default'}>
        {row.registration_source}
      </Badge>
    ),
  },
  {
    header: 'Actions',
    accessor: (row) => (
      <Link
        to="/devices/show/$id"
        params={{ id: row.device_id }}
        className="text-sm text-blue-600 hover:underline dark:text-blue-400"
      >
        View
      </Link>
    ),
  },
]

function DevicesIndexPage() {
  const [filters, setFilters] = useState({ product_id: '', status: '', registration_source: '' })
  const [page, setPage] = useState(1)

  const { data: products } = useProducts(null)
  const { data, isLoading } = useDevices({
    product_id: filters.product_id || null,
    status: (filters.status || null) as DeviceConnectionStatus | null,
    registration_source: (filters.registration_source || null) as RegistrationSource | null,
    page,
    page_size: 10,
  })

  const devices = data?.data ?? []
  const pagination = data?.pagination

  return (
    <div>
      <PageHeader title="Devices" />
      <SearchForm
        fields={[
          {
            name: 'product_id',
            label: 'Product',
            type: 'select',
            options: products?.data?.map((p) => ({ label: p.name, value: p.model_no })) ?? [],
          },
          {
            name: 'status',
            label: 'Status',
            type: 'select',
            options: [
              { label: 'Online', value: 'Online' },
              { label: 'Offline', value: 'Offline' },
            ],
          },
          {
            name: 'registration_source',
            label: 'Registration',
            type: 'select',
            options: [
              { label: 'Auto', value: 'Auto' },
              { label: 'Manual', value: 'Manual' },
            ],
          },
        ]}
        onSearch={(values) => {
          setFilters(values as typeof filters)
          setPage(1)
        }}
      />
      <DataTable
        columns={columns}
        data={devices}
        loading={isLoading}
        emptyMessage="No devices found"
        pagination={pagination ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total } : undefined}
        onPageChange={setPage}
      />
    </div>
  )
}
