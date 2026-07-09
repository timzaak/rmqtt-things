import { useState } from 'react'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { usePropertyShadow, useSetDesired } from '@/hooks/useProperties'
import { extractErrorMessage } from '@/lib/utils'
import type { SetDesiredRequest, ShadowView } from '@/lib/api-generated/types.gen'

const sectionHeading: React.CSSProperties = {
  color: 'var(--color-text-primary)',
  fontSize: '15px',
  fontWeight: 600,
  marginBottom: '4px',
}

const subtitleStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '12px',
  marginBottom: '16px',
}

interface DeltaRow {
  key: string
  desiredValue: unknown
  reportedValue: unknown
  statusTestid: string
  [key: string]: unknown
}

/**
 * Convert a property key to kebab-case for the status data-testid.
 * e.g. `colorTemp` -> `color-temp`, `brightness` -> `brightness`.
 *
 * Strategy: insert a hyphen before each uppercase letter (camel/Pascal boundaries),
 * collapse runs of non-alphanumeric characters into single hyphens, trim leading/
 * trailing hyphens, and lowercase the result.
 */
function toKebabKey(key: string): string {
  const withSeparators = key
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
  return withSeparators.replace(/^-+|-+$/g, '').toLowerCase()
}

function statusTestidFor(key: string): string {
  return `shadow-status-${toKebabKey(key)}`
}

function formatValue(value: unknown): string {
  if (value === undefined || value === null) {
    return '-'
  }
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value)
    } catch {
      return String(value)
    }
  }
  return String(value)
}

/**
 * Build delta table rows from the shadow view.
 *
 * Per backend `compute_delta` (shadow.rs): `delta` is a map of property key ->
 * bare desired value (only keys that have NOT converged). `desired` holds bare
 * values; `reported` holds values wrapped as `{ value, time }`.
 */
function buildDeltaRows(shadow: ShadowView): DeltaRow[] {
  const deltaObj = (shadow.delta ?? {}) as Record<string, unknown>
  const desiredObj = (shadow.desired ?? {}) as Record<string, unknown>
  const reportedObj = (shadow.reported ?? {}) as Record<string, unknown>

  return Object.keys(deltaObj).map((key) => {
    const desiredValue = desiredObj[key]
    const reportedEntry = reportedObj[key] as { value?: unknown } | unknown
    const reportedValue =
      reportedEntry !== null &&
      typeof reportedEntry === 'object' &&
      'value' in (reportedEntry as Record<string, unknown>)
        ? (reportedEntry as { value: unknown }).value
        : reportedEntry
    return {
      key,
      desiredValue,
      reportedValue,
      statusTestid: statusTestidFor(key),
    }
  })
}

interface PropertyShadowSectionProps {
  productId: string
  deviceId: string
}

