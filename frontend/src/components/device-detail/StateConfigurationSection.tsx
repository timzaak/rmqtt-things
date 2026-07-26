import { useState } from 'react'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { usePropertyShadow, useSetDesired } from '@/hooks/useProperties'
import { extractErrorMessage, formatDatetime, formatValue } from '@/lib/utils'
import type { SetDesiredRequest, ShadowView } from '@/lib/api-generated/types.gen'
import { sectionHeading } from './styles'

const subtitleStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '12px',
  marginBottom: '16px',
}

const emptyStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  background: 'var(--color-surface-1)',
  border: '1px solid var(--color-border)',
}

interface StateRow {
  key: string
  hasTarget: boolean
  targetValue: unknown
  currentValue: unknown
  reportedTime: string | null
  syncLabel: string
  syncColor: string
  [key: string]: unknown
}

/**
 * Convert a property key to kebab-case for the Apply button data-testid.
 * e.g. `colorTemp` -> `color-temp`, `brightness` -> `brightness`.
 */
function toKebabKey(key: string): string {
  const withSeparators = key
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
  return withSeparators.replace(/^-+|-+$/g, '').toLowerCase()
}

/** Unwrap a reported entry `{ value, time }` into its bare value. */
function unwrapReportedValue(entry: unknown): unknown {
  if (
    entry !== null &&
    typeof entry === 'object' &&
    'value' in (entry as Record<string, unknown>)
  ) {
    return (entry as { value: unknown }).value
  }
  return entry
}

/** Extract the `time` field from a reported `{ value, time }` entry. */
function unwrapReportedTime(entry: unknown): string | null {
  if (entry !== null && typeof entry === 'object' && 'time' in (entry as Record<string, unknown>)) {
    const t = (entry as { time: unknown }).time
    return typeof t === 'string' ? t : null
  }
  return null
}

const SYNC_COLORS = {
  inSync: '#059669',
  outOfSync: '#d97706',
  notSet: 'var(--color-text-muted)',
} as const

/**
 * Build table rows from the shadow view. The row set is the UNION of reported
 * and desired keys: a reported-only key still gets a row with Target shown as
 * "Target not set".
 *
 * Sync mapping, derived solely from ShadowView:
 * - desired lacks key                       -> "Target not set"
 * - desired has key and delta lacks key     -> "In sync"
 * - desired has key and delta has key       -> "Out of sync"
 *
 * Per backend `compute_delta` (shadow.rs): `delta` is the set of property keys
 * that have NOT converged (key -> bare desired value). `desired` holds bare
 * values; `reported` holds values wrapped as `{ value, time }`.
 */
function buildStateRows(shadow: ShadowView): StateRow[] {
  const desiredObj = (shadow.desired ?? {}) as Record<string, unknown>
  const reportedObj = (shadow.reported ?? {}) as Record<string, unknown>
  const deltaObj = (shadow.delta ?? {}) as Record<string, unknown>

  const keys = Array.from(new Set([...Object.keys(reportedObj), ...Object.keys(desiredObj)])).sort()

  return keys.map((key) => {
    const hasTarget = key in desiredObj
    const reportedEntry = reportedObj[key]
    let syncLabel: string
    let syncColor: string
    if (!hasTarget) {
      syncLabel = 'Target not set'
      syncColor = SYNC_COLORS.notSet
    } else if (key in deltaObj) {
      syncLabel = 'Out of sync'
      syncColor = SYNC_COLORS.outOfSync
    } else {
      syncLabel = 'In sync'
      syncColor = SYNC_COLORS.inSync
    }
    return {
      key,
      hasTarget,
      targetValue: desiredObj[key],
      currentValue: unwrapReportedValue(reportedEntry),
      reportedTime: unwrapReportedTime(reportedEntry),
      syncLabel,
      syncColor,
    }
  })
}

