import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useCert } from '@/hooks/useCerts'
import { PageHeader } from '@/components/ui/page-header'
import { formatDatetime } from '@/lib/utils'
import type { CertStatus } from '@/lib/api-generated/types.gen'

export const certsShowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/certs/show/$id',
  component: CertsShowPage,
})

export const Route = certsShowRoute

const statusLabel: Record<CertStatus, string> = {
  Normal: 'Active',
  InValid: 'Invalid',
  Revoked: 'Revoked',
}

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

function CertsShowPage() {
  const { id: idStr } = certsShowRoute.useParams()
  const id = Number(idStr)
  const { data: cert, isLoading } = useCert(id)

  if (isLoading) {
    return (
      <div className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Loading...
      </div>
    )
  }

  if (!cert) {
    return (
      <div className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Certificate not found.
      </div>
    )
  }

  return (
    <div>
      <PageHeader
        title="Certificate Detail"
        actions={
          <Link
            to="/certs"
            className="rounded-md px-4 py-2 text-sm font-medium"
            style={{
              border: '1px solid var(--color-border)',
              color: 'var(--color-text-secondary)',
              background: 'var(--color-surface-2)',
            }}
          >
            Back to Certificates
          </Link>
        }
      />
      <dl className="max-w-3xl space-y-4">
        <Field label="ID">{cert.id}</Field>
        <Field label="Product">{cert.product_id}</Field>
        <Field label="Device ID">{cert.device_id}</Field>
        <Field label="Certificate">
          <pre
            className="overflow-x-auto whitespace-pre-wrap rounded-md p-3"
            style={{
              background: 'var(--color-surface-2)',
              color: 'var(--color-text-primary)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '12px',
            }}
          >
            {cert.pub_cert}
          </pre>
        </Field>
        <Field label="Status">{statusLabel[cert.status] ?? cert.status}</Field>
        <Field label="Start At">{formatDatetime(cert.start_at)}</Field>
        <Field label="End At">{formatDatetime(cert.end_at)}</Field>
        <Field label="Created At">{formatDatetime(cert.created_at)}</Field>
      </dl>
    </div>
  )
}
