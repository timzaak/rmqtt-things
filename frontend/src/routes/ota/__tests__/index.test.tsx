import { describe, test, expect, vi } from 'vitest'
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'

const mockNavigate = vi.fn()
vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__otaIndexComponent = options.component
      return { options }
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => mockNavigate,
  }
})

const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

const mockUseOtaVersions = vi.fn()
const mockUseDeleteOtaVersion = vi.fn()
vi.mock('@/hooks/useOta', () => ({
  useOtaVersions: (...args: unknown[]) => mockUseOtaVersions(...args),
  useDeleteOtaVersion: () => mockUseDeleteOtaVersion(),
}))

import '../index'

import { mockProducts } from '@/test/fixtures'

const mockOtaData = [
  {
    id: 1,
    product_id: 'SN-100',
    key: 'firmware-main',
    version: 102200, // 1.2.200
    min_version: 100000, // 1.0.0
    max_version: 200000,
    file_key: 'ota/firmware-main.bin',
    bin_length: 102400,
    bin_md5: 'abc123def456',
    status: 1,
    released_at: '2025-01-10T10:00:00Z',
    created_at: '2025-01-10T10:00:00Z',
    updated_at: '2025-01-10T10:00:00Z',
    device_ids: ['device-001'],
    log: 'Initial release',
  },
  {
    id: 2,
    product_id: 'AC-200',
    key: 'firmware-act',
    version: 30001,
    min_version: 10000,
    max_version: null,
    file_key: 'ota/firmware-act.bin',
    bin_length: 51200,
    bin_md5: 'def789abc012',
    status: 1,
    released_at: '2025-02-01T08:00:00Z',
    created_at: '2025-02-01T08:00:00Z',
    updated_at: '2025-02-01T08:00:00Z',
    device_ids: null,
    log: null,
  },
]

describe('OtaIndexPage', () => {
  const Page = (globalThis as Record<string, unknown>).__otaIndexComponent as React.ComponentType

  function setupMocks() {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersions.mockReturnValue({
      data: { data: mockOtaData, pagination: { page: 1, page_size: 10, total: 2 } },
      isLoading: false,
    })
    mockUseDeleteOtaVersion.mockReturnValue({ mutate: vi.fn(), isPending: false })
  }

  test('renders table with correct column headers', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('OTA Versions')).toBeInTheDocument()
    expect(screen.getByText('ID')).toBeInTheDocument()
    expect(screen.getByText('Key')).toBeInTheDocument()
    expect(screen.getByText('Version')).toBeInTheDocument()
    expect(screen.getByText('Min Version')).toBeInTheDocument()
    expect(screen.getByText('Max Version')).toBeInTheDocument()
    expect(screen.getByText('Bin Length')).toBeInTheDocument()
    expect(screen.getByText('Bin MD5')).toBeInTheDocument()
    expect(screen.getByText('Created At')).toBeInTheDocument()
    expect(screen.getByText('Actions')).toBeInTheDocument()
  })

  test('formats version numbers as x.y.z', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // version 102200 = 1.2.200
    expect(screen.getByText('1.2.200')).toBeInTheDocument()
    // min_version 100000 = 1.0.0
    expect(screen.getByText('1.0.0')).toBeInTheDocument()
    // max_version 200000 = 2.0.0
    expect(screen.getByText('2.0.0')).toBeInTheDocument()
    // version 30001 = 0.30.1
    expect(screen.getByText('0.30.1')).toBeInTheDocument()
    // min_version 10000 = 0.10.0
    expect(screen.getByText('0.10.0')).toBeInTheDocument()
  })

  test('shows product name instead of ID', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // Product names appear in both the search dropdown and the table
    const sensorAMatches = screen.getAllByText('Sensor A')
    expect(sensorAMatches.length).toBeGreaterThanOrEqual(1)
    const actuatorBMatches = screen.getAllByText('Actuator B')
    expect(actuatorBMatches.length).toBeGreaterThanOrEqual(1)
  })

  test('shows empty state when no data', () => {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersions.mockReturnValue({
      data: { data: [], pagination: { page: 1, page_size: 10, total: 0 } },
      isLoading: false,
    })
    mockUseDeleteOtaVersion.mockReturnValue({ mutate: vi.fn(), isPending: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('No OTA versions found')).toBeInTheDocument()
  })

  test('product filter dropdown is rendered', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // SearchForm renders a select for product filter
    const productSelect = screen.getByRole('combobox')
    expect(productSelect).toBeInTheDocument()
    // The "All" default option
    expect(screen.getByText('All')).toBeInTheDocument()
    // Product options exist in the select dropdown
    const sensorOptions = screen.getAllByText('Sensor A')
    expect(sensorOptions.length).toBeGreaterThanOrEqual(1)
    const actuatorOptions = screen.getAllByText('Actuator B')
    expect(actuatorOptions.length).toBeGreaterThanOrEqual(1)
  })

  test('delete button triggers confirmation dialog', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    // There are two Delete buttons (one per row), click the first one
    const deleteButtons = screen.getAllByRole('button', { name: /delete/i })
    await user.click(deleteButtons[0])

    // ConfirmDialog should appear
    expect(screen.getByText('Delete OTA Version')).toBeInTheDocument()
    expect(screen.getByText(/are you sure you want to delete/i)).toBeInTheDocument()
    // Dialog has a confirm button with "Delete" text
    const confirmButtons = screen.getAllByRole('button', { name: /delete/i })
    // At least the dialog confirm button should be present
    expect(confirmButtons.length).toBeGreaterThan(deleteButtons.length)
  })

  test('renders Create OTA Version link', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Create OTA Version')).toBeInTheDocument()
  })
})
