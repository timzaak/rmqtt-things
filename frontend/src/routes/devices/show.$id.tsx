import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useDevices, type DeviceRow } from '@/hooks/useDevices'
import { PageHeader } from '@/components/ui/page-header'
import { FactoryMetadataSection } from '@/components/factory-metadata/FactoryMetadataSection'
import { DeviceOverviewSection } from '@/components/device-detail/DeviceOverviewSection'
import { StateConfigurationSection } from '@/components/device-detail/StateConfigurationSection'
import { DeviceOperationsSection } from '@/components/device-detail/DeviceOperationsSection'
import { ReportedDataSection } from '@/components/device-detail/ReportedDataSection'
import { EventHistorySection } from '@/components/device-detail/EventHistorySection'
import { ConnectionHistorySection } from '@/components/device-detail/ConnectionHistorySection'

export const devicesShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/devices/show/$id',
  component: DevicesShowPage,
})

export const Route = devicesShowRoute

const TABS = [
  { key: 'overview', label: 'Overview', testid: undefined },
  {
    key: 'state-configuration',
    label: 'State & Configuration',
    testid: 'device-tab-state-configuration',
  },
  { key: 'operations', label: 'Operations', testid: 'device-tab-operations' },
  { key: 'reported-data', label: 'Reported Data', testid: 'device-tab-reported-data' },
  { key: 'events', label: 'Events', testid: undefined },
  { key: 'connectivity', label: 'Connectivity', testid: undefined },
  { key: 'metadata', label: 'Metadata', testid: undefined },
] as const

type TabKey = (typeof TABS)[number]['key']

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
  const [activeTab, setActiveTab] = useState<TabKey>('overview')

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

      <div
        className="flex flex-wrap gap-1"
        style={{ borderBottom: '1px solid var(--color-border)' }}
        role="tablist"
      >
        {TABS.map((tab) => {
          const active = tab.key === activeTab
          return (
            <button
              key={tab.key}
              role="tab"
              aria-selected={active}
              data-testid={tab.testid}
              onClick={() => setActiveTab(tab.key)}
              className="rounded-t-lg px-3 py-2 text-[13px] font-medium transition-colors"
              style={{
                color: active ? 'var(--color-accent)' : 'var(--color-text-secondary)',
                borderBottom: active ? '2px solid var(--color-accent)' : '2px solid transparent',
                marginBottom: '-1px',
              }}
            >
              {tab.label}
            </button>
          )
        })}
      </div>

      {activeTab === 'overview' && (
        <DeviceOverviewSection device={device} productId={productId} deviceId={id} />
      )}
      {activeTab === 'state-configuration' && (
        <StateConfigurationSection productId={productId} deviceId={id} />
      )}
      {activeTab === 'operations' && (
        <DeviceOperationsSection productId={productId} deviceId={id} />
      )}
      {activeTab === 'reported-data' && <ReportedDataSection productId={productId} deviceId={id} />}
      {activeTab === 'events' && <EventHistorySection productId={productId} deviceId={id} />}
      {activeTab === 'connectivity' && (
        <ConnectionHistorySection productId={productId} deviceId={id} />
      )}
      {activeTab === 'metadata' && <FactoryMetadataSection deviceSn={id} />}
    </div>
  )
}
