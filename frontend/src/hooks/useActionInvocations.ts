import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createServiceCommand,
  deleteServiceCommands,
  getServiceCommands,
} from '@/lib/api-generated/sdk.gen'
import type {
  CreateActionCommandRequest,
  PaginatedResponseActionInvocationView,
} from '@/lib/api-generated/types.gen'

interface ActionInvocationsParams {
  product_id: string
  device_id?: string | null
  page?: number
  page_size?: number
}

export function useActionInvocations(params: ActionInvocationsParams) {
  return useQuery({
    queryKey: ['action-invocations', params],
    queryFn: async () => {
      const res = await getServiceCommands({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as PaginatedResponseActionInvocationView
    },
  })
}

export function useCreateActionInvocation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateActionCommandRequest) => {
      const res = await createServiceCommand({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['action-invocations'] })
    },
  })
}

export function useDeleteActionInvocations() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (ids: number[]) => {
      const res = await deleteServiceCommands({
        query: { ids: ids.join(',') as unknown as number[] },
        throwOnError: true,
      })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['action-invocations'] })
    },
  })
}
