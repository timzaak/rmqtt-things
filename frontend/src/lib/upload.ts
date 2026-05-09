import SparkMD5 from 'spark-md5'
import { adminFileUploadHandler } from '@/lib/api-generated/sdk.gen'

export type UploadStatus = 'idle' | 'uploading' | 'done' | 'error'

export interface UploadResult {
  fileKey: string
  binLength: number
  binMd5: string
}

export async function uploadOtaFile(file: File): Promise<UploadResult> {
  const arrayBuffer = await file.arrayBuffer()
  const md5 = SparkMD5.ArrayBuffer.hash(arrayBuffer)

  const uploadRes = await adminFileUploadHandler({
    body: {
      directory: 'ota',
      fileName: file.name,
      fileType: file.type || 'application/octet-stream',
      useOriginName: false,
    },
    throwOnError: true,
  })
  const { url, fields } = uploadRes.data as { url: string; fields: Record<string, string> }

  const formData = new FormData()
  for (const [key, value] of Object.entries(fields)) {
    formData.append(key, value)
  }
  formData.append('file', file)

  const s3Res = await fetch(url, { method: 'POST', body: formData })
  if (!s3Res.ok) {
    throw new Error(`S3 upload failed: ${s3Res.status} ${s3Res.statusText}`)
  }

  const fileKey = fields['key'] || `${url}/${fields['key']}`
  return { fileKey, binLength: file.size, binMd5: md5 }
}
