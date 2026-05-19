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
    <aside className="flex h-screen w-56 flex-col border-r border-slate-200 bg-white dark:border-slate-700 dark:bg-slate-900">
      <div className="flex h-14 items-center gap-2 border-b border-slate-200 px-4 dark:border-slate-700">
        <MonitorSmartphone className="h-5 w-5 text-slate-700 dark:text-slate-300" />
        <span className="text-sm font-semibold text-slate-900 dark:text-slate-100">
          RMQTT Things
        </span>
      </div>
      <nav className="flex-1 space-y-1 p-2">
        {navItems.map(({ to, label, icon: Icon }) => {
          const active = pathname.startsWith(to)
          return (
            <Link
              key={to}
              to={to}
              className={`flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                active
                  ? 'bg-slate-100 text-slate-900 dark:bg-slate-800 dark:text-slate-100'
                  : 'text-slate-600 hover:bg-slate-50 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200'
              }`}
            >
              <Icon className="h-4 w-4" />
              {label}
            </Link>
          )
        })}
      </nav>
    </aside>
  )
}
