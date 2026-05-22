import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listAlarmRules,
  createAlarmRule,
  getAlarmRule,
  updateAlarmRule,
  updateAlarmRuleStatus,
  deleteAlarmRule,
} from '@/lib/api-generated/sdk.gen'
import type {
  PaginatedResponseAlarmRule,
  AlarmRuleResponse,
  CreateAlarmRuleRequest,
  UpdateAlarmRuleRequest,
} from '@/lib/api-generated/types.gen'

interface AlarmRulesParams {
  product_id?: string | null
  enabled?: boolean | null
  page?: number
  page_size?: number
}

export function useAlarmRules(params: AlarmRulesParams) {
  return useQuery({
    queryKey: ['alarm-rules', params],
    queryFn: async () => {
      const res = await listAlarmRules({
        query: {
          product_id: params.product_id ?? undefined,
          enabled: params.enabled ?? undefined,
          page: params.page,
          page_size: params.page_size,
        },
        throwOnError: true,
      })
      return res.data as unknown as PaginatedResponseAlarmRule
    },
  })
}

export function useAlarmRule(id: number) {
  return useQuery({
    queryKey: ['alarm-rules', id],
    queryFn: async () => {
      const res = await getAlarmRule({ path: { id }, throwOnError: true })
      return (res.data as unknown as AlarmRuleResponse).data
    },
    enabled: !!id,
  })
}

export function useCreateAlarmRule() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateAlarmRuleRequest) => {
      const res = await createAlarmRule({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alarm-rules'] })
    },
  })
}

export function useUpdateAlarmRule() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, ...body }: UpdateAlarmRuleRequest & { id: number }) => {
      const res = await updateAlarmRule({ path: { id }, body, throwOnError: true })
      return res.data
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['alarm-rules'] })
      queryClient.invalidateQueries({ queryKey: ['alarm-rules', variables.id] })
    },
  })
}

export function useUpdateAlarmRuleStatus() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, enabled }: { id: number; enabled: boolean }) => {
      const res = await updateAlarmRuleStatus({
        path: { id },
        body: { enabled },
        throwOnError: true,
      })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alarm-rules'] })
    },
  })
}

export function useDeleteAlarmRule() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (id: number) => {
      const res = await deleteAlarmRule({ path: { id }, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alarm-rules'] })
    },
  })
}
