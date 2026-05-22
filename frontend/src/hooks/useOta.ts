import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getOtaVersions,
  createOtaVersion,
  getOtaVersion,
  updateOtaVersion,
  deleteOtaVersion,
} from '@/lib/api-generated/sdk.gen'
import type {
  PaginatedResponseOtaVersion as OtaVersionPage,
  OtaVersion,
  CreateOtaVersionRequest,
  UpdateOtaVersionRequest,
} from '@/lib/api-generated/types.gen'

interface OtaVersionsParams {
  product_id?: string | null
  page?: number
  page_size?: number
}

export function useOtaVersions(params: OtaVersionsParams) {
  return useQuery({
    queryKey: ['ota-versions', params],
    queryFn: async () => {
      const res = await getOtaVersions({
        query: {
          product_id: params.product_id ?? undefined,
          page: params.page ?? 1,
          page_size: params.page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as unknown as OtaVersionPage
    },
  })
}

export function useOtaVersion(id: number) {
  return useQuery({
    queryKey: ['ota-versions', id],
    queryFn: async () => {
      const res = await getOtaVersion({ path: { id }, throwOnError: true })
      return res.data as OtaVersion
    },
    enabled: !!id,
  })
}

export function useCreateOtaVersion() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateOtaVersionRequest) => {
      const res = await createOtaVersion({ body, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ota-versions'] })
    },
  })
}

export function useUpdateOtaVersion() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, ...body }: UpdateOtaVersionRequest & { id: number }) => {
      const res = await updateOtaVersion({ path: { id }, body, throwOnError: true })
      return res.data
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['ota-versions'] })
      queryClient.invalidateQueries({ queryKey: ['ota-versions', variables.id] })
    },
  })
}

export function useDeleteOtaVersion() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (id: number) => {
      const res = await deleteOtaVersion({ path: { id }, throwOnError: true })
      return res.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ota-versions'] })
    },
  })
}
