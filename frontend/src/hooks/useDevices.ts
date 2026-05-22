import { useQuery } from '@tanstack/react-query'
import { getDeviceStatus, getDeviceStatusHistory } from '@/lib/api-generated/sdk.gen'
import type {
  PaginatedResponseDeviceStatusWithSource,
  SimplePaginatedResponseDeviceStatusHistory as StatusHistoryPage,
  RegistrationSource,
} from '@/lib/api-generated/types.gen'

export type DeviceRow = PaginatedResponseDeviceStatusWithSource['data'][number]

interface DeviceStatusParams {
  product_id: string | null
  device_id?: string | null
  status?: null | import('@/lib/api-generated/types.gen').DeviceConnectionStatus
  registration_source?: null | RegistrationSource
  page?: number
  page_size?: number
}

export function useDevices(params: DeviceStatusParams) {
  return useQuery({
    queryKey: ['devices', params],
    queryFn: async () => {
      const res = await getDeviceStatus({
        query: {
          product_id: params.product_id ?? undefined,
          device_id: params.device_id ?? undefined,
          status: params.status ?? undefined,
          registration_source: params.registration_source ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as PaginatedResponseDeviceStatusWithSource
    },
  })
}

interface DeviceStatusHistoryParams {
  product_id: string
  device_id?: string | null
  page?: number
  page_size?: number
}

export function useDeviceStatusHistory(params: DeviceStatusHistoryParams) {
  return useQuery({
    queryKey: ['device-status-history', params],
    queryFn: async () => {
      const res = await getDeviceStatusHistory({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as StatusHistoryPage
    },
  })
}