export function StateConfigurationSection({
  productId,
  deviceId,
}: {
  productId: string
  deviceId: string
}) {
  const {
    data: shadow,
    isLoading,
    isError,
  } = usePropertyShadow({ product_id: productId, device_id: deviceId })
  const setDesired = useSetDesired()
  const [dialogOpen, setDialogOpen] = useState(false)

  const rows = shadow ? buildStateRows(shadow) : []

  const handleApply = (key: string, targetValue: unknown) => {
    const request: SetDesiredRequest = {
      product_id: productId,
      device_id: deviceId,
      desired: { [key]: targetValue },
    }
    setDesired.mutate(request, {
      onError: (error) => {
        toast.error('Failed to apply target', {
          description: extractErrorMessage(error),
        })
      },
    })
  }

  const columns: Column<StateRow>[] = [
    { header: 'Property', accessor: 'key' },
    {
      header: 'Current',
      accessor: (row) => formatValue(row.currentValue),
    },
    {
      header: 'Target',
      accessor: (row) => (row.hasTarget ? formatValue(row.targetValue) : 'Target not set'),
    },
    {
      header: 'Sync',
      accessor: (row) => (
        <span className="text-[12px] font-semibold" style={{ color: row.syncColor }}>
          {row.syncLabel}
        </span>
      ),
    },
    {
      header: 'Last reported',
      accessor: (row) => (row.reportedTime ? formatDatetime(row.reportedTime) : '-'),
    },
    {
      header: 'Action',
      accessor: (row) =>
        row.hasTarget && row.syncLabel === 'Out of sync' ? (
          <button
            data-testid={`target-apply-button-${toKebabKey(row.key)}`}
            onClick={() => handleApply(row.key, row.targetValue)}
            disabled={setDesired.isPending}
            className="rounded-lg px-2 py-1 text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            Apply
          </button>
        ) : (
          <span style={{ color: 'var(--color-text-muted)' }}>—</span>
        ),
    },
  ]

  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 style={sectionHeading}>State &amp; Configuration</h2>
          <p style={subtitleStyle}>
            Persistent target; changes are applied once and are not auto-retried.
          </p>
        </div>
        <button
          data-testid="target-update-button"
          onClick={() => setDialogOpen(true)}
          className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          style={{ background: 'var(--color-accent)' }}
        >
          Update Target
        </button>
      </div>

      {isError ? (
        <p className="rounded-xl border px-4 py-6 text-center text-[13px]" style={emptyStyle}>
          Failed to load device state
        </p>
      ) : !isLoading && rows.length === 0 ? (
        <p className="rounded-xl px-4 py-3 text-[13px]" style={emptyStyle}>
          No desired state set
        </p>
      ) : (
        <div data-testid="state-configuration-table">
          <DataTable
            columns={columns}
            data={rows}
            loading={isLoading}
            emptyMessage="No properties"
          />
        </div>
      )}

      {dialogOpen && (
        <UpdateTargetDialog
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
                toast.error('Failed to update target', {
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

interface UpdateTargetDialogProps {
  onClose: () => void
  onSubmit: (parsed: Record<string, unknown>) => void
  isSubmitting: boolean
}

/** Dialog for replacing the whole Target (desired) JSON object. */
function UpdateTargetDialog({ onClose, onSubmit, isSubmitting }: UpdateTargetDialogProps) {
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
      data-testid="target-update-dialog"
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
          Update Target
        </h3>
        <textarea
          data-testid="target-json-input"
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
          the target property.
        </p>
        {parseError && (
          <p className="mt-1 text-[12px]" style={{ color: '#dc2626' }}>
            {parseError}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="cancel-button"
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
            data-testid="submit-button"
            onClick={handleSubmit}
            disabled={isSubmitting}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            style={{ background: 'var(--color-accent)' }}
          >
            {isSubmitting ? 'Updating...' : 'Update Target'}
          </button>
        </div>
      </div>
    </div>
  )
}
