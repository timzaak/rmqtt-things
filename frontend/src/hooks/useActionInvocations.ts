import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createServiceCommand, deleteServiceCommands } from '@/lib/api-generated/sdk.gen'
import type { CreateActionCommandRequest } from '@/lib/api-generated/types.gen'

export function useCreateActionInvocation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateActionCommandRequest) => {
      const res = await createServiceCommand({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['action-invocations'] })
      queryClient.invalidateQueries({ queryKey: ['device-operations'] })
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
      queryClient.invalidateQueries({ queryKey: ['device-operations'] })
    },
  })
}
