import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

const mockNavigate = vi.fn()
vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__otaShowComponent = options.component
      return {
        options,
        useParams: () => ({ id: '1' }),
      }
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

const mockUseOtaVersion = vi.fn()
vi.mock('@/hooks/useOta', () => ({
  useOtaVersion: (...args: unknown[]) => mockUseOtaVersion(...args),
}))

import '../show.$id'

import type { Product, OtaVersion } from '@/lib/api-generated/types.gen'

const mockProducts: Product[] = [
  { id: 1, name: 'Sensor A', model_no: 'SN-100', description: 'Temperature sensor', status: 'Online', created_at: '2025-01-01T00:00:00Z', updated_at: '2025-01-02T00:00:00Z' },
  { id: 2, name: 'Actuator B', model_no: 'AC-200', description: null, status: 'Offline', created_at: '2025-01-03T00:00:00Z', updated_at: '2025-01-04T00:00:00Z' },
]

const mockOtaVersion = {
  id: 1,
  product_id: 'SN-100',
  key: 'firmware-main',
  version: 102200, // 1.2.200
  min_version: 100000, // 1.0.0
  max_version: 200000, // 2.0.0
  file_key: 'ota/firmware-main.bin',
  bin_length: 102400,
  bin_md5: 'abc123def456',
  status: 1,
  released_at: '2025-01-10T10:00:00Z',
  created_at: '2025-01-10T10:00:00Z',
  updated_at: '2025-01-10T10:00:00Z',
  device_ids: ['device-001', 'device-002'],
  log: 'Initial release',
} as OtaVersion & { bin_length: number; bin_md5: string }

describe('OtaShowPage', () => {
  const Page = (globalThis as Record<string, unknown>).__otaShowComponent as React.ComponentType

  function setupMocks() {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: mockOtaVersion, isLoading: false })
  }

  test('shows loading state', () => {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: undefined, isLoading: true })

    renderWithProviders(<Page />)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  test('displays all fields as readonly', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('OTA Version Detail')).toBeInTheDocument()
    })

    // Key labels
    expect(screen.getByText('Key')).toBeInTheDocument()
    expect(screen.getByText('File Key')).toBeInTheDocument()
    expect(screen.getByText('Log')).toBeInTheDocument()
    expect(screen.getByText('Bin Length')).toBeInTheDocument()
    expect(screen.getByText('Bin MD5')).toBeInTheDocument()
    expect(screen.getByText('Status')).toBeInTheDocument()

    // Values
    expect(screen.getByText('firmware-main')).toBeInTheDocument()
    expect(screen.getByText('ota/firmware-main.bin')).toBeInTheDocument()
    expect(screen.getByText('Initial release')).toBeInTheDocument()
    expect(screen.getByText('abc123def456')).toBeInTheDocument()
  })

  test('formats version numbers as x.y.z', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('OTA Version Detail')).toBeInTheDocument()
    })

    // version 102200 = 1.2.200
    expect(screen.getByText('1.2.200')).toBeInTheDocument()
    // min_version 100000 = 1.0.0
    expect(screen.getByText('1.0.0')).toBeInTheDocument()
    // max_version 200000 = 2.0.0
    expect(screen.getByText('2.0.0')).toBeInTheDocument()
  })

  test('displays device IDs as tags', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('OTA Version Detail')).toBeInTheDocument()
    })

    expect(screen.getByText('device-001')).toBeInTheDocument()
    expect(screen.getByText('device-002')).toBeInTheDocument()
  })

  test('displays product name instead of ID', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('OTA Version Detail')).toBeInTheDocument()
    })

    // Product name is shown, not the ID
    expect(screen.getByText('Sensor A')).toBeInTheDocument()
  })

  test('renders Back to List link', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Back to List')).toBeInTheDocument()
    })
  })

  test('shows not found when record is null', () => {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: undefined, isLoading: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('OTA version not found.')).toBeInTheDocument()
  })
})
