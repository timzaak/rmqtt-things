import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'

const mockNavigate = vi.fn()
vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__otaEditComponent = options.component
      return {
        options,
        useParams: () => ({ id: '1' }),
      }
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => mockNavigate,
    useBlocker: () => ({ status: 'idle' }),
  }
})

const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

const mockUseOtaVersion = vi.fn()
const mockUseUpdateOtaVersion = vi.fn()
vi.mock('@/hooks/useOta', () => ({
  useOtaVersion: (...args: unknown[]) => mockUseOtaVersion(...args),
  useUpdateOtaVersion: () => mockUseUpdateOtaVersion(),
}))

import '../edit.$id'

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

describe('OtaEditPage', () => {
  const Page = (globalThis as Record<string, unknown>).__otaEditComponent as React.ComponentType

  function setupMocks() {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: mockOtaVersion, isLoading: false })
    mockUseUpdateOtaVersion.mockReturnValue({ mutate: vi.fn(), isPending: false })
  }

  test('shows loading state', () => {
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: undefined, isLoading: true })
    mockUseUpdateOtaVersion.mockReturnValue({ mutate: vi.fn(), isPending: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  test('loads existing data and fills form', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit OTA Version')).toBeInTheDocument()
    })

    // Product field shows name (from otaVersion directly, not form state)
    await waitFor(() => {
      expect(screen.getByDisplayValue('Sensor A')).toBeInTheDocument()
    })
    // Key field shows value (from otaVersion directly)
    await waitFor(() => {
      expect(screen.getByDisplayValue('firmware-main')).toBeInTheDocument()
    })
    // Version field shows formatted version (from otaVersion directly)
    await waitFor(() => {
      expect(screen.getByDisplayValue('1.2.200')).toBeInTheDocument()
    })
    // Min version pre-filled (from form state)
    await waitFor(() => {
      expect(screen.getByDisplayValue('1.0.0')).toBeInTheDocument()
    })
    // Max version pre-filled (from form state)
    await waitFor(() => {
      expect(screen.getByDisplayValue('2.0.0')).toBeInTheDocument()
    })
    // Log pre-filled (from form state)
    await waitFor(() => {
      expect(screen.getByDisplayValue('Initial release')).toBeInTheDocument()
    })
  })

  test('Product, Key, and Version fields are disabled', async () => {
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit OTA Version')).toBeInTheDocument()
    })

    // The disabled fields use <input disabled>, find them by value
    const productInput = await screen.findByDisplayValue('Sensor A') as HTMLInputElement
    expect(productInput.disabled).toBe(true)

    const keyInput = await screen.findByDisplayValue('firmware-main') as HTMLInputElement
    expect(keyInput.disabled).toBe(true)

    const versionInput = await screen.findByDisplayValue('1.2.200') as HTMLInputElement
    expect(versionInput.disabled).toBe(true)
  })

  test('editable fields can be modified and submitted', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    mockUseProducts.mockReturnValue({ data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 2 } }, isLoading: false })
    mockUseOtaVersion.mockReturnValue({ data: mockOtaVersion, isLoading: false })
    mockUseUpdateOtaVersion.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit OTA Version')).toBeInTheDocument()
    })

    // Wait for form state to be initialized from otaVersion data
    await waitFor(() => {
      expect(screen.getByDisplayValue('1.0.0')).toBeInTheDocument()
    })

    // Clear and type a new min_version
    const minVersionInput = screen.getByLabelText(/min version/i)
    await user.clear(minVersionInput)
    await user.type(minVersionInput, '2.0.0')

    // Submit the form
    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 1,
        min_version: '200000',
      }),
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      }),
    )
  })

  test('device IDs are loaded and can be removed', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit OTA Version')).toBeInTheDocument()
    })

    // Device IDs from the loaded data are shown (need to wait for form state)
    await waitFor(() => {
      expect(screen.getByText('device-001')).toBeInTheDocument()
      expect(screen.getByText('device-002')).toBeInTheDocument()
    })

    // Remove one device ID
    const removeButtons = screen.getAllByText('x')
    await user.click(removeButtons[0])

    // One device ID should be removed
    await waitFor(() => {
      expect(screen.queryByText('device-001')).not.toBeInTheDocument()
    })
    expect(screen.getByText('device-002')).toBeInTheDocument()
  })
})
