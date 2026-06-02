import { Link, useRouterState } from '@tanstack/react-router'
import {
  Package,
  MonitorSmartphone,
  FileCode2,
  ShieldCheck,
  Upload,
  Bell,
  AlertTriangle,
} from 'lucide-react'

const navItems = [
  { to: '/products' as const, label: 'Products', icon: Package },
  { to: '/devices' as const, label: 'Devices', icon: MonitorSmartphone },
  { to: '/valid-templates' as const, label: 'Schema', icon: FileCode2 },
  { to: '/certs' as const, label: 'Certificates', icon: ShieldCheck },
  { to: '/ota' as const, label: 'OTA', icon: Upload },
  { to: '/alarm-rules' as string, label: 'Alarm Rules', icon: Bell },
  { to: '/alarms' as string, label: 'Alarms', icon: AlertTriangle },
] as const

export function Sidebar() {
  const router = useRouterState()
  const pathname = router.location.pathname

  return (
    <aside
      className="flex h-screen w-56 flex-col border-r backdrop-blur-xl"
      style={{
        background: 'var(--sidebar-bg)',
        borderColor: 'var(--sidebar-border)',
      }}
    >
      {/* Brand */}
      <div
        className="flex h-14 items-center gap-2.5 border-b px-5"
        style={{ borderColor: 'var(--sidebar-border)' }}
      >
        <div
          className="flex h-7 w-7 items-center justify-center rounded-lg"
          style={{ background: 'var(--color-accent-soft)' }}
        >
          <MonitorSmartphone className="h-4 w-4" style={{ color: 'var(--color-accent)' }} />
        </div>
        <span
          className="text-[13px] font-semibold tracking-tight"
          style={{ color: 'var(--color-text-primary)' }}
        >
          RMQTT Things
        </span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-0.5 p-2.5">
        {navItems.map(({ to, label, icon: Icon }) => {
          const active = pathname.startsWith(to)
          return (
            <Link
              key={to}
              to={to}
              className="group relative flex items-center gap-2.5 rounded-lg px-3 py-2 text-[13px] font-medium transition-all duration-150"
              style={{
                color: active ? 'var(--color-accent)' : 'var(--color-text-secondary)',
                background: active ? 'var(--color-accent-soft)' : 'transparent',
              }}
              onMouseEnter={(e) => {
                if (!active) e.currentTarget.style.background = 'var(--color-surface-2)'
              }}
              onMouseLeave={(e) => {
                if (!active) e.currentTarget.style.background = 'transparent'
              }}
            >
              {active && (
                <span
                  className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-full"
                  style={{ background: 'var(--color-accent)' }}
                />
              )}
              <Icon className="h-[15px] w-[15px]" />
              {label}
            </Link>
          )
        })}
      </nav>

      {/* Bottom decoration */}
      <div className="p-3">
        <div
          className="rounded-lg p-3"
          style={{
            background: 'var(--color-accent-soft)',
            border: '1px solid var(--color-accent-glow)',
          }}
        >
          <p className="text-[11px] font-medium" style={{ color: 'var(--color-accent)' }}>
            IoT Device Manager
          </p>
          <p className="mt-0.5 text-[10px]" style={{ color: 'var(--color-text-muted)' }}>
            MQTT &middot; Certificate &middot; OTA
          </p>
        </div>
      </div>
    </aside>
  )
}
