import { Moon, Sun } from 'lucide-react'
import { useTheme } from '@/components/theme/theme-provider'

export function Header() {
  const { theme, toggleTheme } = useTheme()

  return (
    <header className="flex h-14 items-center justify-end gap-4 border-b border-slate-200 bg-white px-6 dark:border-slate-700 dark:bg-slate-900">
      <button
        type="button"
        onClick={toggleTheme}
        className="rounded-md p-2 text-slate-500 hover:bg-slate-100 hover:text-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        aria-label="Toggle theme"
      >
        {theme === 'light' ? <Moon className="h-4 w-4" /> : <Sun className="h-4 w-4" />}
      </button>
    </header>
  )
}
