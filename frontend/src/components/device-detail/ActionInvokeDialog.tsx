import { useState } from 'react'
import type { CreateActionCommandRequest } from '@/lib/api-generated/types.gen'

// serviceType is free text (PRD A4), but the backend enforces
// `[a-zA-Z0-9_-]{1,32}`. Mirror that contract client-side so
// invalid input never reaches the network.
const SERVICE_TYPE_PATTERN = /^[a-zA-Z0-9_-]{1,32}$/

export function ActionInvokeDialog({
  productId,
  deviceId,
  onClose,
  onSubmit,
  isSubmitting,
}: {
  productId: string
  deviceId: string
  onClose: () => void
  onSubmit: (request: CreateActionCommandRequest) => void
  isSubmitting: boolean
}) {
  const [serviceType, setServiceType] = useState('')
  const [paramsInput, setParamsInput] = useState('')
  const [serviceTypeError, setServiceTypeError] = useState<string | null>(null)
  const [paramsError, setParamsError] = useState<string | null>(null)

  const handleSubmit = () => {
    setServiceTypeError(null)
    setParamsError(null)

    if (!SERVICE_TYPE_PATTERN.test(serviceType)) {
      setServiceTypeError('Allowed: letters, digits, "_" and "-", 1-32 chars')
      return
    }

    const body: CreateActionCommandRequest = {
      productId,
      deviceId,
      serviceType,
    }
    // Only attach params when the user supplied one; an empty box omits the
    // field so the backend default (`{}`) applies.
    if (paramsInput.trim() !== '') {
      try {
        body.params = JSON.parse(paramsInput) as CreateActionCommandRequest['params']
      } catch {
        setParamsError('Invalid JSON')
        return
      }
    }

    onSubmit(body)
  }

  return (
    <div
      data-testid="action-invoke-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-xl p-6 shadow-2xl"
        style={{ background: 'var(--color-surface-1)', border: '1px solid var(--color-border)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-[15px] font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          Invoke Action
        </h3>

        <div className="mt-4">
          <label
            className="text-[11px] font-semibold uppercase tracking-wider"
            style={{ color: 'var(--color-text-muted)' }}
            htmlFor="action-service-type"
          >
            Service Type
          </label>
          <input
            id="action-service-type"
            data-testid="service-type-input"
            type="text"
            value={serviceType}
            onChange={(e) => {
              setServiceType(e.target.value)
              setServiceTypeError(null)
            }}
            placeholder="e.g. reboot"
            className="mt-1 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
            style={{
              fontFamily: "'JetBrains Mono', monospace",
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-2)',
              color: 'var(--color-text-primary)',
            }}
          />
          {serviceTypeError && (
            <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
              {serviceTypeError}
            </p>
          )}
        </div>

        <div className="mt-4">
          <label
            className="text-[11px] font-semibold uppercase tracking-wider"
            style={{ color: 'var(--color-text-muted)' }}
            htmlFor="action-params"
          >
            Params (optional, JSON)
          </label>
          <textarea
            id="action-params"
            data-testid="params-input"
            value={paramsInput}
            onChange={(e) => {
              setParamsInput(e.target.value)
              setParamsError(null)
            }}
            placeholder='{"delaySeconds": 5}'
            rows={6}
            className="mt-1 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
            style={{
              fontFamily: "'JetBrains Mono', monospace",
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-2)',
              color: 'var(--color-text-primary)',
            }}
          />
          {paramsError && (
            <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
              {paramsError}
            </p>
          )}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="cancel-button"
            onClick={onClose}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors"
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
            data-testid="submit-button"
            onClick={handleSubmit}
            disabled={isSubmitting || !serviceType.trim()}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {isSubmitting ? 'Invoking...' : 'Invoke'}
          </button>
        </div>
      </div>
    </div>
  )
}
