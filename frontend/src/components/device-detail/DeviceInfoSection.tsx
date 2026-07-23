import type { DeviceRow } from '@/hooks/useDevices'
import { formatDatetime } from '@/lib/utils'
import { cardStyle, labelStyle, sectionHeading, valueStyle } from './styles'

export function DeviceInfoSection({ device }: { device: DeviceRow }) {
  return (
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
  )
}
