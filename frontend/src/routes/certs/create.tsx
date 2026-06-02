import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useIssueCert, type IssuedCert } from '@/hooks/useCerts'
import { useProducts } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'
import { toDatetimeLocal } from '@/lib/utils'

function downloadPem(content: string, filename: string) {
  const blob = new Blob([content], { type: 'application/x-pem-file' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

export const certsCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/certs/create',
  component: CertsCreatePage,
})

export const Route = certsCreateRoute

function CertsCreatePage() {
  const issueCert = useIssueCert()
  const { data: products } = useProducts()

  const [issued, setIssued] = useState<{ cert: IssuedCert; deviceId: string } | null>(null)

  const now = new Date()
  const oneYearLater = new Date(now)
  oneYearLater.setFullYear(oneYearLater.getFullYear() + 1)

  const [form, setForm] = useState({
    product_id: '',
    device_id: '',
    force: false,
    start_at: toDatetimeLocal(now),
    end_at: toDatetimeLocal(oneYearLater),
  })

  const isDirty =
    issued === null && (form.product_id !== '' || form.device_id !== '' || form.force !== false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    issueCert.mutate(
      {
        product_id: form.product_id,
        device_id: form.device_id,
        force: form.force,
        start_at: new Date(form.start_at).toISOString(),
        end_at: new Date(form.end_at).toISOString(),
      },
      {
        onSuccess: (data) => {
          setIssued({ cert: data, deviceId: form.device_id })
        },
        onError: (error) => {
          toast.error('Failed to issue certificate', { description: error.message })
        },
      }
    )
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Issue Certificate" />

      {issued ? (
        <div
          className="max-w-lg rounded-md p-6"
          style={{ background: 'var(--color-surface-1)', border: '1px solid #22c55e' }}
        >
          <h2 className="text-lg font-semibold" style={{ color: '#059669' }}>
            Certificate Issued Successfully
          </h2>

          <div
            className="mt-3 rounded p-3 text-sm"
            style={{
              border: '1px solid #d97706',
              background: 'var(--color-surface-2)',
              color: '#d97706',
            }}
          >
            Private key is shown only once. Please download it now as it will not be stored on the
            server.
          </div>

          <div className="mt-4 flex gap-3">
            <button
              type="button"
              onClick={() => downloadPem(issued.cert.cert_pem, `${issued.deviceId}.pem`)}
              className="rounded-md px-4 py-2 text-sm font-medium"
              style={{ background: 'var(--color-accent)', color: '#fff' }}
            >
              Download Certificate
            </button>
            <button
              type="button"
              onClick={() => downloadPem(issued.cert.key_pem, `${issued.deviceId}.key`)}
              className="rounded-md px-4 py-2 text-sm font-medium"
              style={{ background: 'var(--color-accent)', color: '#fff' }}
            >
              Download Private Key
            </button>
          </div>

          <div className="mt-4 space-y-3">
            <div>
              <dt
                className="mb-1 text-sm font-medium"
                style={{ color: 'var(--color-text-secondary)' }}
              >
                Certificate
              </dt>
              <pre
                className="overflow-x-auto whitespace-pre-wrap rounded-md p-3"
                style={{
                  background: 'var(--color-surface-2)',
                  color: 'var(--color-text-primary)',
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: '12px',
                }}
              >
                {issued.cert.cert_pem}
              </pre>
            </div>
            <div>
              <dt
                className="mb-1 text-sm font-medium"
                style={{ color: 'var(--color-text-secondary)' }}
              >
                Private Key
              </dt>
              <pre
                className="overflow-x-auto whitespace-pre-wrap rounded-md p-3"
                style={{
                  background: 'var(--color-surface-2)',
                  color: 'var(--color-text-primary)',
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: '12px',
                }}
              >
                {issued.cert.key_pem}
              </pre>
            </div>
          </div>

          <Link
            to="/certs"
            className="mt-4 inline-block text-sm font-medium underline"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            Back to Certificates
          </Link>
        </div>
      ) : (
        <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
          <div>
            <label
              htmlFor="product_id"
              className="mb-1 block text-sm font-medium"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              Product <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <select
              id="product_id"
              required
              value={form.product_id}
              onChange={(e) => setForm((f) => ({ ...f, product_id: e.target.value }))}
              className="w-full px-3 py-2"
              style={{
                border: '1px solid var(--color-border)',
                background: 'var(--color-surface-1)',
                color: 'var(--color-text-primary)',
                borderRadius: '8px',
                fontSize: '13px',
              }}
            >
              <option value="">Select a product</option>
              {(products?.data ?? []).map((p) => (
                <option key={p.model_no} value={p.model_no}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label
              htmlFor="device_id"
              className="mb-1 block text-sm font-medium"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              Device ID <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <input
              id="device_id"
              type="text"
              required
              value={form.device_id}
              onChange={(e) => setForm((f) => ({ ...f, device_id: e.target.value }))}
              className="w-full px-3 py-2"
              style={{
                border: '1px solid var(--color-border)',
                background: 'var(--color-surface-1)',
                color: 'var(--color-text-primary)',
                borderRadius: '8px',
                fontSize: '13px',
              }}
            />
          </div>
          <div className="flex items-center gap-2">
            <input
              id="force"
              type="checkbox"
              checked={form.force}
              onChange={(e) => setForm((f) => ({ ...f, force: e.target.checked }))}
              className="h-4 w-4"
              style={{ borderColor: 'var(--color-border)' }}
            />
            <label
              htmlFor="force"
              className="text-sm font-medium"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              Force re-issue
            </label>
          </div>
          <div>
            <label
              htmlFor="start_at"
              className="mb-1 block text-sm font-medium"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              Start At <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <input
              id="start_at"
              type="datetime-local"
              required
              value={form.start_at}
              onChange={(e) => setForm((f) => ({ ...f, start_at: e.target.value }))}
              className="w-full px-3 py-2"
              style={{
                border: '1px solid var(--color-border)',
                background: 'var(--color-surface-1)',
                color: 'var(--color-text-primary)',
                borderRadius: '8px',
                fontSize: '13px',
              }}
            />
          </div>
          <div>
            <label
              htmlFor="end_at"
              className="mb-1 block text-sm font-medium"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              End At <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <input
              id="end_at"
              type="datetime-local"
              required
              value={form.end_at}
              onChange={(e) => setForm((f) => ({ ...f, end_at: e.target.value }))}
              className="w-full px-3 py-2"
              style={{
                border: '1px solid var(--color-border)',
                background: 'var(--color-surface-1)',
                color: 'var(--color-text-primary)',
                borderRadius: '8px',
                fontSize: '13px',
              }}
            />
          </div>
          <div className="flex gap-2 pt-2">
            <button
              type="submit"
              disabled={issueCert.isPending}
              className="rounded-md px-4 py-2 text-sm font-medium disabled:opacity-50"
              style={{ background: 'var(--color-accent)', color: '#fff' }}
            >
              {issueCert.isPending ? 'Issuing...' : 'Issue'}
            </button>
            <Link
              to="/certs"
              className="rounded-md px-4 py-2 text-sm font-medium"
              style={{
                border: '1px solid var(--color-border)',
                color: 'var(--color-text-secondary)',
                background: 'var(--color-surface-2)',
              }}
            >
              Cancel
            </Link>
          </div>
        </form>
      )}
    </div>
  )
}
