import { useQuery } from '@tanstack/react-query'
import {
  getFactoryDeviceViewHandler,
  queryComponentChangesHandler,
} from '@/lib/api-generated/sdk.gen'
import type {
  FactoryDeviceView,
  PaginatedResponseFactoryMetadataChangeLog,
} from '@/lib/api-generated/types.gen'

export const useFactoryMetadata = (deviceSn: string) => {
  return useQuery({
    queryKey: ['factory-metadata', deviceSn],
    queryFn: async () => {
      const res = await getFactoryDeviceViewHandler({
        path: { deviceSn },
        throwOnError: true,
      })
      return res.data as unknown as FactoryDeviceView
    },
  })
}

export const useComponentChangeLog = (componentSn: string, page: number) => {
  return useQuery({
    queryKey: ['component-change-log', componentSn, page],
    enabled: !!componentSn,
    queryFn: async () => {
      const res = await queryComponentChangesHandler({
        path: { componentSn },
        query: { page, page_size: 10 },
        throwOnError: true,
      })
      return res.data as unknown as PaginatedResponseFactoryMetadataChangeLog
    },
  })
}
