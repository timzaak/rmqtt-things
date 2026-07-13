import { useState } from 'react'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { usePropertyCommands, usePropertyShadow, useSetDesired } from '@/hooks/useProperties'
import { extractErrorMessage } from '@/lib/utils'
import { formatDatetime } from '@/lib/utils'
import type { CommandSource, SetDesiredRequest, ShadowView } from '@/lib/api-generated/types.gen'

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

const metaStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '12px',
  marginTop: '4px',
}

interface DesiredRow {
  key: string
  desiredValue: unknown
  reportedValue: unknown
  reportedTime: string | null
  statusTestid: string
  statusLabel: string
  statusColor: string
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

/** Status colors mirroring CommandHistorySection (show.$id.tsx). */
const STATUS_COLORS = {
  converged: '#059669',
  pending: '#d97706',
  failed: '#dc2626',
} as const

/**
 * Resolve the display status for a desired property key, combining whether the
 * reported value has converged with the delivery status of the most recent
 * DesiredDelta command targeting that key.
 *
 * - converged (reported == desired) -> "Converged" (green)
 * - not converged + command Failed  -> "Delivery failed" (red)
 * - not converged + command Pending -> "Queued" (amber)
 * - not converged + command Sent    -> "Sent" (amber)
 * - not converged + command Success -> "Replied, not converged" (amber)
 * - not converged + no command      -> "Pending convergence" (amber, original wording)
 */
function resolveStatus(
  converged: boolean,
  commandStatus: 'Pending' | 'Sent' | 'Success' | 'Failed' | undefined
): { label: string; color: string } {
  if (converged) {
    return { label: 'Converged', color: STATUS_COLORS.converged }
  }
  switch (commandStatus) {
    case 'Failed':
      return { label: 'Delivery failed', color: STATUS_COLORS.failed }
    case 'Pending':
      return { label: 'Queued', color: STATUS_COLORS.pending }
    case 'Sent':
      return { label: 'Sent', color: STATUS_COLORS.pending }
    case 'Success':
      return { label: 'Replied, not converged', color: STATUS_COLORS.pending }
    default:
      // No DesiredDelta command found for this key yet.
      return { label: 'Pending convergence', color: STATUS_COLORS.pending }
  }
}

interface DesiredDeltaIndex {
  /** key -> most recent DesiredDelta command status (commands already sorted updated_time DESC by backend) */
  byKey: Map<string, 'Pending' | 'Sent' | 'Success' | 'Failed'>
}

/**
 * Build an index from property key -> latest DesiredDelta command status.
 * Only commands whose `source === 'DesiredDelta'` and whose `command` JSON
 * contains the key are considered. The backend returns commands ordered by
 * `updated_time DESC`, so the first match per key wins.
 */
function indexDesiredDeltaCommands(
  commands: Array<{ command: unknown; status: string; source: CommandSource }>
): DesiredDeltaIndex {
  const byKey = new Map<string, 'Pending' | 'Sent' | 'Success' | 'Failed'>()
  for (const cmd of commands) {
    if (cmd.source !== 'DesiredDelta') continue
    if (
      cmd.status !== 'Pending' &&
      cmd.status !== 'Sent' &&
      cmd.status !== 'Success' &&
      cmd.status !== 'Failed'
    ) {
      continue
    }
    const obj =
      cmd.command !== null && typeof cmd.command === 'object' && !Array.isArray(cmd.command)
        ? (cmd.command as Record<string, unknown>)
        : null
    if (!obj) continue
    for (const key of Object.keys(obj)) {
      // First occurrence wins (most recent due to DESC ordering).
      if (!byKey.has(key)) {
        byKey.set(key, cmd.status)
      }
    }
  }
  return { byKey }
}

/**
 * Build table rows from the shadow view, covering ALL desired keys (not just
 * the delta). Each row reports desired/reported values, the reported time, and
 * a status derived from convergence + the latest DesiredDelta command.
 *
 * Per backend `compute_delta` (shadow.rs): `delta` is a map of property key ->
 * bare desired value (only keys that have NOT converged). `desired` holds bare
 * values; `reported` holds values wrapped as `{ value, time }`.
 */
function buildDesiredRows(shadow: ShadowView, deltaIndex: DesiredDeltaIndex): DesiredRow[] {
  const desiredObj = (shadow.desired ?? {}) as Record<string, unknown>
  const reportedObj = (shadow.reported ?? {}) as Record<string, unknown>

  return Object.keys(desiredObj).map((key) => {
    const desiredValue = desiredObj[key]
    const reportedEntry = reportedObj[key]
    const reportedValue = unwrapReportedValue(reportedEntry)
    const reportedTime = unwrapReportedTime(reportedEntry)
    const converged = reportedValue === desiredValue
    const commandStatus = deltaIndex.byKey.get(key)
    const { label, color } = resolveStatus(converged, commandStatus)
    return {
      key,
      desiredValue,
      reportedValue,
      reportedTime,
      statusTestid: statusTestidFor(key),
      statusLabel: label,
      statusColor: color,
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
  // Fetch commands to correlate each desired key with its latest DesiredDelta
  // delivery status. page_size 100 is generous; a single device rarely has
  // more than a handful of recent commands. useSetDesired already invalidates
  // ['property-commands'] on success, so this stays fresh.
  const { data: commandsData } = usePropertyCommands({
    product_id: productId,
    device_id: deviceId,
    page: 1,
    page_size: 100,
  })
  const setDesired = useSetDesired()
  const [dialogOpen, setDialogOpen] = useState(false)

  const desiredDoc = (shadow?.desired ?? {}) as Record<string, unknown>
  const hasDesired = Object.keys(desiredDoc).length > 0

  const deltaIndex = indexDesiredDeltaCommands(commandsData?.data ?? [])
  const desiredRows = shadow ? buildDesiredRows(shadow, deltaIndex) : []

  const columns: Column<DesiredRow>[] = [
    { header: 'Property', accessor: 'key' },
    { header: 'Desired Value', accessor: (row) => formatValue(row.desiredValue) },
    { header: 'Reported Value', accessor: (row) => formatValue(row.reportedValue) },
    {
      header: 'Reported Time',
      accessor: (row) => (row.reportedTime ? formatDatetime(row.reportedTime) : '-'),
    },
    {
      header: 'Status',
      accessor: (row) => (
        <span
          data-testid={row.statusTestid}
          className="text-[12px] font-semibold"
          style={{ color: row.statusColor }}
        >
          {row.statusLabel}
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
          {shadow?.desired_updated_time && (
            <p style={metaStyle}>Desired updated: {formatDatetime(shadow.desired_updated_time)}</p>
          )}
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
            data={desiredRows}
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
