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
            className="mb-1 block text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            Name <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            id="name"
            type="text"
            required
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            className="w-full rounded-md px-3 py-2 text-sm"
            style={{
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-1)',
              color: 'var(--color-text-primary)',
              borderRadius: '8px',
              fontSize: '13px',
            }}
          />
        </div>
        <div>
          <label
            htmlFor="model_no"
            className="mb-1 block text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            Model Number <span style={{ color: '#dc2626' }}>*</span>
          </label>
          <input
            id="model_no"
            type="text"
            required
            value={form.model_no}
            onChange={(e) => setForm((f) => ({ ...f, model_no: e.target.value }))}
            className="w-full rounded-md px-3 py-2 text-sm"
            style={{
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-1)',
              color: 'var(--color-text-primary)',
              borderRadius: '8px',
              fontSize: '13px',
            }}
          />
        </div>
        <div>
          <label
            htmlFor="description"
            className="mb-1 block text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            Description
          </label>
          <textarea
            id="description"
            value={form.description}
            onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
            rows={3}
            className="w-full rounded-md px-3 py-2 text-sm"
            style={{
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-1)',
              color: 'var(--color-text-primary)',
              borderRadius: '8px',
              fontSize: '13px',
            }}
          />
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={createProduct.isPending}
            className="rounded-md px-4 py-2 text-sm font-medium"
            style={{ background: 'var(--color-text-primary)', color: 'var(--color-surface-1)' }}
          >
            {createProduct.isPending ? 'Creating...' : 'Create'}
          </button>
          <Link
            to="/products"
            className="rounded-md px-4 py-2 text-sm font-medium"
            style={{
              border: '1px solid var(--color-border)',
              color: 'var(--color-text-secondary)',
              background: 'transparent',
            }}
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
