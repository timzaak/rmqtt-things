import { useEffect, useState } from 'react'
import { DataTable, type Column } from '@/components/ui/data-table'
import { toast } from '@/components/ui/sonner'
import { useFactoryMetadata } from '@/hooks/useFactoryMetadata'
import { extractErrorMessage, formatDatetime } from '@/lib/utils'
import type { FileAttachment, FactoryComponentView } from '@/lib/api-generated/types.gen'
import { ComponentChangeLogDrawer } from './ComponentChangeLogDrawer'

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

const placeholderCardStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  background: 'var(--color-surface-1)',
  border: '1px solid var(--color-border)',
  borderRadius: '12px',
  padding: '16px',
  fontSize: '13px',
}

const mutedMetadataStyle: React.CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '12px',
  fontStyle: 'italic',
}

/**
 * Read-only text rendering of a component's file attachments (design §4.4.2,
 * scope-adjusted: no S3 direct links, no presigned URLs). Each attachment is
 * shown as its `fileName` plus an optional `(contentType, sizeBytes)` suffix.
 *
 * Structured as a sub-component for clarity only; it renders plain text only —
 * no anchor elements, no download hooks, no URL fetching of any kind.
 */
function FileAttachmentLinks({ attachments }: { attachments: FileAttachment[] }) {
  if (attachments.length === 0) {
    return <span style={{ color: 'var(--color-text-muted)' }}>-</span>
  }
  return (
    <ul className="space-y-1">
      {attachments.map((attachment) => {
        const meta: string[] = []
        if (attachment.contentType) {
          meta.push(attachment.contentType)
        }
        if (typeof attachment.sizeBytes === 'number' && attachment.sizeBytes > 0) {
          meta.push(formatBytes(attachment.sizeBytes))
        }
        const metaSuffix = meta.length > 0 ? ` (${meta.join(', ')})` : ''
        return (
          <li
            key={attachment.fileKey}
            className="text-[12px]"
            style={{
              color: 'var(--color-text-primary)',
              fontFamily: "'JetBrains Mono', monospace",
            }}
          >
            {attachment.fileName}
            {metaSuffix && <span style={{ color: 'var(--color-text-muted)' }}>{metaSuffix}</span>}
          </li>
        )
      })}
    </ul>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
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

interface FactoryComponentRow {
  componentSn: string
  componentType: string | null
  metadata: unknown
  fileAttachments: FileAttachment[]
  updatedAt: string | null
  [key: string]: unknown
}

function toRow(component: FactoryComponentView): FactoryComponentRow {
  return {
    componentSn: component.componentSn,
    componentType: component.componentType ?? null,
    metadata: component.metadata ?? null,
    fileAttachments: (component.fileAttachments ?? []) as FileAttachment[],
    updatedAt: component.updatedAt ?? null,
  }
}

/**
 * Coerce the react-query error payload into the backend's 404 body shape
 * `{ error: string }`. Returns null if the payload does not match.
 */
function readErrorMessage(error: unknown): string | null {
  if (
    error !== null &&
    typeof error === 'object' &&
    'error' in error &&
    typeof (error as { error?: unknown }).error === 'string'
  ) {
    return (error as { error: string }).error
  }
  return null
}

const NOT_FOUND_MESSAGE = 'Device has no factory metadata'

function isNotFoundError(error: unknown): boolean {
  return readErrorMessage(error)?.includes(NOT_FOUND_MESSAGE) ?? false
}

interface FactoryMetadataSectionProps {
  deviceSn: string
}

export function FactoryMetadataSection({ deviceSn }: FactoryMetadataSectionProps) {
  const { data, isLoading, isError, error } = useFactoryMetadata(deviceSn)
  const [selectedSn, setSelectedSn] = useState<string | null>(null)
  const [deviceLogOpen, setDeviceLogOpen] = useState(false)

  // Surface non-404 errors via toast (consistent with PropertyShadowSection's
  // error styling). The 404 "no metadata" branch is a normal empty state and
  // must NOT toast.
  useEffect(() => {
    if (isError && !isNotFoundError(error)) {
      toast.error('Failed to load factory metadata', {
        description: extractErrorMessage(error),
      })
    }
  }, [isError, error])

  const showNotFoundCard = isError && isNotFoundError(error)
  const showGenericErrorCard = isError && !isNotFoundError(error)

  const rows = data ? data.components.map(toRow) : []

  // A single drawer instance serves both component-level and device-level
  // change logs. `drawerSn` collapses the two independent triggers into one
  // nullable SN: a selected component SN wins, otherwise fall back to the
  // device SN when the device-level entry is open. The parent's `key={drawerSn}`
  // remounts the drawer on any SN switch so its internal page resets cleanly.
  const drawerSn = selectedSn ?? (deviceLogOpen ? deviceSn : null)

  const columns: Column<FactoryComponentRow>[] = [
    {
      header: 'Component SN',
      accessor: (row) => (
        // data-testid mounted on the first-cell container (DataTable owns <tr>).
        <span data-testid={`factory-component-row-${row.componentSn}`}>{row.componentSn}</span>
      ),
    },
    {
      header: 'Type',
      accessor: (row) =>
        row.componentType ? (
          <span style={{ color: 'var(--color-text-primary)' }}>{row.componentType}</span>
        ) : (
          <span style={{ color: 'var(--color-text-muted)' }}>-</span>
        ),
    },
    {
      header: 'Metadata',
      accessor: (row) =>
        row.metadata === null || row.metadata === undefined ? (
          <span style={mutedMetadataStyle}>Metadata not arrived</span>
        ) : (
          <pre
            className="max-w-md overflow-auto text-[11px]"
            style={{ fontFamily: "'JetBrains Mono', monospace" }}
          >
            {JSON.stringify(row.metadata, null, 2)}
          </pre>
        ),
    },
    {
      header: 'File Attachments',
      accessor: (row) => <FileAttachmentLinks attachments={row.fileAttachments} />,
    },
    {
      header: 'Updated At',
      accessor: (row) =>
        row.updatedAt ? (
          formatDatetime(row.updatedAt)
        ) : (
          <span style={{ color: 'var(--color-text-muted)' }}>-</span>
        ),
    },
    {
      header: 'Actions',
      accessor: (row) => (
        <button
          data-testid={`factory-component-changes-btn-${row.componentSn}`}
          onClick={() => setSelectedSn(row.componentSn)}
          className="text-[12px] font-medium hover:underline transition-opacity hover:opacity-80"
          style={{ color: 'var(--color-accent)' }}
        >
          View change log
        </button>
      ),
    },
  ]

  return (
    <section data-testid="factory-metadata-section">
      <div className="mb-4">
        <h2 style={sectionHeading}>Factory Metadata</h2>
        <p style={subtitleStyle}>
          Reported by the factory line; partial components may not have arrived yet.
        </p>
      </div>

      <p
        className="mb-4 rounded-xl px-4 py-2 text-[12px]"
        style={{
          color: 'var(--color-text-muted)',
          background: 'var(--color-surface-1)',
          border: '1px solid var(--color-border)',
        }}
        data-testid="factory-device-metadata-block"
      >
        {`Device-level metadata:${data?.deviceMetadata ? '' : ' not available'}`}
        {data?.deviceMetadata && (
          <>
            <pre
              className="mt-2 overflow-auto text-[11px]"
              style={{ fontFamily: "'JetBrains Mono', monospace" }}
            >
              {formatValue(data.deviceMetadata.metadata)}
            </pre>
            <FileAttachmentLinks
              attachments={
                (data.deviceMetadata.fileAttachments ?? []) as unknown as FileAttachment[]
              }
            />
            {data.deviceMetadata.updatedAt && (
              <span style={{ display: 'block', marginTop: '4px' }}>
                Updated {formatDatetime(data.deviceMetadata.updatedAt)}
              </span>
            )}
            <button
              data-testid="factory-device-changes-btn"
              onClick={() => setDeviceLogOpen(true)}
              className="mt-2 text-[12px] font-medium hover:underline transition-opacity hover:opacity-80"
              style={{ color: 'var(--color-accent)' }}
            >
              View change log
            </button>
          </>
        )}
      </p>

      {showNotFoundCard ? (
        <p style={placeholderCardStyle} data-testid="factory-metadata-empty">
          This device has no factory metadata
        </p>
      ) : showGenericErrorCard ? (
        <p style={placeholderCardStyle} data-testid="factory-metadata-error">
          Failed to load factory metadata
        </p>
      ) : (
        <DataTable columns={columns} data={rows} loading={isLoading} emptyMessage="No components" />
      )}

      <ComponentChangeLogDrawer
        key={drawerSn ?? 'closed'}
        sn={drawerSn}
        onClose={() => {
          setSelectedSn(null)
          setDeviceLogOpen(false)
        }}
      />
    </section>
  )
}
