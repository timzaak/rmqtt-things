import { LogOut, Moon, Sun } from 'lucide-react'
import { useTheme } from '@/components/theme/theme-provider'
import { logout } from '@/lib/api-generated/sdk.gen'
import { getLoginUrl, resetAuthCheck } from '@/lib/auth'
import { queryClient } from '@/lib/query-client'

export function Header() {
  const { theme, toggleTheme } = useTheme()

  const handleLogout = async () => {
    // Best-effort: revoke the Herald token family; ignore failure so the client
    // always ends the session locally regardless of backend reachability.
    try {
      await logout()
    } catch {
      // no-op
    }
    queryClient.clear()
    resetAuthCheck()
    window.location.href = getLoginUrl()
  }

  return (
    <header
      className="flex h-12 items-center justify-end gap-3 border-b px-6"
      style={{
        background: 'var(--color-surface-1)',
        borderColor: 'var(--sidebar-border)',
      }}
    >
      <button
        type="button"
        onClick={toggleTheme}
        className="flex h-8 w-8 items-center justify-center rounded-lg transition-colors duration-150"
        style={{ color: 'var(--color-text-muted)' }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background = 'var(--color-surface-2)'
          e.currentTarget.style.color = 'var(--color-text-primary)'
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = 'transparent'
          e.currentTarget.style.color = 'var(--color-text-muted)'
        }}
        aria-label="Toggle theme"
      >
        {theme === 'light' ? (
          <Moon className="h-[15px] w-[15px]" />
        ) : (
          <Sun className="h-[15px] w-[15px]" />
        )}
      </button>
      <button
        type="button"
        onClick={handleLogout}
        className="flex h-8 w-8 items-center justify-center rounded-lg transition-colors duration-150"
        style={{ color: 'var(--color-text-muted)' }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background = 'var(--color-surface-2)'
          e.currentTarget.style.color = 'var(--color-text-primary)'
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = 'transparent'
          e.currentTarget.style.color = 'var(--color-text-muted)'
        }}
        aria-label="Log out"
        title="Log out"
      >
        <LogOut className="h-[15px] w-[15px]" />
      </button>
    </header>
  )
}