export function PropertyShadowSection({ productId, deviceId }: PropertyShadowSectionProps) {
  const { data: shadow, isLoading } = usePropertyShadow({
    product_id: productId,
    device_id: deviceId,
  })
  const setDesired = useSetDesired()
  const [dialogOpen, setDialogOpen] = useState(false)

  const desiredDoc = (shadow?.desired ?? {}) as Record<string, unknown>
  const hasDesired = Object.keys(desiredDoc).length > 0

  const deltaRows = shadow ? buildDeltaRows(shadow) : []

  const columns: Column<DeltaRow>[] = [
    { header: 'Property', accessor: 'key' },
    { header: 'Desired Value', accessor: (row) => formatValue(row.desiredValue) },
    { header: 'Reported Value', accessor: (row) => formatValue(row.reportedValue) },
    {
      header: 'Status',
      accessor: (row) => (
        <span
          data-testid={row.statusTestid}
          className="text-[12px] font-semibold"
          style={{ color: '#d97706' }}
        >
          Pending convergence
        </span>
      ),
    },
  ]

  return (
    <section data-testid="shadow-section">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 style={sectionHeading}>Desired State (Shadow)</h2>
          <p style={subtitleStyle}>
            The platform does not auto-repush; the admin decides whether to set again.
          </p>
        </div>
        <button
          data-testid="shadow-set-button"
          onClick={() => setDialogOpen(true)}
          className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          style={{ background: 'var(--color-accent)' }}
        >
          Set Desired State
        </button>
      </div>

      {!isLoading && !hasDesired ? (
        <p
          className="rounded-xl px-4 py-3 text-[13px]"
          style={{
            color: 'var(--color-text-muted)',
            background: 'var(--color-surface-1)',
            border: '1px solid var(--color-border)',
          }}
        >
          No desired state set
        </p>
      ) : (
        <div data-testid="shadow-delta-table">
          <DataTable
            columns={columns}
            data={deltaRows}
            loading={isLoading}
            emptyMessage="Converged"
          />
        </div>
      )}

      {dialogOpen && (
        <SetDesiredDialog
          onClose={() => setDialogOpen(false)}
          onSubmit={(parsed) => {
            const request: SetDesiredRequest = {
              product_id: productId,
              device_id: deviceId,
              desired: parsed,
            }
            setDesired.mutate(request, {
              onSuccess: () => {
                setDialogOpen(false)
              },
              onError: (error) => {
                toast.error('Failed to set desired state', {
                  description: extractErrorMessage(error),
                })
              },
            })
          }}
          isSubmitting={setDesired.isPending}
        />
      )}
    </section>
  )
}

interface SetDesiredDialogProps {
  onClose: () => void
  onSubmit: (parsed: Record<string, unknown>) => void
  isSubmitting: boolean
}

function SetDesiredDialog({ onClose, onSubmit, isSubmitting }: SetDesiredDialogProps) {
  const [jsonInput, setJsonInput] = useState('')
  const [parseError, setParseError] = useState<string | null>(null)

  const handleSubmit = () => {
    setParseError(null)
    try {
      const parsed = JSON.parse(jsonInput)
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        setParseError('Input must be a JSON object')
        return
      }
      onSubmit(parsed as Record<string, unknown>)
    } catch {
      setParseError('Invalid JSON')
    }
  }

  const handleCancel = () => {
    setJsonInput('')
    setParseError(null)
    onClose()
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.5)', backdropFilter: 'blur(4px)' }}
      onClick={handleCancel}
    >
      <div
        className="w-full max-w-lg rounded-xl p-6 shadow-2xl"
        style={{ background: 'var(--color-surface-1)', border: '1px solid var(--color-border)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-[15px] font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          Set Desired State
        </h3>
        <textarea
          data-testid="shadow-desired-editor"
          value={jsonInput}
          onChange={(e) => {
            setJsonInput(e.target.value)
            setParseError(null)
          }}
          placeholder='{"brightness": 80}'
          rows={8}
          className="mt-4 w-full rounded-lg px-3 py-2 text-[12px] placeholder:opacity-40 focus:outline-none"
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            border: '1px solid var(--color-border)',
            background: 'var(--color-surface-2)',
            color: 'var(--color-text-primary)',
          }}
        />
        <p className="mt-2 text-[12px]" style={{ color: 'var(--color-text-muted)' }}>
          A <code style={{ fontFamily: "'JetBrains Mono', monospace" }}>null</code> value removes
          the desired property.
        </p>
        {parseError && (
          <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
            {parseError}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="shadow-cancel-button"
            onClick={handleCancel}
            disabled={isSubmitting}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors disabled:opacity-50"
            style={{
              color: 'var(--color-text-secondary)',
              border: '1px solid var(--color-border)',
            }}
            onMouseEnter={(e) => {
              if (!e.currentTarget.disabled)
                e.currentTarget.style.background = 'var(--color-surface-2)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent'
            }}
          >
            Cancel
          </button>
          <button
            data-testid="shadow-submit-button"
            onClick={handleSubmit}
            disabled={isSubmitting}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {isSubmitting ? 'Setting...' : 'Set Desired'}
          </button>
        </div>
      </div>
    </div>
  )
}
