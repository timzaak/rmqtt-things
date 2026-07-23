import { describe, test, it, expect, vi } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type {
  DeviceStatusWithSource,
  ShadowView,
  FactoryDeviceView,
  FactoryComponentView,
  FactoryMetadataChangeLog,
} from '@/lib/api-generated/types.gen'

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
    Link: ({
      to,
      children,
      ...props
    }: {
      to: string
      children: React.ReactNode
      [k: string]: unknown
    }) => (
      <a href={to} {...props}>
        {children}
      </a>
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
const mockUsePropertyShadow = vi.fn()
const mockUseSetDesired = vi.fn()

vi.mock('@/hooks/useProperties', () => ({
  usePropertyLatest: (...args: unknown[]) => mockUsePropertyLatest(...args),
  usePropertyHistory: (...args: unknown[]) => mockUsePropertyHistory(...args),
  usePropertyCommands: (...args: unknown[]) => mockUsePropertyCommands(...args),
  useCreatePropertyCommand: () => mockUseCreatePropertyCommand(),
  useDeletePropertyCommands: () => mockUseDeletePropertyCommands(),
  usePropertyShadow: (...args: unknown[]) => mockUsePropertyShadow(...args),
  useSetDesired: () => mockUseSetDesired(),
}))

const mockUseEventHistory = vi.fn()
vi.mock('@/hooks/useEvents', () => ({
  useEventHistory: (...args: unknown[]) => mockUseEventHistory(...args),
}))

const mockUseFactoryMetadata = vi.fn()
const mockUseComponentChangeLog = vi.fn()
vi.mock('@/hooks/useFactoryMetadata', () => ({
  useFactoryMetadata: (...args: unknown[]) => mockUseFactoryMetadata(...args),
  useComponentChangeLog: (...args: unknown[]) => mockUseComponentChangeLog(...args),
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
  mockUsePropertyHistory.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUsePropertyCommands.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUseEventHistory.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUseCreatePropertyCommand.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUseDeletePropertyCommands.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUsePropertyShadow.mockReturnValue({
    data: { desired: {}, reported: {}, delta: {} },
    isLoading: false,
  })
  mockUseSetDesired.mockReturnValue({ mutate: vi.fn(), isPending: false, error: null })
  // Factory metadata: loading complete, no data. Cases needing data mock per-test.
  mockUseFactoryMetadata.mockReturnValue({ data: undefined, isLoading: false, isError: false })
  mockUseComponentChangeLog.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
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
      })
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

// --- Property Shadow section fixtures ---

/**
 * Build a converged shadow view: desired present but delta empty.
 */
function makeConvergedShadow(): ShadowView {
  return {
    desired: { brightness: 80 },
    reported: { brightness: { value: 80, time: '2025-01-01T10:00:00Z' } },
    delta: {},
    desired_updated_time: '2025-01-01T09:00:00Z',
    reported_updated_time: '2025-01-01T10:00:00Z',
  }
}

/**
 * Build a pending shadow view: delta non-empty (desired not yet converged).
 */
function makePendingShadow(): ShadowView {
  return {
    desired: { brightness: 80, colorTemp: 4000 },
    reported: {
      brightness: { value: 80, time: '2025-01-01T10:00:00Z' },
      colorTemp: { value: 3000, time: '2025-01-01T10:00:00Z' },
    },
    delta: { colorTemp: 4000 },
    desired_updated_time: '2025-01-01T09:00:00Z',
    reported_updated_time: '2025-01-01T10:00:00Z',
  }
}

// --- Factory metadata section fixtures ---

/**
 * Build a factory device view with no device-level metadata and no components
 * by default. Callers override `components` to populate the left-join table.
 */
function makeFactoryDeviceView(overrides: Partial<FactoryDeviceView> = {}): FactoryDeviceView {
  return {
    deviceSn: 'test-device-001',
    deviceMetadata: null,
    components: [],
    ...overrides,
  }
}

/**
 * Build a single component view with sensible defaults (a camera with a
 * certificate file attachment). Override any field per-test to exercise the
 * left-join partial-data fallbacks (`metadata: null`, `fileAttachments: []`,
 * `updatedAt: null`).
 */
function makeFactoryComponentView(
  overrides: Partial<FactoryComponentView> = {}
): FactoryComponentView {
  return {
    componentSn: 'comp-camera-001',
    componentType: 'camera',
    metadata: { firmware: '1.2.3' },
    fileAttachments: [
      {
        fileKey: 'certs/cert.pem',
        fileName: 'cert.pem',
        contentType: 'application/x-pem-file',
        sizeBytes: 2048,
      },
    ],
    updatedAt: '2026-07-18T10:00:00Z',
    ...overrides,
  }
}

/**
 * Build a single change-log entry. The backend returns SNAKE_CASE keys
 * (`component_sn`, `created_at`); `before: null` represents the initial
 * report (rendered as "Initial report" in the drawer).
 */
function makeChangeLogEntry(
  overrides: Partial<FactoryMetadataChangeLog> = {}
): FactoryMetadataChangeLog {
  return {
    id: 1,
    component_sn: 'comp-camera-001',
    before: null,
    after: { firmware: '1.2.3' },
    actor: 'factory',
    created_at: '2026-07-18T10:00:00Z',
    ...overrides,
  }
}

describe('Property Shadow section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('renders shadow section title and container', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Desired State (Shadow)')).toBeInTheDocument()
    expect(screen.getByTestId('shadow-section')).toBeInTheDocument()
  })

  test('renders delta rows when delta is not empty', () => {
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makePendingShadow(), isLoading: false })

    renderWithProviders(<Page />)

    const table = screen.getByTestId('shadow-delta-table')
    expect(table).toBeInTheDocument()
    // colorTemp -> kebab-case "color-temp"
    expect(screen.getByTestId('shadow-status-color-temp')).toBeInTheDocument()
    // The delta key value should appear in the desired-value column
    expect(table.textContent).toContain('4000')
    // Reported value (3000) should also appear
    expect(table.textContent).toContain('3000')
  })

  test('shows converged state when delta is empty', () => {
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makeConvergedShadow(), isLoading: false })

    renderWithProviders(<Page />)

    // desired present + delta empty => every desired key has converged, so the
    // table renders a row whose Status cell shows "Converged" (green).
    expect(screen.getByTestId('shadow-delta-table')).toBeInTheDocument()
    expect(screen.getByText('Converged')).toBeInTheDocument()
  })

  test('opens editor on set button click', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    // Editor is not present initially
    expect(screen.queryByTestId('shadow-desired-editor')).not.toBeInTheDocument()

    await user.click(screen.getByTestId('shadow-set-button'))

    expect(screen.getByTestId('shadow-desired-editor')).toBeInTheDocument()
  })

  test('closes editor on cancel button click', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    await user.click(screen.getByTestId('shadow-set-button'))
    expect(screen.getByTestId('shadow-desired-editor')).toBeInTheDocument()

    await user.click(screen.getByTestId('shadow-cancel-button'))

    expect(screen.queryByTestId('shadow-desired-editor')).not.toBeInTheDocument()
  })

  test('calls mutate with valid JSON', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    mockUseSetDesired.mockReturnValue({ mutate: mockMutate, isPending: false, error: null })

    renderWithProviders(<Page />)

    await user.click(screen.getByTestId('shadow-set-button'))
    const editor = screen.getByTestId('shadow-desired-editor')
    fireEvent.change(editor, { target: { value: '{"brightness": 90}' } })

    await user.click(screen.getByTestId('shadow-submit-button'))

    expect(mockMutate).toHaveBeenCalledTimes(1)
    expect(mockMutate).toHaveBeenCalledWith(
      {
        product_id: 'product-a',
        device_id: 'test-device-001',
        desired: { brightness: 90 },
      },
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      })
    )
  })

  test('shows parse error for invalid JSON', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    mockUseSetDesired.mockReturnValue({ mutate: mockMutate, isPending: false, error: null })

    renderWithProviders(<Page />)

    await user.click(screen.getByTestId('shadow-set-button'))
    const editor = screen.getByTestId('shadow-desired-editor')
    fireEvent.change(editor, { target: { value: 'not valid json' } })

    await user.click(screen.getByTestId('shadow-submit-button'))

    expect(screen.getByText('Invalid JSON')).toBeInTheDocument()
    expect(mockMutate).not.toHaveBeenCalled()
  })

  test('shows backend error when submitting empty desired object', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    // A desired view already exists so the table/desired area stays intact.
    mockUsePropertyShadow.mockReturnValue({ data: makeConvergedShadow(), isLoading: false })
    // Simulate the backend rejecting `{}`; the real error path surfaces via the
    // mutation's onError callback into a sonner toast.
    mockUseSetDesired.mockReturnValue({
      mutate: mockMutate,
      isPending: false,
      error: null,
    })

    renderWithProviders(<Page />)

    await user.click(screen.getByTestId('shadow-set-button'))
    const editor = screen.getByTestId('shadow-desired-editor')
    fireEvent.change(editor, { target: { value: '{}' } })

    await user.click(screen.getByTestId('shadow-submit-button'))

    // Empty object is a valid JSON object, so mutate IS called with desired: {}
    expect(mockMutate).toHaveBeenCalledWith(
      {
        product_id: 'product-a',
        device_id: 'test-device-001',
        desired: {},
      },
      expect.objectContaining({ onError: expect.any(Function) })
    )

    // Drive the backend error path: invoke the onError callback the component
    // registered, surfacing the backend rejection message via the sonner toast.
    const onError = mockMutate.mock.calls[0][1].onError as (e: unknown) => void
    onError(new Error('desired must be a non-empty JSON object'))

    const { toast } = await import('sonner')
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('Failed to set desired state', {
        description: 'desired must be a non-empty JSON object',
      })
    })

    // Existing desired view is not destroyed by the failed submit
    expect(screen.getByTestId('shadow-section')).toBeInTheDocument()
    // The dialog stays open on failure (editor remains available)
    expect(screen.getByTestId('shadow-desired-editor')).toBeInTheDocument()
  })

  test('shows delivery failed status when desired delta command failed', () => {
    setupMocks()
    // desired brightness=80, reported missing => not converged.
    mockUsePropertyShadow.mockReturnValue({
      data: {
        desired: { brightness: 80 },
        reported: {},
        delta: { brightness: 80 },
        desired_updated_time: '2025-01-01T09:00:00Z',
        reported_updated_time: null,
      },
      isLoading: false,
    })
    // Latest DesiredDelta command for brightness is Failed.
    mockUsePropertyCommands.mockReturnValue({
      data: {
        data: [
          {
            id: 1,
            product_id: 'product-a',
            device_id: 'test-device-001',
            command: { brightness: 80 },
            status: 'Failed',
            source: 'DesiredDelta',
            created_time: '2025-01-01T09:00:00Z',
            updated_time: '2025-01-01T09:01:00Z',
          },
        ],
        pagination: undefined,
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByTestId('shadow-status-brightness')).toHaveTextContent('Delivery failed')
  })

  test('shows queued status when desired delta command is pending', () => {
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({
      data: {
        desired: { brightness: 80 },
        reported: {},
        delta: { brightness: 80 },
        desired_updated_time: '2025-01-01T09:00:00Z',
        reported_updated_time: null,
      },
      isLoading: false,
    })
    mockUsePropertyCommands.mockReturnValue({
      data: {
        data: [
          {
            id: 1,
            product_id: 'product-a',
            device_id: 'test-device-001',
            command: { brightness: 80 },
            status: 'Pending',
            source: 'DesiredDelta',
            created_time: '2025-01-01T09:00:00Z',
            updated_time: '2025-01-01T09:00:00Z',
          },
        ],
        pagination: undefined,
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByTestId('shadow-status-brightness')).toHaveTextContent('Queued')
  })

  test('shows replied not converged when command succeeded but reported still differs', () => {
    setupMocks()
    // desired brightness=80, reported brightness=50 => not converged despite ack.
    mockUsePropertyShadow.mockReturnValue({
      data: {
        desired: { brightness: 80 },
        reported: { brightness: { value: 50, time: '2025-01-01T10:00:00Z' } },
        delta: { brightness: 80 },
        desired_updated_time: '2025-01-01T09:00:00Z',
        reported_updated_time: '2025-01-01T10:00:00Z',
      },
      isLoading: false,
    })
    mockUsePropertyCommands.mockReturnValue({
      data: {
        data: [
          {
            id: 1,
            product_id: 'product-a',
            device_id: 'test-device-001',
            command: { brightness: 80 },
            status: 'Success',
            source: 'DesiredDelta',
            created_time: '2025-01-01T09:00:00Z',
            updated_time: '2025-01-01T09:01:00Z',
          },
        ],
        pagination: undefined,
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByTestId('shadow-status-brightness')).toHaveTextContent(
      'Replied, not converged'
    )
  })

  test('ignores one-shot commands when resolving desired status', () => {
    setupMocks()
    // desired brightness=80, reported missing => not converged, no DesiredDelta
    // command exists. A one-shot command on the same key must NOT be correlated.
    mockUsePropertyShadow.mockReturnValue({
      data: {
        desired: { brightness: 80 },
        reported: {},
        delta: { brightness: 80 },
        desired_updated_time: '2025-01-01T09:00:00Z',
        reported_updated_time: null,
      },
      isLoading: false,
    })
    mockUsePropertyCommands.mockReturnValue({
      data: {
        data: [
          {
            id: 1,
            product_id: 'product-a',
            device_id: 'test-device-001',
            command: { brightness: 10 },
            status: 'Success',
            source: 'OneShot',
            created_time: '2025-01-01T08:00:00Z',
            updated_time: '2025-01-01T08:01:00Z',
          },
        ],
        pagination: undefined,
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    // One-shot command ignored => falls back to "Pending convergence".
    expect(screen.getByTestId('shadow-status-brightness')).toHaveTextContent('Pending convergence')
  })
})

describe('Factory metadata section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('renders section container', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByTestId('factory-metadata-section')).toBeInTheDocument()
  })

  test('shows device-level metadata not available placeholder', () => {
    setupMocks()
    // deviceMetadata is reserved/always null this round; the section surfaces
    // an explicit "not available" hint rather than rendering nothing.
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({ deviceMetadata: null }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText(/Device-level metadata:/i)).toHaveTextContent(
      'Device-level metadata: not available'
    )
  })

  test('renders one row per associated component', () => {
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        components: [
          makeFactoryComponentView({ componentSn: 'comp-camera-001' }),
          makeFactoryComponentView({
            componentSn: 'comp-sensor-002',
            componentType: 'sensor',
          }),
        ],
      }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)

    for (const sn of ['comp-camera-001', 'comp-sensor-002']) {
      expect(screen.getByTestId(`factory-component-row-${sn}`)).toBeInTheDocument()
      expect(screen.getByTestId(`factory-component-changes-btn-${sn}`)).toBeInTheDocument()
    }
  })

  const nullFieldCases: Array<{
    label: string
    override: Partial<FactoryComponentView>
    expected: string
  }> = [
    {
      label: 'metadata',
      override: { metadata: null },
      expected: 'Metadata not arrived',
    },
    {
      label: 'fileAttachments',
      override: { fileAttachments: [] },
      expected: '-',
    },
    {
      label: 'updatedAt',
      override: { updatedAt: null },
      expected: '-',
    },
  ]

  it.each(nullFieldCases)('renders null-field fallback for $label', ({ override, expected }) => {
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        components: [makeFactoryComponentView(override)],
      }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByTestId('factory-component-row-comp-camera-001')).toBeInTheDocument()
    // The fallback text must appear somewhere inside the section.
    expect(screen.getByTestId('factory-metadata-section').textContent).toContain(expected)
  })

  test('opens change log drawer when View change log button is clicked', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        components: [makeFactoryComponentView({ componentSn: 'comp-camera-001' })],
      }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)

    expect(screen.queryByTestId('component-change-log-drawer')).not.toBeInTheDocument()

    await user.click(screen.getByTestId('factory-component-changes-btn-comp-camera-001'))

    expect(await screen.findByTestId('component-change-log-drawer')).toBeInTheDocument()
  })

  test('renders Initial report when change log entry before is null', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        components: [makeFactoryComponentView({ componentSn: 'comp-camera-001' })],
      }),
      isLoading: false,
      isError: false,
    })
    // First entry has no predecessor: `before: null` renders as "Initial report".
    mockUseComponentChangeLog.mockReturnValue({
      data: { data: [makeChangeLogEntry({ before: null })], pagination: undefined },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    await user.click(screen.getByTestId('factory-component-changes-btn-comp-camera-001'))

    expect(await screen.findByText('Initial report')).toBeInTheDocument()
  })

  test('shows error message when hook returns isError (non-404)', async () => {
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('boom'),
    })

    renderWithProviders(<Page />)

    expect(screen.getByTestId('factory-metadata-error')).toBeInTheDocument()
    // Non-404 errors surface via a sonner toast (mirrors shadow error path).
    const { toast } = await import('sonner')
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'Failed to load factory metadata',
        expect.objectContaining({ description: 'boom' })
      )
    })
  })

  test('shows empty state card when hook returns 404 error', async () => {
    setupMocks()
    // react-query exposes the thrown backend 404 body as `error`. The factory
    // section matches it as a normal empty state and does NOT toast.
    mockUseFactoryMetadata.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: { error: 'Device has no factory metadata' },
    })

    // toast.error is a shared module-level mock (see src/test/setup.ts); clear
    // prior tests' calls so we observe only what happens in this test.
    const { toast } = await import('sonner')
    vi.mocked(toast.error).mockClear()

    renderWithProviders(<Page />)

    expect(screen.getByTestId('factory-metadata-empty')).toHaveTextContent(
      'This device has no factory metadata'
    )
    // No error card and no toast on the 404 branch.
    expect(screen.queryByTestId('factory-metadata-error')).not.toBeInTheDocument()
    expect(toast.error).not.toHaveBeenCalled()
  })
})
