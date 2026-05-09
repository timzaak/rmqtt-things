import { flushSync } from 'react-dom'
import { useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useProduct, useUpdateProduct } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const productsEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/products/edit/$id',
  component: ProductsEditPage,
})

export const Route = productsEditRoute

function ProductsEditPage() {
  const { id: idStr } = productsEditRoute.useParams()
  const id = Number(idStr)
  const navigate = useNavigate()
  const { data: product, isLoading } = useProduct(id)
  const updateProduct = useUpdateProduct()

  const [form, setForm] = useState({ name: '', description: '' })
  const [prevProduct, setPrevProduct] = useState<typeof product>(undefined)

  if (product && product !== prevProduct) {
    setPrevProduct(product)
    setForm({ name: product.name, description: product.description ?? '' })
  }

  const isDirty =
    prevProduct !== undefined &&
    product !== undefined &&
    (form.name !== product.name || form.description !== (product.description ?? ''))

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    updateProduct.mutate(
      { id, name: form.name, description: form.description },
      {
        onSuccess: () => {
          flushSync(() => setPrevProduct(undefined))
          navigate({ to: '/products' })
        },
        onError: (error) => {
          toast.error('Failed to update product', { description: error.message })
        },
      },
    )
  }

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit Product" />
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        <div>
          <label htmlFor="name" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
            Name <span className="text-red-500">*</span>
          </label>
          <input
            id="name"
            type="text"
            required
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
          />
        </div>
        <div>
          <label htmlFor="model_no" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
            Model Number
          </label>
          <input
            id="model_no"
            type="text"
            disabled
            value={product?.model_no ?? ''}
            className="w-full rounded-md border border-slate-300 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-400"
          />
        </div>
        <div>
          <label htmlFor="description" className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300">
            Description
          </label>
          <textarea
            id="description"
            value={form.description}
            onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
            rows={3}
            className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
          />
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={updateProduct.isPending}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            {updateProduct.isPending ? 'Saving...' : 'Save'}
          </button>
          <Link
            to="/products"
            className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
