import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getPropertyLatest,
  getPropertyHistory,
  getPropertyHistoryKeys,
  getPropertyHistorySeries,
  createPropertyCommand,
  deletePropertyCommands,
  getPropertyShadow,
  setPropertyDesired,
} from '@/lib/api-generated/sdk.gen'
import type {
  SimplePaginatedResponsePropertyLatest as PropertyLatestPage,
  SimplePaginatedResponsePropertyHistory as PropertyHistoryPage,
  PropertyChartKeysResponse,
  PropertySeriesListResponse,
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

export function usePropertyHistory(params: PropertyHistoryParams, options?: { enabled?: boolean }) {
  return useQuery({
    queryKey: ['property-history', params],
    enabled: options?.enabled ?? true,
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

export function useCreatePropertyCommand() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreatePropertyCommandRequest) => {
      const res = await createPropertyCommand({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['property-commands'] })
      queryClient.invalidateQueries({ queryKey: ['device-operations'] })
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
      queryClient.invalidateQueries({ queryKey: ['device-operations'] })
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
      queryClient.invalidateQueries({ queryKey: ['device-operations'] })
    },
  })
}

interface PropertyChartKeysParams {
  product_id: string
  device_id: string
}

// Numeric property discovery for the chart view. lookback_days is left to the
// contract default (30 days) so the selector always matches the longest preset
// time range.
export function usePropertyHistoryKeys(params: PropertyChartKeysParams) {
  return useQuery({
    queryKey: ['property-history-keys', params],
    queryFn: async () => {
      const res = await getPropertyHistoryKeys({
        query: {
          product_id: params.product_id,
          device_id: params.device_id,
        },
        throwOnError: true,
      })
      return res.data as unknown as PropertyChartKeysResponse
    },
  })
}

export interface PropertySeriesParams {
  product_id: string
  device_id: string
  keys: string[]
  start_time: string
  end_time: string
}

export function usePropertyHistorySeries(
  params: PropertySeriesParams,
  options?: { enabled?: boolean }
) {
  return useQuery({
    queryKey: ['property-history-series', params],
    enabled: options?.enabled ?? true,
    queryFn: async () => {
      const res = await getPropertyHistorySeries({
        query: {
          product_id: params.product_id,
          device_id: params.device_id,
          keys: params.keys,
          start_time: params.start_time,
          end_time: params.end_time,
        },
        throwOnError: true,
      })
      return res.data as unknown as PropertySeriesListResponse
    },
  })
}
