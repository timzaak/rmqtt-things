import { Moon, Sun } from 'lucide-react'
import { useTheme } from '@/components/theme/theme-provider'

export function Header() {
  const { theme, toggleTheme } = useTheme()

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
    </header>
  )
}
