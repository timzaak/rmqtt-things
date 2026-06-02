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

  const [form, setForm] = useState({ name: '', description: '', auto_provisioning: false })
  const [prevProduct, setPrevProduct] = useState<typeof product>(undefined)

  if (product && product !== prevProduct) {
    setPrevProduct(product)
    setForm({
      name: product.name,
      description: product.description ?? '',
      auto_provisioning: product.auto_provisioning,
    })
  }

  const isDirty =
    prevProduct !== undefined &&
    product !== undefined &&
    (form.name !== product.name ||
      form.description !== (product.description ?? '') ||
      form.auto_provisioning !== product.auto_provisioning)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    updateProduct.mutate(
      {
        id,
        name: form.name,
        description: form.description,
        auto_provisioning: form.auto_provisioning,
      },
      {
        onSuccess: () => {
          flushSync(() => setPrevProduct(undefined))
          navigate({ to: '/products' })
        },
        onError: (error) => {
          toast.error('Failed to update product', { description: error.message })
        },
      }
    )
  }

  if (isLoading) {
    return (
      <div className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Loading...
      </div>
    )
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit Product" />
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
            Model Number
          </label>
          <input
            id="model_no"
            type="text"
            disabled
            value={product?.model_no ?? ''}
            className="w-full rounded-md px-3 py-2 text-sm"
            style={{
              border: '1px solid var(--color-border)',
              background: 'var(--color-surface-2)',
              color: 'var(--color-text-muted)',
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
        <div>
          <label
            className="mb-1 block text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            Auto Provisioning
          </label>
          <label className="inline-flex items-center gap-2">
            <input
              type="checkbox"
              checked={form.auto_provisioning}
              onChange={(e) => setForm((f) => ({ ...f, auto_provisioning: e.target.checked }))}
              className="h-4 w-4 rounded"
              style={{ borderColor: 'var(--color-border)', color: 'var(--color-text-primary)' }}
            />
            <span className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
              Enable device auto-provisioning for this product
            </span>
          </label>
        </div>
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={updateProduct.isPending}
            className="rounded-md px-4 py-2 text-sm font-medium"
            style={{ background: 'var(--color-text-primary)', color: 'var(--color-surface-1)' }}
          >
            {updateProduct.isPending ? 'Saving...' : 'Save'}
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
