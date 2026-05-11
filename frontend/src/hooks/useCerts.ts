import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { listCertsHandler, issueCertHandler, updateCertStatusHandler } from '@/lib/api-generated/sdk.gen'
import { client } from '@/lib/api-generated/client.gen'
import type { CertIssue, SimplePaginatedResponseCertIssue as CertPage, IssueCertRequest, UpdateCertStatusRequest, DeviceConnectionStatus } from '@/lib/api-generated/types.gen'

/** Backend will soon return this shape instead of a plain string. */
export interface IssuedCert {
  cert_pem: string
  key_pem: string
}

export interface CaCertResponse {
  ca_pem: string
}

interface CertsParams {
  product_id: string | null
  device_id?: string | null
  status?: null | DeviceConnectionStatus
  page?: number
  page_size?: number
}

export function useCerts(params: CertsParams) {
  return useQuery({
    queryKey: ['certs', params],
    queryFn: async () => {
      const res = await listCertsHandler({
        query: {
          product_id: params.product_id ?? undefined,
          device_id: params.device_id ?? undefined,
          status: params.status ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as CertPage
    },
  })
}

export function useCert(id: number) {
  return useQuery({
    queryKey: ['cert', id],
    queryFn: async () => {
      const res = await client.get<CertIssue, unknown, true>({
        url: '/api/admin/ca/cert/{id}',
        path: { id },
        throwOnError: true,
      })
      return res.data as unknown as CertIssue
    },
    enabled: Number.isFinite(id),
  })
}

export function useIssueCert() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: IssueCertRequest): Promise<IssuedCert> => {
      const res = await issueCertHandler({ body, throwOnError: true })
      return res.data as unknown as IssuedCert
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['certs'] })
    },
  })
}

export function useUpdateCertStatus() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: UpdateCertStatusRequest) => {
      const res = await updateCertStatusHandler({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['certs'] })
    },
  })
}

export function useCaCert() {
  return useQuery({
    queryKey: ['ca-cert'],
    queryFn: async () => {
      const res = await client.get<CaCertResponse, unknown, true>({
        url: '/api/admin/ca/pem',
        throwOnError: true,
      })
      return res.data as unknown as CaCertResponse
    },
  })
}
