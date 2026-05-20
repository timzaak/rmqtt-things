import { describe, test, expect, vi } from 'vitest'
import { screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type { DeviceStatusWithSource } from '@/lib/api-generated/types.gen'

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType; path?: string }) => {
      ;(globalThis as Record<string, unknown>).__devicesShowComponent = options.component
      // Return a route object with useParams that returns a fixed device id
      const routeObj = {
        options,
        useParams: () => ({ id: 'test-device-001' }),
      }
      return routeObj
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => vi.fn(),
  }
})

// Mock hooks
const mockUseDevices = vi.fn()
vi.mock('@/hooks/useDevices', () => ({
  useDevices: (...args: unknown[]) => mockUseDevices(...args),
  useDeviceStatusHistory: () => ({
    data: { data: [], pagination: undefined },
    isLoading: false,
  }),
}))

const mockUsePropertyLatest = vi.fn()
const mockUsePropertyHistory = vi.fn()
const mockUsePropertyCommands = vi.fn()
const mockUseCreatePropertyCommand = vi.fn()
const mockUseDeletePropertyCommands = vi.fn()

vi.mock('@/hooks/useProperties', () => ({
  usePropertyLatest: (...args: unknown[]) => mockUsePropertyLatest(...args),
  usePropertyHistory: (...args: unknown[]) => mockUsePropertyHistory(...args),
  usePropertyCommands: (...args: unknown[]) => mockUsePropertyCommands(...args),
  useCreatePropertyCommand: () => mockUseCreatePropertyCommand(),
  useDeletePropertyCommands: () => mockUseDeletePropertyCommands(),
}))

const mockUseEventHistory = vi.fn()
vi.mock('@/hooks/useEvents', () => ({
  useEventHistory: (...args: unknown[]) => mockUseEventHistory(...args),
}))

// Import the module to trigger createRoute and capture the component
import '../show.$id'

const mockDevice: DeviceStatusWithSource = {
  device_id: 'test-device-001',
  product_id: 'product-a',
  status: 'Online',
  ip_address: '192.168.1.10',
  last_online_at: '2025-01-01T10:00:00Z',
  last_offline_at: null,
  created_at: '2025-01-01T00:00:00Z',
  updated_at: '2025-01-01T10:00:00Z',
  registration_source: 'Manual',
}

function setupMocks(deviceData = mockDevice) {
  mockUseDevices.mockReturnValue({
    data: {
      data: [deviceData],
      pagination: { page: 1, page_size: 1, total: 1 },
    },
    isLoading: false,
  })
  mockUsePropertyLatest.mockReturnValue({ data: { data: [] }, isLoading: false })
  mockUsePropertyHistory.mockReturnValue({ data: { data: [], pagination: undefined }, isLoading: false })
  mockUsePropertyCommands.mockReturnValue({ data: { data: [], pagination: undefined }, isLoading: false })
  mockUseEventHistory.mockReturnValue({ data: { data: [], pagination: undefined }, isLoading: false })
  mockUseCreatePropertyCommand.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUseDeletePropertyCommands.mockReturnValue({ mutate: vi.fn(), isPending: false })
}

describe('DevicesShowPage', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('renders page title "Device Detail"', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Device Detail')).toBeInTheDocument()
  })

  test('renders back link to devices list', () => {
    setupMocks()

    renderWithProviders(<Page />)

    const backLink = screen.getByText(/Back to Devices/)
    expect(backLink).toBeInTheDocument()
    expect(backLink.closest('a')).toHaveAttribute('href', '/devices')
  })

  test('renders basic info card with device data', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Device Info')).toBeInTheDocument()
    expect(screen.getByText('test-device-001')).toBeInTheDocument()
    expect(screen.getByText('product-a')).toBeInTheDocument()
    expect(screen.getByText('192.168.1.10')).toBeInTheDocument()
  })

  test('renders section headings for all data areas', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Latest Properties')).toBeInTheDocument()
    expect(screen.getByText('Property History')).toBeInTheDocument()
    expect(screen.getByText('Event History')).toBeInTheDocument()
    expect(screen.getByText('Property Commands')).toBeInTheDocument()
    expect(screen.getByText('Connection History')).toBeInTheDocument()
  })

  test('renders property history table with mock data', () => {
    setupMocks()
    mockUsePropertyHistory.mockReturnValue({
      data: {
        data: [
          {
            id: 1,
            properties: { temperature: 25.5 },
            reported_time: '2025-01-01T10:00:00Z',
            created_time: '2025-01-01T10:00:00Z',
          },
        ],
        pagination: { page: 1, page_size: 10, total: 1 },
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText('1')).toBeInTheDocument()
    // Check that property data is rendered (inside a <pre> block)
    expect(screen.getByText(/temperature/)).toBeInTheDocument()
  })

  test('renders command history with Send Command button', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByRole('button', { name: /send command/i })).toBeInTheDocument()
  })

  test('opens command dialog when Send Command is clicked', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    await user.click(screen.getByRole('button', { name: /send command/i }))

    // Dialog heading distinguishes from the button text
    expect(screen.getByRole('heading', { name: /send command/i })).toBeInTheDocument()
    expect(screen.getByPlaceholderText('{"key": "value"}')).toBeInTheDocument()
  })

  test('submits command with valid JSON input', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    mockUseCreatePropertyCommand.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    // Open dialog
    await user.click(screen.getByRole('button', { name: /send command/i }))

    // Use fireEvent.change to set JSON value (avoids userEvent special char interpretation)
    const textarea = screen.getByPlaceholderText('{"key": "value"}')
    fireEvent.change(textarea, { target: { value: '{"action": "reboot"}' } })

    // Submit
    await user.click(screen.getByRole('button', { name: /^send$/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      {
        product_id: 'product-a',
        device_id: 'test-device-001',
        command: { action: 'reboot' },
      },
      expect.objectContaining({
        onSuccess: expect.any(Function),
      }),
    )
  })

  test('shows parse error for invalid JSON', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    // Open dialog
    await user.click(screen.getByRole('button', { name: /send command/i }))

    // Use fireEvent.change to set invalid JSON
    const textarea = screen.getByPlaceholderText('{"key": "value"}')
    fireEvent.change(textarea, { target: { value: 'not valid json' } })

    // Submit
    await user.click(screen.getByRole('button', { name: /^send$/i }))

    expect(screen.getByText('Invalid JSON')).toBeInTheDocument()
  })

  test('shows device not found when API returns empty', () => {
    mockUseDevices.mockReturnValue({
      data: { data: [], pagination: { page: 1, page_size: 1, total: 0 } },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText('Device not found.')).toBeInTheDocument()
  })

  test('shows loading state', () => {
    mockUseDevices.mockReturnValue({
      data: undefined,
      isLoading: true,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })
})
