import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useDevices, useDeviceStatusHistory, type DeviceRow } from '@/hooks/useDevices'
import {
  usePropertyLatest,
  usePropertyHistory,
  usePropertyCommands,
  useCreatePropertyCommand,
  useDeletePropertyCommands,
} from '@/hooks/useProperties'
import { useEventHistory } from '@/hooks/useEvents'
import { DataTable, type Column } from '@/components/ui/data-table'
import { PageHeader } from '@/components/ui/page-header'
import { PropertyShadowSection } from '@/components/property-shadow/PropertyShadowSection'
import { FactoryMetadataSection } from '@/components/factory-metadata/FactoryMetadataSection'
import { formatDatetime } from '@/lib/utils'

export const devicesShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/devices/show/$id',
  component: DevicesShowPage,
})

export const Route = devicesShowRoute

const sectionHeading: React.CSSProperties = {
  color: 'var(--color-text-primary)',
  fontSize: '15px',
  fontWeight: 600,
  marginBottom: '16px',
}

const labelStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '11px',
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.05em',
}

const valueStyle: React.CSSProperties = {
  color: 'var(--color-text-primary)',
  fontSize: '13px',
  fontWeight: 500,
  fontFamily: "'JetBrains Mono', monospace",
}

const cardStyle: React.CSSProperties = {
  background: 'var(--color-surface-1)',
  border: '1px solid var(--color-border)',
  borderRadius: '12px',
  padding: '16px',
}

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
        <p style={{ color: 'var(--color-text-muted)', fontSize: '13px' }}>Loading...</p>
      </div>
    )
  }

  if (!device) {
    return (
      <div>
        <PageHeader title="Device Detail" />
        <p style={{ color: 'var(--color-text-muted)', fontSize: '13px' }}>Device not found.</p>
      </div>
    )
  }

  return <DeviceDetailContent id={id} productId={device.product_id} device={device} />
}

