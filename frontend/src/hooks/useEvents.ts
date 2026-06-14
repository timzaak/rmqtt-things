import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getEventHistory,
  getEventValidTemplates,
  createEventValidTemplate,
  getEventValidTemplate,
  updateEventValidTemplate,
  updateEventValidTemplateStatus,
  deleteEventValidTemplate,
} from '@/lib/api-generated/sdk.gen'
import type {
  SimplePaginatedResponseEventHistory as EventHistoryPage,
  PaginatedResponseEventValidTemplate as ValidTemplatePage,
  EventValidTemplate,
  CreateEventValidTemplateRequest,
  UpdateEventValidTemplateRequest,
  UpdateEventValidTemplateStatusRequest,
} from '@/lib/api-generated/types.gen'

interface EventHistoryParams {
  product_id: string
  device_id?: string | null
  page?: number
  page_size?: number
}

export function useEventHistory(params: EventHistoryParams) {
  return useQuery({
    queryKey: ['event-history', params],
    queryFn: async () => {
      const res = await getEventHistory({
        query: {
          product_id: params.product_id,
          device_id: params.device_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as EventHistoryPage
    },
  })
}

interface EventValidTemplatesParams {
  product_id?: string | null
  event?: string | null
  page?: number
  page_size?: number
}

export function useEventValidTemplates(params: EventValidTemplatesParams) {
  return useQuery({
    queryKey: ['valid-templates', params],
    queryFn: async () => {
      const res = await getEventValidTemplates({
        query: {
          product_id: params.product_id ?? undefined,
          event: params.event ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as ValidTemplatePage
    },
  })
}

export function useEventValidTemplate(id: number) {
  return useQuery({
    queryKey: ['valid-templates', id],
    queryFn: async () => {
      const res = await getEventValidTemplate({ path: { id }, throwOnError: true })
      return res.data as EventValidTemplate
    },
    enabled: !!id,
  })
}

export function useCreateEventValidTemplate() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateEventValidTemplateRequest) => {
      const res = await createEventValidTemplate({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['valid-templates'] })
    },
  })
}

export function useUpdateEventValidTemplate() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, ...body }: UpdateEventValidTemplateRequest & { id: number }) => {
      const res = await updateEventValidTemplate({ path: { id }, body, throwOnError: true })
      return res.data
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['valid-templates'] })
      queryClient.invalidateQueries({ queryKey: ['valid-templates', variables.id] })
    },
  })
}

export function useUpdateEventValidTemplateStatus() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, ...body }: UpdateEventValidTemplateStatusRequest & { id: number }) => {
      const res = await updateEventValidTemplateStatus({ path: { id }, body, throwOnError: true })
      return res.data
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['valid-templates'] })
      queryClient.invalidateQueries({ queryKey: ['valid-templates', variables.id] })
    },
  })
}

export function useDeleteEventValidTemplate() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (id: number) => {
      const res = await deleteEventValidTemplate({ path: { id }, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['valid-templates'] })
    },
  })
}
