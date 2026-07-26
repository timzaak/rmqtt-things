import { useQuery } from '@tanstack/react-query'
import { getDeviceOperations } from '@/lib/api-generated/sdk.gen'
import type {
  CommandStatus,
  DeviceOperationType,
  DeviceOperationView,
  PaginationInfo,
} from '@/lib/api-generated/types.gen'

// The generated client maps this endpoint to the untyped `PaginatedResponse`
// schema, so declare the concrete page shape locally and cast the response to
// it.
type DeviceOperationPage = {
  data: DeviceOperationView[]
  pagination: PaginationInfo
}

interface DeviceOperationsParams {
  product_id: string
  device_id?: string | null
  operation_type?: DeviceOperationType | null
  status?: CommandStatus | null
  page?: number
  page_size?: number
}

export function useDeviceOperations(params: DeviceOperationsParams) {
  return useQuery({
    queryKey: ['device-operations', params],
    queryFn: async () => {
      const res = await getDeviceOperations({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          operation_type: params.operation_type ?? undefined,
          status: params.status ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as DeviceOperationPage
    },
  })
}
