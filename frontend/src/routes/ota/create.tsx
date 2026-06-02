import { useState, useRef } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useCreateOtaVersion } from '@/hooks/useOta'
import { useProducts } from '@/hooks/useProducts'
import { parseVersion, validateVersion, VERSION_REGEX } from '@/lib/version'
import { uploadOtaFile, type UploadStatus } from '@/lib/upload'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const otaCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ota/create',
  component: OtaCreatePage,
})

export const Route = otaCreateRoute

interface FormState {
  product_id: string
  key: string
  version: string
  min_version: string
  max_version: string
  file_key: string
  log: string
  device_ids: string[]
  bin_length: number
  bin_md5: string
}

const initialForm: FormState = {
  product_id: '',
  key: '',
  version: '',
  min_version: '',
  max_version: '',
  file_key: '',
  log: '',
  device_ids: [],
  bin_length: 0,
  bin_md5: '',
}

const inputStyle = {
  border: '1px solid var(--color-border)',
  background: 'var(--color-surface-1)',
  color: 'var(--color-text-primary)',
  borderRadius: '8px',
  fontSize: '13px',
}
const labelStyle = { color: 'var(--color-text-secondary)' }

function versionBlurHandler(fieldName: string) {
  return (e: React.FocusEvent<HTMLInputElement>) => {
    if (e.target.value && !VERSION_REGEX.test(e.target.value)) {
      toast.error(`${fieldName} must be in x.y.z format (e.g., 1.2.34)`)
    }
  }
}

