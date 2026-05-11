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
      <dt className="mb-1 text-sm font-medium text-slate-700 dark:text-slate-300">{label}</dt>
      <dd className="text-sm text-slate-900 dark:text-slate-100">{children}</dd>
    </div>
  )
}

function CertsShowPage() {
  const { id: idStr } = certsShowRoute.useParams()
  const id = Number(idStr)
  const { data: cert, isLoading } = useCert(id)

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  if (!cert) {
    return <div className="text-sm text-slate-500">Certificate not found.</div>
  }

  return (
    <div>
      <PageHeader
        title="Certificate Detail"
        actions={
          <Link
            to="/certs"
            className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
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
          <pre className="overflow-x-auto whitespace-pre-wrap rounded-md bg-slate-100 p-3 font-mono text-xs text-slate-900 dark:bg-slate-900 dark:text-slate-100">
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
