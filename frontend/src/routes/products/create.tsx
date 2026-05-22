import { flushSync } from 'react-dom'
import { useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useCreateProduct } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'

export const productsCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/products/create',
  component: ProductsCreatePage,
})

export const Route = productsCreateRoute

const initialForm = { name: '', model_no: '', description: '' }

function ProductsCreatePage() {
  const navigate = useNavigate()
  const createProduct = useCreateProduct()
  const [form, setForm] = useState(initialForm)

  const isDirty = Object.values(form).some((v) => v !== '')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    createProduct.mutate(
      { name: form.name, model_no: form.model_no, description: form.description || null },
      {
        onSuccess: () => {
          flushSync(() => setForm(initialForm))
          navigate({ to: '/products' })
        },
        onError: (error) => {
          toast.error('Failed to create product', { description: error.message })
        },
      }
    )
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Create Product" />
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        <div>
          <label
            htmlFor="name"
            className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
          >
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
          <label
            htmlFor="model_no"
            className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
          >
            Model Number <span className="text-red-500">*</span>
          </label>
          <input
            id="model_no"
            type="text"
            required
            value={form.model_no}
            onChange={(e) => setForm((f) => ({ ...f, model_no: e.target.value }))}
            className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
          />
        </div>
        <div>
          <label
            htmlFor="description"
            className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300"
          >
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
            disabled={createProduct.isPending}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            {createProduct.isPending ? 'Creating...' : 'Create'}
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