function DeviceDetailContent({
  id,
  productId,
  device,
}: {
  id: string
  productId: string
  device: DeviceRow
}) {
  return (
    <div className="space-y-8">
      <PageHeader title="Device Detail" />
      <Link
        to="/devices"
        className="text-[13px] font-medium hover:underline transition-opacity hover:opacity-80"
        style={{ color: 'var(--color-accent)' }}
      >
        &larr; Back to Devices
      </Link>

      <section>
        <h2 style={sectionHeading}>Device Info</h2>
        <div
          className="grid grid-cols-2 gap-4 rounded-xl sm:grid-cols-3 lg:grid-cols-6"
          style={cardStyle}
        >
          <div>
            <p style={labelStyle}>Device ID</p>
            <p style={valueStyle}>{device.device_id}</p>
          </div>
          <div>
            <p style={labelStyle}>Product ID</p>
            <p style={valueStyle}>{device.product_id}</p>
          </div>
          <div>
            <p style={labelStyle}>Status</p>
            <p
              style={{
                ...valueStyle,
                color: device.status === 'Online' ? '#059669' : 'var(--color-text-muted)',
              }}
            >
              {device.status}
            </p>
          </div>
          <div>
            <p style={labelStyle}>IP Address</p>
            <p style={valueStyle}>{device.ip_address ?? '-'}</p>
          </div>
          <div>
            <p style={labelStyle}>Last Online</p>
            <p style={valueStyle}>
              {device.last_online_at ? formatDatetime(device.last_online_at) : '-'}
            </p>
          </div>
          <div>
            <p style={labelStyle}>Last Offline</p>
            <p style={valueStyle}>
              {device.last_offline_at ? formatDatetime(device.last_offline_at) : '-'}
            </p>
          </div>
        </div>
      </section>

      <LatestPropertiesSection productId={productId} deviceId={id} />
      <PropertyShadowSection productId={productId} deviceId={id} />
      <FactoryMetadataSection deviceSn={id} />
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

function PropertyHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = usePropertyHistory({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
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
      <h2 style={sectionHeading}>Property History</h2>
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
    </section>
  )
}

function EventHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useEventHistory({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Events',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.events, null, 2)}
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
      <h2 style={sectionHeading}>Event History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No event history"
        pagination={
          pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
        }
        onPageChange={setPage}
      />
    </section>
  )
}

function CommandHistorySection({ productId, deviceId }: { productId: string; deviceId: string }) {
  const [page, setPage] = useState(1)
  const [dialogOpen, setDialogOpen] = useState(false)

  const { data, isLoading } = usePropertyCommands({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const createCommand = useCreatePropertyCommand()
  const deleteCommands = useDeletePropertyCommands()

  const handleDelete = (commandId: number) => {
    deleteCommands.mutate([commandId])
  }

  const statusColors: Record<string, string> = {
    Pending: '#d97706',
    Sent: 'var(--color-accent)',
    Success: '#059669',
    Failed: '#dc2626',
    Deleted: 'var(--color-text-muted)',
  }

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Command',
      accessor: (row) => (
        <pre
          className="max-w-md overflow-auto text-[11px]"
          style={{ fontFamily: "'JetBrains Mono', monospace" }}
        >
          {JSON.stringify(row.command, null, 2)}
        </pre>
      ),
    },
    {
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        return (
          <span
            className="text-[12px] font-semibold"
            style={{ color: statusColors[status] ?? 'var(--color-text-secondary)' }}
          >
            {status}
          </span>
        )
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
            className="text-[12px] font-medium hover:underline disabled:opacity-50"
            style={{ color: '#dc2626' }}
          >
            Delete
          </button>
        ) : null,
    },
  ]

  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <h2 style={sectionHeading}>Property Commands</h2>
        <button
          onClick={() => setDialogOpen(true)}
          className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          style={{ background: 'var(--color-accent)' }}
        >
          Send Command
        </button>
      </div>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No commands"
        pagination={
          pagination
            ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
            : undefined
        }
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
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-xl p-6 shadow-2xl"
        style={{ background: 'var(--color-surface-1)', border: '1px solid var(--color-border)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-[15px] font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          Send Command
        </h3>
        <textarea
          value={jsonInput}
          onChange={(e) => {
            setJsonInput(e.target.value)
            setParseError(null)
          }}
          placeholder='{"key": "value"}'
          rows={8}
          className="mt-4 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            border: '1px solid var(--color-border)',
            background: 'var(--color-surface-2)',
            color: 'var(--color-text-primary)',
          }}
        />
        {parseError && (
          <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
            {parseError}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors"
            style={{
              color: 'var(--color-text-secondary)',
              border: '1px solid var(--color-border)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--color-surface-2)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent'
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSubmitting || !jsonInput.trim()}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {isSubmitting ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}

function ConnectionHistorySection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const [page, setPage] = useState(1)
  const { data, isLoading } = useDeviceStatusHistory({
    product_id: productId,
    device_id: deviceId,
    page,
    page_size: 10,
  })
  const items = data?.data ?? []
  const pagination = data?.pagination

  const columns: Column<Record<string, unknown>>[] = [
    { header: 'ID', accessor: 'id' },
    {
      header: 'Status',
      accessor: (row) => {
        const status = row.status as string
        return (
          <span
            className="text-[12px] font-semibold"
            style={{
              color: status === 'Online' ? '#059669' : 'var(--color-text-muted)',
            }}
          >
            {status}
          </span>
        )
      },
    },
    { header: 'IP Address', accessor: (row) => (row.ip_address as string | null) ?? '-' },
    {
      header: 'Connected At',
      accessor: (row) =>
        (row.connected_at as string | null) ? formatDatetime(row.connected_at as string) : '-',
    },
    {
      header: 'Disconnected At',
      accessor: (row) =>
        (row.disconnected_at as string | null)
          ? formatDatetime(row.disconnected_at as string)
          : '-',
    },
    { header: 'Reason', accessor: (row) => (row.reason as string | null) ?? '-' },
  ]

  return (
    <section>
      <h2 style={sectionHeading}>Connection History</h2>
      <DataTable
        columns={columns}
        data={items as unknown as Record<string, unknown>[]}
        loading={isLoading}
        emptyMessage="No connection history"
        pagination={
          pagination ? { page: pagination.page, pageSize: pagination.page_size } : undefined
        }
        onPageChange={setPage}
      />
    </section>
  )
}
