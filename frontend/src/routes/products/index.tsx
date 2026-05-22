import { useState } from 'react'
import { createRoute, Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { rootRoute } from '../__root'
import { useProducts } from '@/hooks/useProducts'
import { DataTable, type Column } from '@/components/ui/data-table'
import { SearchForm } from '@/components/ui/search-form'
import { PageHeader } from '@/components/ui/page-header'
import type { Product } from '@/lib/api-generated/types.gen'
import { formatDatetime } from '@/lib/utils'

export const productsIndexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/products',
  component: ProductsIndexPage,
})

export const Route = productsIndexRoute

const columns: Column<Product>[] = [
  { header: 'ID', accessor: 'id' },
  { header: 'Name', accessor: 'name' },
  { header: 'Model Number', accessor: 'model_no' },
  { header: 'Description', accessor: (row) => row.description ?? '-' },
  { header: 'Status', accessor: 'status' },
  { header: 'Created At', accessor: (row) => formatDatetime(row.created_at) },
  { header: 'Updated At', accessor: (row) => formatDatetime(row.updated_at) },
  {
    header: 'Actions',
    accessor: (row) => (
      <Link
        to="/products/edit/$id"
        params={{ id: String(row.id) }}
        className="text-sm text-blue-600 hover:underline dark:text-blue-400"
      >
        Edit
      </Link>
    ),
  },
]

function ProductsIndexPage() {
  const [search, setSearch] = useState<string>('')
  const [page, setPage] = useState(1)

  const { data, isLoading } = useProducts(search || null, page, 10)

  const products = data?.data ?? []
  const pagination = data?.pagination

  return (
    <div>
      <PageHeader
        title="Products"
        actions={
          <Link
            to="/products/create"
            className="inline-flex h-9 items-center gap-1.5 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
          >
            <Plus className="h-4 w-4" />
            Create Product
          </Link>
        }
      />
      <SearchForm
        fields={[{ name: 'search', label: 'Search', placeholder: 'Name or Model Number' }]}
        onSearch={(values) => {
          setSearch(values.search)
          setPage(1)
        }}
      />
      <DataTable
        columns={columns}
        data={products}
        loading={isLoading}
        emptyMessage="No products found"
        pagination={
          pagination
            ? { page: pagination.page, pageSize: pagination.page_size, total: pagination.total }
            : undefined
        }
        onPageChange={setPage}
      />
    </div>
  )
}
