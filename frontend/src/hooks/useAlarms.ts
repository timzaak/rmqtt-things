import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { listAlarms, ackAlarm } from '@/lib/api-generated/sdk.gen'

interface AlarmsParams {
  product_id?: string | null
  device_id?: string | null
  level?: string | null
  acknowledged?: boolean | null
  page?: number
  page_size?: number
}

export function useAlarms(params: AlarmsParams) {
  return useQuery({
    queryKey: ['alarms', params],
    queryFn: async () => {
      const res = await listAlarms({
        query: {
          product_id: params.product_id ?? undefined,
          device_id: params.device_id ?? undefined,
          level: params.level ?? undefined,
          acknowledged: params.acknowledged ?? undefined,
          page: params.page,
          page_size: params.page_size,
        },
        throwOnError: true,
      })
      return res.data
    },
  })
}

export function useAckAlarm() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (id: number) => {
      const res = await ackAlarm({ path: { id }, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alarms'] })
    },
  })
}
