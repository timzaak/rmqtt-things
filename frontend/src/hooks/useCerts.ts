import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { listCertsHandler, issueCertHandler, updateCertStatusHandler } from '@/lib/api-generated/sdk.gen'
import type { SimplePaginatedResponseCertIssue as CertPage, IssueCertRequest, UpdateCertStatusRequest, DeviceConnectionStatus } from '@/lib/api-generated/types.gen'

/** Backend will soon return this shape instead of a plain string. */
export interface IssuedCert {
  cert_pem: string
  key_pem: string
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
