import { useState } from 'react'
import { useCreatePropertyCommand, usePropertyShadow } from '@/hooks/useProperties'

// Advanced one-time write entry. Adds a Target-conflict warning: a key
// written here that already exists in Target (desired) with a different value
// can leave the device out of sync, because this write does not change Target.
const TARGET_CONFLICT_WARNING =
  'This one-time write does not change Target and may leave the device out of sync.'

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export function DirectPropertyWriteDialog({
  productId,
  deviceId,
  onClose,
}: {
  productId: string
  deviceId: string
  onClose: () => void
}) {
  const [jsonInput, setJsonInput] = useState('')
  const [parseError, setParseError] = useState<string | null>(null)

  const createCommand = useCreatePropertyCommand()
  const { data: shadow } = usePropertyShadow({ product_id: productId, device_id: deviceId })

  // Evaluate the warning live as the user edits valid JSON, so the conflict is
  // visible before submitting. Parse failures are reported on submit instead.
  const targetConflict = (() => {
    if (jsonInput.trim() === '') return false
    let parsed: unknown
    try {
      parsed = JSON.parse(jsonInput)
    } catch {
      return false
    }
    if (!isPlainObject(parsed) || !isPlainObject(shadow?.desired)) return false
    const desired = shadow.desired
    return Object.entries(parsed).some(
      ([key, value]) => key in desired && JSON.stringify(desired[key]) !== JSON.stringify(value)
    )
  })()

  const handleSubmit = () => {
    setParseError(null)
    let parsed: unknown
    try {
      parsed = JSON.parse(jsonInput)
    } catch {
      setParseError('Invalid JSON')
      return
    }
    createCommand.mutate(
      { product_id: productId, device_id: deviceId, command: parsed },
      { onSuccess: onClose }
    )
  }

  return (
    <div
      data-testid="direct-property-write-dialog"
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
          Direct Property Write
        </h3>
        <textarea
          data-testid="command-json-input"
          value={jsonInput}
          onChange={(e) => {
            setJsonInput(e.target.value)
            setParseError(null)
          }}
          placeholder='{"key": "value"}'
          rows={8}
          className="mt-4 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            border: '1px solid var(--color-border)',
            background: 'var(--color-surface-2)',
            color: 'var(--color-text-primary)',
          }}
        />
        {parseError && (
          <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
            {parseError}
          </p>
        )}
        {targetConflict && (
          <p
            data-testid="target-conflict-warning"
            className="mt-2 rounded-lg px-3 py-2 text-[12px]"
            style={{ color: '#d97706', background: 'rgba(217,119,6,0.1)' }}
          >
            {TARGET_CONFLICT_WARNING}
          </p>
        )}
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
            disabled={createCommand.isPending || !jsonInput.trim()}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {createCommand.isPending ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}
