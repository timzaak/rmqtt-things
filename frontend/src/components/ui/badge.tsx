import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const badgeVariants = cva('inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium', {
  variants: {
    variant: {
      default: 'bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-300',
      success: 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300',
      danger: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
      warning: 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300',
      info: 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300',
    },
  },
  defaultVariants: {
    variant: 'default',
  },
})

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />
}

// eslint-disable-next-line react-refresh/only-export-components
export { badgeVariants }
