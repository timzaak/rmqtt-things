import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useOtaVersion } from '@/hooks/useOta'
import { useProducts } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { Badge } from '@/components/ui/badge'
import { formatVersion } from '@/lib/version'
import { formatDatetime } from '@/lib/utils'

export const otaShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ota/show/$id',
  component: OtaShowPage,
})

export const Route = otaShowRoute

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <dt className="mb-1 text-sm font-medium" style={{ color: 'var(--color-text-secondary)' }}>
        {label}
      </dt>
      <dd className="text-sm" style={{ color: 'var(--color-text-primary)' }}>
        {children}
      </dd>
    </div>
  )
}

function OtaShowPage() {
  const { id: idStr } = otaShowRoute.useParams()
  const id = Number(idStr)
  const { data: record, isLoading } = useOtaVersion(id)
  const { data: products } = useProducts()

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])

  if (isLoading) {
    return (
      <div className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
        Loading...
      </div>
    )
  }

  if (!record) {
    return (
      <div className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
        OTA version not found.
      </div>
    )
  }

  const extendedRecord = record as typeof record & {
    bin_length?: number | null
    bin_md5?: string | null
  }

  return (
    <div>
      <PageHeader
        title="OTA Version Detail"
        actions={
          <Link
            to="/ota"
            className="rounded-md border px-4 py-2 text-sm font-medium"
            style={{
              borderColor: 'var(--color-border)',
              color: 'var(--color-text-secondary)',
              background: 'var(--color-surface-1)',
            }}
            data-testid="ota-show-back-link"
          >
            Back to List
          </Link>
        }
      />
      <dl className="max-w-lg space-y-4">
        <Field label="ID">{record.id}</Field>
        <Field label="Product">{productMap.get(record.product_id) ?? record.product_id}</Field>
        <Field label="Key">{record.key}</Field>
        <Field label="Version">{formatVersion(record.version)}</Field>
        <Field label="Min Version">{formatVersion(record.min_version)}</Field>
        <Field label="Max Version">
          {record.max_version != null ? formatVersion(record.max_version) : '-'}
        </Field>
        <Field label="File Key">{record.file_key}</Field>
        <Field label="Log">{record.log != null ? String(record.log) : '-'}</Field>
        <Field label="Bin Length">{extendedRecord.bin_length ?? '-'}</Field>
        <Field label="Bin MD5">{extendedRecord.bin_md5 ?? '-'}</Field>
        <Field label="Released At">{formatDatetime(record.released_at)}</Field>
        <Field label="Status">{record.status}</Field>
        <Field label="Created At">{formatDatetime(record.created_at)}</Field>
      </dl>
      <div className="mt-6">
        <dt className="mb-2 text-sm font-medium" style={{ color: 'var(--color-text-secondary)' }}>
          Device IDs
        </dt>
        {record.device_ids?.length ? (
          <dd className="flex flex-wrap gap-1.5">
            {record.device_ids.map((deviceId) => (
              <Badge key={deviceId} variant="info">
                {deviceId}
              </Badge>
            ))}
          </dd>
        ) : (
          <dd className="text-sm" style={{ color: 'var(--color-text-primary)' }}>
            -
          </dd>
        )}
      </div>
    </div>
  )
}
