import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type { DeviceStatus } from '@/lib/api-generated/types.gen'

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__devicesIndexComponent = options.component
      return { options }
    },
    Link: ({ to, params, children, ...props }: { to: string; params?: Record<string, string>; children: React.ReactNode; [k: string]: unknown }) => {
      // Resolve $id-style params in the URL, matching TanStack Router behavior
      let href = to
      if (params) {
        for (const [key, value] of Object.entries(params)) {
          href = href.replace(`$${key}`, value)
        }
      }
      return <a href={href} {...props}>{children}</a>
    },
    useNavigate: () => vi.fn(),
  }
})

// Mock hooks
const mockUseDevices = vi.fn()
vi.mock('@/hooks/useDevices', () => ({
  useDevices: (...args: unknown[]) => mockUseDevices(...args),
}))

const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

// Import the module to trigger createRoute and capture the component
import '../index'

const mockDevices: DeviceStatus[] = [
  {
    device_id: 'device-001',
    product_id: 'product-a',
    status: 'Online',
    ip_address: '192.168.1.10',
    last_online_at: '2025-01-01T10:00:00Z',
    last_offline_at: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T10:00:00Z',
  },
  {
    device_id: 'device-002',
    product_id: 'product-b',
    status: 'Offline',
    ip_address: null,
    last_online_at: '2025-01-02T08:00:00Z',
    last_offline_at: '2025-01-02T09:00:00Z',
    created_at: '2025-01-02T00:00:00Z',
    updated_at: '2025-01-02T09:00:00Z',
  },
]

function getDefaultMocks() {
  mockUseProducts.mockReturnValue({ data: [], isLoading: false })
  mockUseDevices.mockReturnValue({ data: { data: [], pagination: { page: 1, page_size: 10, total: 0 } }, isLoading: false })
}

describe('DevicesIndexPage', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesIndexComponent as React.ComponentType

  test('renders page title "Devices"', () => {
    getDefaultMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Devices')).toBeInTheDocument()
  })

  test('renders filter area with Product and Status selects', () => {
    getDefaultMocks()

    const { container } = renderWithProviders(<Page />)

    // SearchForm renders labels without htmlFor, so we check labels exist
    // "Product" and "Status" appear as label text; use getAllByText for "Status"
    // since it also appears as a table column header
    expect(screen.getByText('Product')).toBeInTheDocument()
    expect(screen.getAllByText('Status').length).toBeGreaterThanOrEqual(1)
    // And there are select elements for the filters
    const selects = container.querySelectorAll('select')
    expect(selects.length).toBe(2)
  })

  test('shows empty state when API returns no devices', () => {
    getDefaultMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('No devices found')).toBeInTheDocument()
  })

  test('renders device list with Device ID as clickable links', () => {
    mockUseProducts.mockReturnValue({ data: [], isLoading: false })
    mockUseDevices.mockReturnValue({ data: { data: mockDevices, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('device-001')).toBeInTheDocument()
    expect(screen.getByText('device-002')).toBeInTheDocument()

    // Device ID should be a link pointing to the detail page
    const link1 = screen.getByText('device-001').closest('a')
    expect(link1).toHaveAttribute('href', '/devices/show/device-001')

    const link2 = screen.getByText('device-002').closest('a')
    expect(link2).toHaveAttribute('href', '/devices/show/device-002')
  })

  test('renders pagination controls when paginated data returned', () => {
    mockUseProducts.mockReturnValue({ data: [], isLoading: false })
    mockUseDevices.mockReturnValue({
      data: {
        data: mockDevices,
        pagination: { page: 1, page_size: 10, total: 25 },
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText(/Page 1 of 3/)).toBeInTheDocument()
  })

  test('filter search triggers refetch with selected values', async () => {
    const user = userEvent.setup()
    getDefaultMocks()

    const { container } = renderWithProviders(<Page />)

    // Find the Status select (second select element)
    const selects = container.querySelectorAll('select')
    const statusSelect = selects[1] // Product is first, Status is second
    await user.selectOptions(statusSelect, 'Online')
    await user.click(screen.getByRole('button', { name: /search/i }))

    await waitFor(() => {
      expect(mockUseDevices).toHaveBeenLastCalledWith(
        expect.objectContaining({ status: 'Online' }),
      )
    })
  })
})
