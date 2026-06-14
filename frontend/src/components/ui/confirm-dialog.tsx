import { X } from 'lucide-react'

interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  onConfirm: () => void
  confirmText?: string
  variant?: 'default' | 'danger'
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
  confirmText = 'Confirm',
  variant = 'default',
}: ConfirmDialogProps) {
  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="fixed inset-0"
        style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
        onClick={() => onOpenChange(false)}
      />
      <div
        className="relative w-full max-w-md rounded-xl p-6 shadow-2xl"
        style={{
          background: 'var(--color-surface-1)',
          border: '1px solid var(--color-border)',
        }}
      >
        <button
          onClick={() => onOpenChange(false)}
          className="absolute right-4 top-4 transition-colors"
          style={{ color: 'var(--color-text-muted)' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = 'var(--color-text-primary)'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = 'var(--color-text-muted)'
          }}
        >
          <X className="h-4 w-4" />
        </button>
        <h2 className="text-base font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          {title}
        </h2>
        {description && (
          <p
            className="mt-2 text-[13px] leading-relaxed"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            {description}
          </p>
        )}
        <div className="mt-6 flex justify-end gap-2">
          <button
            onClick={() => onOpenChange(false)}
            className="rounded-lg px-4 py-2 text-[13px] font-medium transition-colors duration-150"
            style={{
              color: 'var(--color-text-secondary)',
              border: '1px solid var(--color-border)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--color-surface-2)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent'
            }}
          >
            Cancel
          </button>
          <button
            data-testid="confirm-dialog-confirm"
            onClick={() => {
              onConfirm()
              onOpenChange(false)
            }}
            className="rounded-lg px-4 py-2 text-[13px] font-medium text-white transition-all duration-150"
            style={{
              background: variant === 'danger' ? '#dc2626' : 'var(--color-accent)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.opacity = '0.9'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.opacity = '1'
            }}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  )
}
