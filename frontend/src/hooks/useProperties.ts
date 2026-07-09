import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getPropertyLatest,
  getPropertyHistory,
  getPropertyCommands,
  createPropertyCommand,
  deletePropertyCommands,
  getPropertyShadow,
  setPropertyDesired,
} from '@/lib/api-generated/sdk.gen'
import type {
  SimplePaginatedResponsePropertyLatest as PropertyLatestPage,
  SimplePaginatedResponsePropertyHistory as PropertyHistoryPage,
  PaginatedResponsePropertyCommand as PropertyCommandPage,
  CreatePropertyCommandRequest,
  ShadowView,
  SetDesiredRequest,
} from '@/lib/api-generated/types.gen'

interface PropertyLatestParams {
  product_id: string
  device_id?: string | null
  page?: number
  page_size?: number
}

export function usePropertyLatest(params: PropertyLatestParams) {
  return useQuery({
    queryKey: ['property-latest', params],
    queryFn: async () => {
      const res = await getPropertyLatest({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as PropertyLatestPage
    },
  })
}

interface PropertyHistoryParams {
  product_id: string
  device_id?: string | null
  page?: number
  page_size?: number
}

export function usePropertyHistory(params: PropertyHistoryParams) {
  return useQuery({
    queryKey: ['property-history', params],
    queryFn: async () => {
      const res = await getPropertyHistory({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as PropertyHistoryPage
    },
  })
}

interface PropertyCommandsParams {
  product_id: string
  device_id?: string | null
  status?: null | import('@/lib/api-generated/types.gen').CommandStatus
  page?: number
  page_size?: number
}

export function usePropertyCommands(params: PropertyCommandsParams) {
  return useQuery({
    queryKey: ['property-commands', params],
    queryFn: async () => {
      const res = await getPropertyCommands({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          status: params.status ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as PropertyCommandPage
    },
  })
}

export function useCreatePropertyCommand() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreatePropertyCommandRequest) => {
      const res = await createPropertyCommand({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['property-commands'] })
    },
  })
}

export function useDeletePropertyCommands() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (ids: number[]) => {
      const res = await deletePropertyCommands({
        query: { ids: ids.join(',') as unknown as number[] },
        throwOnError: true,
      })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['property-commands'] })
    },
  })
}

interface PropertyShadowParams {
  product_id: string
  device_id: string
}

export const usePropertyShadow = (params: PropertyShadowParams) => {
  return useQuery({
    queryKey: ['property-shadow', params],
    queryFn: async () => {
      const res = await getPropertyShadow({
        query: {
          product_id: params.product_id,
          device_id: params.device_id,
        },
        throwOnError: true,
      })
      return res.data as unknown as ShadowView
    },
  })
}

export const useSetDesired = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: SetDesiredRequest) => {
      const res = await setPropertyDesired({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['property-shadow'] })
      queryClient.invalidateQueries({ queryKey: ['property-commands'] })
      queryClient.invalidateQueries({ queryKey: ['property-latest'] })
    },
  })
}
