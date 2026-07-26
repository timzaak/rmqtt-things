import { LatestPropertiesSection } from './LatestPropertiesSection'
import { PropertyHistorySection } from './PropertyHistorySection'

/**
 * Reported Data tab: pure composition of the existing latest-properties and
 * property-history sections.
 */
export function ReportedDataSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  return (
    <div className="space-y-8">
      <LatestPropertiesSection productId={productId} deviceId={deviceId} />
      <PropertyHistorySection productId={productId} deviceId={deviceId} />
    </div>
  )
}
