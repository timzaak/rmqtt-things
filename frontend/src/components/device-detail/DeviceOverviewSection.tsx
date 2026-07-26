import type { DeviceRow } from '@/hooks/useDevices'
import { useDeviceOperations } from '@/hooks/useDeviceOperations'
import { DeviceInfoSection } from './DeviceInfoSection'
import { LatestPropertiesSection } from './LatestPropertiesSection'

/**
 * Overview tab: device info + current properties + a failed-operations
 * summary. The summary runs a count-only operations query (status=Failed,
 * page_size=1) and reads only pagination.total. It is a distinct request
 * from the Operations tab (different params), not a cache reuse.
 */
export function DeviceOverviewSection({
  device,
  productId,
  deviceId,
}: {
  device: DeviceRow
  productId: string
  deviceId: string
}) {
  const { data: failedOps } = useDeviceOperations({
    product_id: productId,
    device_id: deviceId,
    status: 'Failed',
    page: 1,
    page_size: 1,
  })
  const failedTotal = failedOps?.pagination?.total ?? 0

  return (
    <div className="space-y-8">
      <DeviceInfoSection device={device} />
      <LatestPropertiesSection productId={productId} deviceId={deviceId} />
      {failedTotal > 0 && (
        <p
          data-testid="failed-operations-summary"
          className="rounded-xl border px-4 py-3 text-[13px]"
          style={{
            background: 'var(--color-surface-1)',
            borderColor: 'var(--color-border)',
            color: '#dc2626',
          }}
        >
          {failedTotal} operation{failedTotal === 1 ? '' : 's'} failed. See the Operations tab for
          details.
        </p>
      )}
    </div>
  )
}
