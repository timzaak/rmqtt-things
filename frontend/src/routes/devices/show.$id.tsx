import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useDevices, type DeviceRow } from '@/hooks/useDevices'
import { PageHeader } from '@/components/ui/page-header'
import { PropertyShadowSection } from '@/components/property-shadow/PropertyShadowSection'
import { FactoryMetadataSection } from '@/components/factory-metadata/FactoryMetadataSection'
import { DeviceInfoSection } from '@/components/device-detail/DeviceInfoSection'
import { LatestPropertiesSection } from '@/components/device-detail/LatestPropertiesSection'
import { PropertyHistorySection } from '@/components/device-detail/PropertyHistorySection'
import { EventHistorySection } from '@/components/device-detail/EventHistorySection'
import { PropertyCommandsSection } from '@/components/device-detail/PropertyCommandsSection'
import { ConnectionHistorySection } from '@/components/device-detail/ConnectionHistorySection'

export const devicesShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/devices/show/$id',
  component: DevicesShowPage,
})

export const Route = devicesShowRoute

const TABS = [
  { key: 'overview', label: 'Overview' },
  { key: 'shadow', label: 'Shadow' },
  { key: 'commands', label: 'Commands' },
  { key: 'property-history', label: 'Property History' },
  { key: 'events', label: 'Events' },
  { key: 'connection', label: 'Connection' },
  { key: 'factory-metadata', label: 'Factory Metadata' },
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
        <div className="space-y-8">
          <DeviceInfoSection device={device} />
          <LatestPropertiesSection productId={productId} deviceId={id} />
        </div>
      )}
      {activeTab === 'shadow' && <PropertyShadowSection productId={productId} deviceId={id} />}
      {activeTab === 'commands' && <PropertyCommandsSection productId={productId} deviceId={id} />}
      {activeTab === 'property-history' && (
        <PropertyHistorySection productId={productId} deviceId={id} />
      )}
      {activeTab === 'events' && <EventHistorySection productId={productId} deviceId={id} />}
      {activeTab === 'connection' && (
        <ConnectionHistorySection productId={productId} deviceId={id} />
      )}
      {activeTab === 'factory-metadata' && <FactoryMetadataSection deviceSn={id} />}
    </div>
  )
}
