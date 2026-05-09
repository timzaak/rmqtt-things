import { useState, useRef } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useOtaVersion, useUpdateOtaVersion } from '@/hooks/useOta'
import { useProducts } from '@/hooks/useProducts'
import { formatVersion, parseVersion, validateVersion, VERSION_REGEX } from '@/lib/version'
import { uploadOtaFile, type UploadStatus } from '@/lib/upload'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const otaEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ota/edit/$id',
  component: OtaEditPage,
})

export const Route = otaEditRoute

interface FormState {
  min_version: string
  max_version: string
  file_key: string
  log: string
  device_ids: string[]
  bin_length: number
  bin_md5: string
}

const inputClass =
  'w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100'
const labelClass = 'mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300'
const disabledClass =
  'w-full rounded-md border border-slate-300 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-400'

function versionBlurHandler(fieldName: string) {
  return (e: React.FocusEvent<HTMLInputElement>) => {
    if (e.target.value && !VERSION_REGEX.test(e.target.value)) {
      toast.error(`${fieldName} must be in x.y.z format (e.g., 1.2.34)`)
    }
  }
}

function OtaEditPage() {
  const { id: idStr } = otaEditRoute.useParams()
  const id = Number(idStr)
  const navigate = useNavigate()
  const { data: otaVersion, isLoading } = useOtaVersion(id)
  const updateMutation = useUpdateOtaVersion()
  const { data: products } = useProducts()

  const [form, setForm] = useState<FormState>({
    min_version: '',
    max_version: '',
    file_key: '',
    log: '',
    device_ids: [],
    bin_length: 0,
    bin_md5: '',
  })
  const [prevData, setPrevData] = useState<typeof otaVersion>(undefined)
  const [initialForm, setInitialForm] = useState<FormState | null>(null)
  const [uploadStatus, setUploadStatus] = useState<UploadStatus>('idle')
  const [uploadError, setUploadError] = useState('')
  const [deviceInput, setDeviceInput] = useState('')
  const fileInputRef = useRef<HTMLInputElement>(null)

  if (otaVersion && otaVersion !== prevData) {
    setPrevData(otaVersion)
    const initialized: FormState = {
      min_version: formatVersion(otaVersion.min_version),
      max_version: otaVersion.max_version != null ? formatVersion(otaVersion.max_version) : '',
      file_key: otaVersion.file_key,
      log: otaVersion.log != null ? String(otaVersion.log) : '',
      device_ids: otaVersion.device_ids ?? [],
      bin_length: (otaVersion as Record<string, unknown>).bin_length as number ?? 0,
      bin_md5: ((otaVersion as Record<string, unknown>).bin_md5 as string) ?? '',
    }
    setForm(initialized)
    setInitialForm(initialized)
  }

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])
  const productName = otaVersion ? (productMap.get(otaVersion.product_id) ?? otaVersion.product_id) : ''

  const isDirty =
    initialForm !== null &&
    (form.min_version !== initialForm.min_version ||
      form.max_version !== initialForm.max_version ||
      form.file_key !== initialForm.file_key ||
      form.log !== initialForm.log ||
      form.device_ids.length !== initialForm.device_ids.length ||
      form.device_ids.some((d, i) => d !== initialForm!.device_ids[i]))

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
    const deviceId = deviceInput.trim()
    if (deviceId && !form.device_ids.includes(deviceId)) {
      setForm((f) => ({ ...f, device_ids: [...f.device_ids, deviceId] }))
    }
    setDeviceInput('')
  }

  const removeDeviceId = (deviceId: string) => {
    setForm((f) => ({ ...f, device_ids: f.device_ids.filter((d) => d !== deviceId) }))
  }

  const handleDeviceKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      addDeviceId()
    }
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!form.min_version) {
      toast.error('Min Version is required')
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

    updateMutation.mutate(
      {
        id,
        min_version: String(parseVersion(form.min_version)),
        max_version: form.max_version ? String(parseVersion(form.max_version)) : null,
        file_key: form.file_key || null,
        log: form.log || null,
        device_ids: form.device_ids.length > 0 ? form.device_ids : null,
        bin_length: form.bin_length,
        bin_md5: form.bin_md5,
      },
      {
        onSuccess: () => {
          toast.success('OTA version updated')
          navigate({ to: '/ota' })
        },
        onError: (error) => {
          toast.error('Failed to update OTA version', { description: error.message })
        },
      },
    )
  }

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  if (!otaVersion) {
    return <div className="text-sm text-red-500">OTA version not found</div>
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit OTA Version" />
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        {/* Product (disabled) */}
        <div>
          <label className={labelClass}>Product</label>
          <input
            type="text"
            disabled
            value={productName}
            className={disabledClass}
          />
        </div>

        {/* Key (disabled) */}
        <div>
          <label className={labelClass}>Key</label>
          <input
            type="text"
            disabled
            value={otaVersion.key}
            className={disabledClass}
          />
        </div>

        {/* Version (disabled) */}
        <div>
          <label className={labelClass}>Version</label>
          <input
            type="text"
            disabled
            value={formatVersion(otaVersion.version)}
            className={disabledClass}
          />
        </div>

        {/* Min Version */}
        <div>
          <label htmlFor="min_version" className={labelClass}>
            Min Version <span className="text-red-500">*</span>
          </label>
          <input
            id="min_version"
            type="text"
            required
            placeholder="e.g., 1.0.0"
            value={form.min_version}
            onBlur={versionBlurHandler('Min Version')}
            onChange={(e) => setForm((f) => ({ ...f, min_version: e.target.value }))}
            className={inputClass}
          />
        </div>

        {/* Max Version */}
        <div>
          <label htmlFor="max_version" className={labelClass}>
            Max Version
          </label>
          <input
            id="max_version"
            type="text"
            placeholder="e.g., 2.0.0"
            value={form.max_version}
            onBlur={versionBlurHandler('Max Version')}
            onChange={(e) => setForm((f) => ({ ...f, max_version: e.target.value }))}
            className={inputClass}
          />
        </div>

        {/* File Upload */}
        <div>
          <label className={labelClass}>Firmware File</label>
          <input
            ref={fileInputRef}
            type="file"
            onChange={handleFileChange}
            className="block w-full text-sm text-slate-500 file:mr-4 file:rounded-md file:border-0 file:bg-slate-900 file:px-4 file:py-2 file:text-sm file:font-medium file:text-white hover:file:bg-slate-800 dark:file:bg-slate-100 dark:file:text-slate-900 dark:hover:file:bg-slate-200"
          />
          {uploadStatus === 'uploading' && (
            <p className="mt-1 text-sm text-blue-600 dark:text-blue-400">Uploading...</p>
          )}
          {uploadStatus === 'done' && (
            <p className="mt-1 text-sm text-green-600 dark:text-green-400">File uploaded</p>
          )}
          {uploadStatus === 'error' && (
            <p className="mt-1 text-sm text-red-600 dark:text-red-400">{uploadError}</p>
          )}
        </div>

        {/* Bin Length (readonly) */}
        <div>
          <label className={labelClass}>Bin Length</label>
          <input
            type="text"
            readOnly
            value={form.bin_length || ''}
            placeholder="Auto-calculated on file upload"
            className={`${inputClass} bg-slate-50 dark:bg-slate-900`}
          />
        </div>

        {/* Bin MD5 (readonly) */}
        <div>
          <label className={labelClass}>Bin MD5</label>
          <input
            type="text"
            readOnly
            value={form.bin_md5}
            placeholder="Auto-calculated on file upload"
            className={`${inputClass} bg-slate-50 dark:bg-slate-900`}
          />
        </div>

        {/* Log */}
        <div>
          <label htmlFor="log" className={labelClass}>
            Log
          </label>
          <textarea
            id="log"
            value={form.log}
            onChange={(e) => setForm((f) => ({ ...f, log: e.target.value }))}
            rows={3}
            className={inputClass}
          />
        </div>

        {/* Device IDs */}
        <div>
          <label className={labelClass}>Device IDs</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={deviceInput}
              onChange={(e) => setDeviceInput(e.target.value)}
              onKeyDown={handleDeviceKeyDown}
              placeholder="Enter device ID and press Enter"
              className={inputClass}
            />
            <button
              type="button"
              onClick={addDeviceId}
              className="shrink-0 rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
            >
              Add
            </button>
          </div>
          {form.device_ids.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1">
              {form.device_ids.map((deviceId) => (
                <span
                  key={deviceId}
                  className="inline-flex items-center gap-1 rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-700 dark:bg-slate-700 dark:text-slate-200"
                >
                  {deviceId}
                  <button
                    type="button"
                    onClick={() => removeDeviceId(deviceId)}
                    className="text-slate-400 hover:text-red-500 dark:text-slate-500 dark:hover:text-red-400"
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
            disabled={updateMutation.isPending || uploadStatus === 'uploading'}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            {updateMutation.isPending ? 'Saving...' : 'Save'}
          </button>
          <Link
            to="/ota"
            className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
