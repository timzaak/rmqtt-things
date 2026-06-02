import { cn } from '@/lib/utils'

const variantStyles: Record<string, React.CSSProperties> = {
  default: {
    background: 'var(--color-surface-2)',
    color: 'var(--color-text-secondary)',
  },
  success: {
    background: 'rgba(16, 185, 129, 0.1)',
    color: '#059669',
  },
  danger: {
    background: 'rgba(239, 68, 68, 0.1)',
    color: '#dc2626',
  },
  warning: {
    background: 'rgba(249, 115, 22, 0.1)',
    color: '#ea580c',
  },
  info: {
    background: 'var(--color-accent-soft)',
    color: 'var(--color-accent)',
  },
}

type Variant = keyof typeof variantStyles

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: Variant
}

export function Badge({ className, variant = 'default', style, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold tracking-wide',
        className
      )}
      style={{ ...variantStyles[variant], ...style }}
      {...props}
    />
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export { variantStyles as badgeVariants }
