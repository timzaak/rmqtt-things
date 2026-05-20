import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useDevices, useDeviceStatusHistory, type DeviceRow } from '@/hooks/useDevices'
import { usePropertyLatest, usePropertyHistory, usePropertyCommands, useCreatePropertyCommand, useDeletePropertyCommands } from '@/hooks/useProperties'
import { useEventHistory } from '@/hooks/useEvents'
import { DataTable, type Column } from '@/components/ui/data-table'
import { PageHeader } from '@/components/ui/page-header'
import { formatDatetime } from '@/lib/utils'

export const devicesShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/devices/show/$id',
  component: DevicesShowPage,
})

export const Route = devicesShowRoute

function DevicesShowPage() {
  const { id } = devicesShowRoute.useParams()

  const { data: deviceData, isLoading: deviceLoading } = useDevices({
    product_id: null,
    device_id: id,
    page: 1,
    page_size: 1,
  })

  const device = deviceData?.data?.[0]

  if (deviceLoading) {
    return (
      <div>
        <PageHeader title="Device Detail" />
        <p className="text-slate-500">Loading...</p>
      </div>
    )
  }

  if (!device) {
    return (
      <div>
        <PageHeader title="Device Detail" />
        <p className="text-slate-500">Device not found.</p>
      </div>
    )
  }

  return <DeviceDetailContent id={id} productId={device.product_id} device={device} />
}

function DeviceDetailContent({ id, productId, device }: { id: string; productId: string; device: DeviceRow }) {
  return (
    <div className="space-y-8">
      <PageHeader title="Device Detail" />
      <Link to="/devices" className="text-sm text-blue-600 hover:underline dark:text-blue-400">
        &larr; Back to Devices
      </Link>

      <section>
        <h2 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Device Info</h2>
        <div className="grid grid-cols-2 gap-4 rounded-lg border border-slate-200 p-4 dark:border-slate-800 sm:grid-cols-3 lg:grid-cols-6">
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Device ID</p>
            <p className="text-sm font-medium text-slate-900 dark:text-slate-100">{device.device_id}</p>
          </div>
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Product ID</p>
            <p className="text-sm font-medium text-slate-900 dark:text-slate-100">{device.product_id}</p>
          </div>
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Status</p>
            <p className={`text-sm font-medium ${device.status === 'Online' ? 'text-green-600 dark:text-green-400' : 'text-slate-400 dark:text-slate-500'}`}>
              {device.status}
            </p>
          </div>
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">IP Address</p>
            <p className="text-sm font-medium text-slate-900 dark:text-slate-100">{device.ip_address ?? '-'}</p>
          </div>
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Last Online</p>
            <p className="text-sm font-medium text-slate-900 dark:text-slate-100">{device.last_online_at ? formatDatetime(device.last_online_at) : '-'}</p>
          </div>
          <div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Last Offline</p>
            <p className="text-sm font-medium text-slate-900 dark:text-slate-100">{device.last_offline_at ? formatDatetime(device.last_offline_at) : '-'}</p>
          </div>
        </div>
      </section>

      <LatestPropertiesSection productId={productId} deviceId={id} />
      <PropertyHistorySection productId={productId} deviceId={id} />
      <EventHistorySection productId={productId} deviceId={id} />
      <CommandHistorySection productId={productId} deviceId={id} />
      <ConnectionHistorySection productId={productId} deviceId={id} />
    </div>
  )
}

function LatestPropertiesSection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const { data, isLoading } = usePropertyLatest({ product_id: productId, device_id: deviceId })
  const items = data?.data ?? []

  const columns: Column<Record<string, unknown>>[] = [
    {
      header: 'Properties',
      accessor: (row) => <pre className="max-w-md overflow-auto text-xs">{JSON.stringify(row.properties, null, 2)}</pre>,
    },
    { header: 'Updated Time', accessor: (row) => formatDatetime(row.updated_time as string) },
  ]

  return (
    <section>
      <h2 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Latest Properties</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No latest properties"
      />
    </section>
  )
}

function PropertyHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = usePropertyHistory({ product_id: productId, device_id: deviceId, page, page_size: 10 })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Properties',
      accessor: (row) => <pre className="max-w-md overflow-auto text-xs">{JSON.stringify(row.properties, null, 2)}</pre>,
    },
    { header: 'Reported Time', accessor: (row) => (row.reported_time as string | null) ? formatDatetime(row.reported_time as string) : '-' },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
  ]

  return (
    <section>
      <h2 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Property History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No property history"
        pagination={pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined}
        onPageChange={setPage}
      />
    </section>
  )
}

function EventHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useEventHistory({ product_id: productId, device_id: deviceId, page, page_size: 10 })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Events',
      accessor: (row) => <pre className="max-w-md overflow-auto text-xs">{JSON.stringify(row.events, null, 2)}</pre>,
    },
    { header: 'Reported Time', accessor: (row) => (row.reported_time as string | null) ? formatDatetime(row.reported_time as string) : '-' },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
  ]

  return (
    <section>
      <h2 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Event History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No event history"
        pagination={pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined}
        onPageChange={setPage}
      />
    </section>
  )
}

function CommandHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const [dialogOpen, setDialogOpen] = useState(false)

  const { data, isLoading } = usePropertyCommands({ product_id: productId, device_id: deviceId, page, page_size: 10 })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const createCommand = useCreatePropertyCommand()
  const deleteCommands = useDeletePropertyCommands()

  const handleDelete = (commandId: number) => {
    deleteCommands.mutate([commandId])
  }

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Command',
      accessor: (row) => <pre className="max-w-md overflow-auto text-xs">{JSON.stringify(row.command, null, 2)}</pre>,
    },
    {
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        const colorMap: Record<string, string> = {
          Pending: 'text-yellow-600 dark:text-yellow-400',
          Sent: 'text-blue-600 dark:text-blue-400',
          Success: 'text-green-600 dark:text-green-400',
          Failed: 'text-red-600 dark:text-red-400',
          Deleted: 'text-slate-400 dark:text-slate-500',
        }
        return <span className={colorMap[status] ?? ''}>{status}</span>
      },
    },
    { header: 'Created Time', accessor: (row) => formatDatetime(row.created_time as string) },
    { header: 'Updated Time', accessor: (row) => formatDatetime(row.updated_time as string) },
    {
      header: 'Actions',
      accessor: (row) =>
        row.status === 'Pending' ? (
          <button
            onClick={() => handleDelete(row.id as number)}
            disabled={deleteCommands.isPending}
            className="text-sm text-red-600 hover:underline disabled:opacity-50 dark:text-red-400"
          >
            Delete
          </button>
        ) : null,
    },
  ]

  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">Property Commands</h2>
        <button
          onClick={() => setDialogOpen(true)}
          className="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700 dark:bg-blue-500 dark:hover:bg-blue-600"
        >
          Send Command
        </button>
      </div>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No commands"
        pagination={pagination ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total } : undefined}
        onPageChange={setPage}
      />
      {dialogOpen && (
        <CommandDialog
          productId={productId}
          deviceId={deviceId}
          onClose={() => setDialogOpen(false)}
          onSubmit={(command) => {
            createCommand.mutate(command, {
              onSuccess: () => {
                setDialogOpen(false)
              },
            })
          }}
          isSubmitting={createCommand.isPending}
        />
      )}
    </section>
  )
}

function CommandDialog({
  productId,
  deviceId,
  onClose,
  onSubmit,
  isSubmitting,
}: {
  productId: string
  deviceId: string
  onClose: () => void
  onSubmit: (command: { product_id: string; device_id: string; command: unknown }) => void
  isSubmitting: boolean
}) {
  const [jsonInput, setJsonInput] = useState('')
  const [parseError, setParseError] = useState<string | null>(null)

  const handleSubmit = () => {
    setParseError(null)
    try {
      const parsed = JSON.parse(jsonInput)
      onSubmit({ product_id: productId, device_id: deviceId, command: parsed })
    } catch {
      setParseError('Invalid JSON')
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-lg rounded-lg bg-white p-6 shadow-xl dark:bg-slate-900" onClick={(e) => e.stopPropagation()}>
        <h3 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Send Command</h3>
        <textarea
          value={jsonInput}
          onChange={(e) => { setJsonInput(e.target.value); setParseError(null) }}
          placeholder='{"key": "value"}'
          rows={8}
          className="w-full rounded-md border border-slate-300 bg-white px-3 py-2 font-mono text-sm text-slate-900 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100"
        />
        {parseError && <p className="mt-1 text-sm text-red-600">{parseError}</p>}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-md border border-slate-300 px-3 py-1.5 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSubmitting || !jsonInput.trim()}
            className="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 dark:bg-blue-500 dark:hover:bg-blue-600"
          >
            {isSubmitting ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}

function ConnectionHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useDeviceStatusHistory({ product_id: productId, device_id: deviceId, page, page_size: 10 })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        return (
          <span className={status === 'Online' ? 'text-green-600 dark:text-green-400' : 'text-slate-400 dark:text-slate-500'}>
            {status}
          </span>
        )
      },
    },
    { header: 'IP Address', accessor: (row) => (row.ip_address as string | null) ?? '-' },
    { header: 'Connected At', accessor: (row) => (row.connected_at as string | null) ? formatDatetime(row.connected_at as string) : '-' },
    { header: 'Disconnected At', accessor: (row) => (row.disconnected_at as string | null) ? formatDatetime(row.disconnected_at as string) : '-' },
    { header: 'Reason', accessor: (row) => (row.reason as string | null) ?? '-' },
  ]

  return (
    <section>
      <h2 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">Connection History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No connection history"
        pagination={pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined}
        onPageChange={setPage}
      />
    </section>
  )
}