function OtaCreatePage() {
  const navigate = useNavigate()
  const createOtaVersion = useCreateOtaVersion()
  const { data: products } = useProducts()
  const [form, setForm] = useState<FormState>(initialForm)
  const [uploadStatus, setUploadStatus] = useState<UploadStatus>('idle')
  const [uploadError, setUploadError] = useState('')
  const [deviceInput, setDeviceInput] = useState('')
  const fileInputRef = useRef<HTMLInputElement>(null)

  const isDirty =
    form.product_id !== '' ||
    form.key !== '' ||
    form.version !== '' ||
    form.min_version !== '' ||
    form.max_version !== '' ||
    form.file_key !== '' ||
    form.log !== '' ||
    form.device_ids.length > 0

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    setUploadStatus('uploading')
    setUploadError('')

    try {
      const result = await uploadOtaFile(file)
      setForm((f) => ({
        ...f,
        file_key: result.fileKey,
        bin_length: result.binLength,
        bin_md5: result.binMd5,
      }))
      setUploadStatus('done')
    } catch (err) {
      setUploadStatus('error')
      setUploadError(err instanceof Error ? err.message : 'Upload failed')
      toast.error('File upload failed', {
        description: err instanceof Error ? err.message : 'Unknown error',
      })
    }
  }

  const addDeviceId = () => {
    const id = deviceInput.trim()
    if (id && !form.device_ids.includes(id)) {
      setForm((f) => ({ ...f, device_ids: [...f.device_ids, id] }))
    }
    setDeviceInput('')
  }

  const removeDeviceId = (id: string) => {
    setForm((f) => ({ ...f, device_ids: f.device_ids.filter((d) => d !== id) }))
  }

  const handleDeviceKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      addDeviceId()
    }
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!form.product_id || !form.key || !form.version || !form.min_version || !form.file_key) {
      toast.error('Please fill in all required fields')
      return
    }

    const versionError = validateVersion(form.version, 'Version')
    if (versionError) {
      toast.error(versionError)
      return
    }
    const minVersionError = validateVersion(form.min_version, 'Min Version')
    if (minVersionError) {
      toast.error(minVersionError)
      return
    }
    if (form.max_version) {
      const maxVersionError = validateVersion(form.max_version, 'Max Version')
      if (maxVersionError) {
        toast.error(maxVersionError)
        return
      }
    }

    createOtaVersion.mutate(
      {
        product_id: form.product_id,
        key: form.key,
        version: String(parseVersion(form.version)),
        min_version: String(parseVersion(form.min_version)),
        max_version: form.max_version ? String(parseVersion(form.max_version)) : null,
        file_key: form.file_key,
        log: form.log || null,
        device_ids: form.device_ids.length > 0 ? form.device_ids : null,
        bin_length: form.bin_length,
        bin_md5: form.bin_md5,
      },
      {
        onSuccess: () => {
          toast.success('OTA version created')
          navigate({ to: '/ota' })
        },
        onError: (error) => {
          toast.error('Failed to create OTA version', { description: error.message })
        },
      }
    )
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Create OTA Version" />
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        {/* Product */}
        <div>
          <label htmlFor="product_id" className="mb-1 block text-sm font-medium" style={labelStyle}>
            Product <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <select
            id="product_id"
            required
            value={form.product_id}
            onChange={(e) => setForm((f) => ({ ...f, product_id: e.target.value }))}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          >
            <option value="">Select a product</option>
            {products?.data?.map((p) => (
              <option key={p.id} value={p.model_no}>
                {p.name}
              </option>
            ))}
          </select>
        </div>

        {/* Key */}
        <div>
          <label htmlFor="key" className="mb-1 block text-sm font-medium" style={labelStyle}>
            Key <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            id="key"
            type="text"
            required
            value={form.key}
            onChange={(e) => setForm((f) => ({ ...f, key: e.target.value }))}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          />
        </div>

        {/* Version */}
        <div>
          <label htmlFor="version" className="mb-1 block text-sm font-medium" style={labelStyle}>
            Version <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            id="version"
            type="text"
            required
            placeholder="e.g., 1.2.34"
            value={form.version}
            onBlur={versionBlurHandler('Version')}
            onChange={(e) => setForm((f) => ({ ...f, version: e.target.value }))}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          />
        </div>

        {/* Min Version */}
        <div>
          <label
            htmlFor="min_version"
            className="mb-1 block text-sm font-medium"
            style={labelStyle}
          >
            Min Version <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            id="min_version"
            type="text"
            required
            placeholder="e.g., 1.0.0"
            value={form.min_version}
            onBlur={versionBlurHandler('Min Version')}
            onChange={(e) => setForm((f) => ({ ...f, min_version: e.target.value }))}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          />
        </div>

        {/* Max Version */}
        <div>
          <label
            htmlFor="max_version"
            className="mb-1 block text-sm font-medium"
            style={labelStyle}
          >
            Max Version
          </label>
          <input
            id="max_version"
            type="text"
            placeholder="e.g., 2.0.0"
            value={form.max_version}
            onBlur={versionBlurHandler('Max Version')}
            onChange={(e) => setForm((f) => ({ ...f, max_version: e.target.value }))}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          />
        </div>

        {/* File Upload */}
        <div>
          <label className="mb-1 block text-sm font-medium" style={labelStyle}>
            Firmware File <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            ref={fileInputRef}
            type="file"
            onChange={handleFileChange}
            className="block w-full text-sm"
            style={{ color: 'var(--color-text-muted)' }}
          />
          {uploadStatus === 'uploading' && (
            <p className="mt-1 text-sm" style={{ color: 'var(--color-accent)' }}>
              Uploading...
            </p>
          )}
          {uploadStatus === 'done' && (
            <p className="mt-1 text-sm" style={{ color: '#059669' }}>
              File uploaded
            </p>
          )}
          {uploadStatus === 'error' && (
            <p className="mt-1 text-sm" style={{ color: '#dc2626' }}>
              {uploadError}
            </p>
          )}
        </div>

        {/* Bin Length (auto) */}
        <div>
          <label className="mb-1 block text-sm font-medium" style={labelStyle}>
            Bin Length
          </label>
          <input
            type="text"
            readOnly
            value={form.bin_length || ''}
            placeholder="Auto-calculated on file upload"
            className="w-full px-3 py-2 text-sm"
            style={{ ...inputStyle, background: 'var(--color-surface-2)' }}
          />
        </div>

        {/* Bin MD5 (auto) */}
        <div>
          <label className="mb-1 block text-sm font-medium" style={labelStyle}>
            Bin MD5
          </label>
          <input
            type="text"
            readOnly
            value={form.bin_md5}
            placeholder="Auto-calculated on file upload"
            className="w-full px-3 py-2 text-sm"
            style={{ ...inputStyle, background: 'var(--color-surface-2)' }}
          />
        </div>

        {/* Log */}
        <div>
          <label htmlFor="log" className="mb-1 block text-sm font-medium" style={labelStyle}>
            Log
          </label>
          <textarea
            id="log"
            value={form.log}
            onChange={(e) => setForm((f) => ({ ...f, log: e.target.value }))}
            rows={3}
            className="w-full px-3 py-2 text-sm"
            style={inputStyle}
          />
        </div>

        {/* Device IDs */}
        <div>
          <label className="mb-1 block text-sm font-medium" style={labelStyle}>
            Device IDs
          </label>
          <div className="flex gap-2">
            <input
              type="text"
              value={deviceInput}
              onChange={(e) => setDeviceInput(e.target.value)}
              onKeyDown={handleDeviceKeyDown}
              placeholder="Enter device ID and press Enter"
              className="w-full px-3 py-2 text-sm"
              style={inputStyle}
            />
            <button
              type="button"
              onClick={addDeviceId}
              className="shrink-0 rounded-md border px-3 py-2 text-sm font-medium"
              style={{
                borderColor: 'var(--color-border)',
                color: 'var(--color-text-secondary)',
                background: 'var(--color-surface-1)',
              }}
            >
              Add
            </button>
          </div>
          {form.device_ids.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1">
              {form.device_ids.map((id) => (
                <span
                  key={id}
                  className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium"
                  style={{
                    background: 'var(--color-surface-2)',
                    color: 'var(--color-text-secondary)',
                  }}
                >
                  {id}
                  <button
                    type="button"
                    onClick={() => removeDeviceId(id)}
                    style={{ color: 'var(--color-text-muted)' }}
                  >
                    x
                  </button>
                </span>
              ))}
            </div>
          )}
        </div>

        {/* Submit / Cancel */}
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={createOtaVersion.isPending || uploadStatus === 'uploading'}
            className="rounded-md px-4 py-2 text-sm font-medium disabled:opacity-50"
            style={{ background: 'var(--color-accent)', color: '#fff' }}
          >
            {createOtaVersion.isPending ? 'Creating...' : 'Create'}
          </button>
          <Link
            to="/ota"
            className="rounded-md border px-4 py-2 text-sm font-medium"
            style={{
              borderColor: 'var(--color-border)',
              color: 'var(--color-text-secondary)',
              background: 'var(--color-surface-1)',
            }}
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
