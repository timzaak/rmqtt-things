import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { listProducts, createProduct, getProduct, updateProduct } from '@/lib/api-generated/sdk.gen'
import type {
  Product,
  CreateProductRequest,
  UpdateProductRequest,
  PaginatedResponseProduct,
} from '@/lib/api-generated/types.gen'

export function useProducts(search?: string | null, page?: number, page_size?: number) {
  return useQuery({
    queryKey: ['products', search, page, page_size],
    queryFn: async () => {
      const res = await listProducts({
        query: {
          search: search ?? undefined,
          page: page ?? 1,
          page_size: page_size ?? 10,
        },
        throwOnError: true,
      })
      return res.data as PaginatedResponseProduct
    },
  })
}

export function useProduct(id: number) {
  return useQuery({
    queryKey: ['products', id],
    queryFn: async () => {
      const res = await getProduct({ path: { id }, throwOnError: true })
      return res.data as Product
    },
    enabled: !!id,
  })
}

export function useCreateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateProductRequest) => {
      const res = await createProduct({ body, throwOnError: true })
      return res.data as Product
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['products'] })
    },
  })
}

export function useUpdateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, ...body }: UpdateProductRequest & { id: number }) => {
      const res = await updateProduct({ path: { id }, body, throwOnError: true })
      return res.data as Product
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['products'] })
      queryClient.invalidateQueries({ queryKey: ['products', variables.id] })
    },
  })
}
