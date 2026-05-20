import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      // Store component on globalThis so it survives hoisting boundaries
      ;(globalThis as Record<string, unknown>).__productsIndexComponent = options.component
      return { options }
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => vi.fn(),
  }
})

// Mock useProducts hook
const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

// Import the module to trigger createRoute and capture the component
import '../index'

import { mockProducts } from '@/test/fixtures'

describe('ProductsIndexPage', () => {
  const Page = (globalThis as Record<string, unknown>).__productsIndexComponent as React.ComponentType

  test('renders product table when data loaded', () => {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('Products')).toBeInTheDocument()
    expect(screen.getByText('Sensor A')).toBeInTheDocument()
    expect(screen.getByText('SN-100')).toBeInTheDocument()
    expect(screen.getByText('Actuator B')).toBeInTheDocument()
    expect(screen.getByText('AC-200')).toBeInTheDocument()
    expect(screen.getByText('Create Product')).toBeInTheDocument()
  })

  test('shows empty state when no products', () => {
    mockUseProducts.mockReturnValue({ data: { data: [], pagination: { page: 1, page_size: 10, total: 0 } }, isLoading: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('No products found')).toBeInTheDocument()
  })

  test('search form triggers refetch with search term', async () => {
    const user = userEvent.setup()
    mockUseProducts.mockReturnValue({ data: { data: [], pagination: { page: 1, page_size: 10, total: 0 } }, isLoading: false })

    renderWithProviders(<Page />)

    const searchInput = screen.getByPlaceholderText('Name or Model Number')
    await user.type(searchInput, 'sensor')
    await user.click(screen.getByRole('button', { name: /search/i }))

    await waitFor(() => {
      expect(mockUseProducts).toHaveBeenLastCalledWith('sensor', 1, 10)
    })
  })
})
