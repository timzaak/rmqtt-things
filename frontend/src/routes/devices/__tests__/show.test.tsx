import { describe, test, it, expect, vi } from 'vitest'
import { screen, fireEvent, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type {
  DeviceStatusWithSource,
  ShadowView,
  DeviceOperationView,
  FactoryDeviceView,
  FactoryDeviceMetadataView,
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
const mockUsePropertyHistoryKeys = vi.fn()
const mockUsePropertyHistorySeries = vi.fn()
const mockUseCreatePropertyCommand = vi.fn()
const mockUsePropertyShadow = vi.fn()
const mockUseSetDesired = vi.fn()

vi.mock('@/hooks/useProperties', () => ({
  usePropertyLatest: (...args: unknown[]) => mockUsePropertyLatest(...args),
  usePropertyHistory: (...args: unknown[]) => mockUsePropertyHistory(...args),
  usePropertyHistoryKeys: (...args: unknown[]) => mockUsePropertyHistoryKeys(...args),
  usePropertyHistorySeries: (...args: unknown[]) => mockUsePropertyHistorySeries(...args),
  useCreatePropertyCommand: () => mockUseCreatePropertyCommand(),
  usePropertyShadow: (...args: unknown[]) => mockUsePropertyShadow(...args),
  useSetDesired: () => mockUseSetDesired(),
}))

// jsdom has no canvas 2d context; the chart canvas is covered by the
// Playwright demo, so the presentation component is stubbed here.
vi.mock('@/components/device-detail/PropertyChart', () => ({
  PropertyChart: () => <div data-testid="property-chart-container" />,
}))

const mockUseEventHistory = vi.fn()
vi.mock('@/hooks/useEvents', () => ({
  useEventHistory: (...args: unknown[]) => mockUseEventHistory(...args),
}))

// Run Action submission (DeviceOperationsSection -> ActionInvokeDialog). The
// section mounts only when its tab is active, but the hook module is imported
// at page-load time; mock it so no network call is ever issued.
const mockUseCreateActionInvocation = vi.fn()
vi.mock('@/hooks/useActionInvocations', () => ({
  useCreateActionInvocation: () => mockUseCreateActionInvocation(),
}))

// Unified operations query: used by DeviceOverviewSection (mounted by default)
// and DeviceOperationsSection.
const mockUseDeviceOperations = vi.fn()
vi.mock('@/hooks/useDeviceOperations', () => ({
  useDeviceOperations: (...args: unknown[]) => mockUseDeviceOperations(...args),
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
  mockUsePropertyHistoryKeys.mockReturnValue({
    data: { data: [] },
    isLoading: false,
    isSuccess: true,
    isError: false,
    refetch: vi.fn(),
  })
  mockUsePropertyHistorySeries.mockReturnValue({
    data: { data: [] },
    isLoading: false,
    isSuccess: true,
    isError: false,
    refetch: vi.fn(),
  })
  mockUseEventHistory.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUseCreatePropertyCommand.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUseCreateActionInvocation.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUseDeviceOperations.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
    isError: false,
  })
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

/**
 * Sections behind tabs mount lazily; tests must activate the tab before
 * asserting on its content. `user` must come from the calling test so the
 * click participates in the same user-event session.
 */
async function openTab(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByRole('tab', { name }))
}

// --- Device detail fixtures ---

/**
 * Build a ShadowView. `desired` holds bare values, `reported` entries are
 * wrapped as `{ value, time }`, and `delta` lists keys that have not
 * converged (key -> bare desired value) — mirrors backend `compute_delta`.
 */
function makeShadow(overrides: Partial<ShadowView> = {}): ShadowView {
  return {
    desired: {},
    reported: {},
    delta: {},
    ...overrides,
  }
}

/** Desired/reported/delta for a brightness target that has not converged. */
function makeOutOfSyncShadow(): ShadowView {
  return makeShadow({
    desired: { brightness: 80 },
    reported: { brightness: { value: 50, time: '2025-01-01T10:00:00Z' } },
    delta: { brightness: 80 },
  })
}

/** Build one unified operations row with sensible defaults. */
function makeOperation(overrides: Partial<DeviceOperationView> = {}): DeviceOperationView {
  return {
    operationId: 'property:1',
    name: 'Sync target',
    operationType: 'targetSync',
    status: 'Success',
    payload: { brightness: 80 },
    createdTime: '2025-01-01T10:00:00Z',
    updatedTime: '2025-01-01T10:00:00Z',
    ...overrides,
  }
}

/** Params of the most recent useDeviceOperations call (any mounted section). */
function lastOperationsParams(): Record<string, unknown> {
  const calls = mockUseDeviceOperations.mock.calls
  return calls[calls.length - 1][0] as Record<string, unknown>
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

describe('tab navigation', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('defaults to Overview and keeps the other six sections unmounted', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // Overview is the default tab: device info + latest properties.
    expect(screen.getByRole('heading', { name: 'Device Info' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Latest Properties' })).toBeInTheDocument()
    // Other sections stay unmounted (and thus unfetched) until their tab opens.
    expect(screen.queryByRole('heading', { name: 'State & Configuration' })).not.toBeInTheDocument()
    expect(screen.queryByTestId('device-operations-table')).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Property History' })).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Event History' })).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Connection History' })).not.toBeInTheDocument()
    expect(screen.queryByTestId('factory-metadata-section')).not.toBeInTheDocument()
  })

  test('exposes stable testids on the new tabs', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByTestId('device-tab-state-configuration')).toBeInTheDocument()
    expect(screen.getByTestId('device-tab-operations')).toBeInTheDocument()
    expect(screen.getByTestId('device-tab-reported-data')).toBeInTheDocument()
  })

  test('mounts the selected section and unmounts the previous one', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)

    await openTab(user, 'State & Configuration')
    expect(screen.getByRole('heading', { name: 'State & Configuration' })).toBeInTheDocument()
    // The overview content unmounts once another tab is active.
    expect(screen.queryByRole('heading', { name: 'Device Info' })).not.toBeInTheDocument()

    await openTab(user, 'Operations')
    expect(screen.getByTestId('device-operations-table')).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'State & Configuration' })).not.toBeInTheDocument()

    await openTab(user, 'Reported Data')
    expect(screen.getByRole('heading', { name: 'Property History' })).toBeInTheDocument()
    expect(screen.queryByTestId('device-operations-table')).not.toBeInTheDocument()

    await openTab(user, 'Events')
    expect(screen.getByRole('heading', { name: 'Event History' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Property History' })).not.toBeInTheDocument()

    await openTab(user, 'Connectivity')
    expect(screen.getByRole('heading', { name: 'Connection History' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Event History' })).not.toBeInTheDocument()

    await openTab(user, 'Metadata')
    expect(screen.getByTestId('factory-metadata-section')).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Connection History' })).not.toBeInTheDocument()
  })
})

describe('Reported Data section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('defaults to the chart view with view-switch buttons', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUsePropertyHistoryKeys.mockReturnValue({
      data: {
        data: [
          { key: 'temperature', sampleCount: 3 },
          { key: 'humidity', sampleCount: 1 },
        ],
      },
      isLoading: false,
      isSuccess: true,
      isError: false,
      refetch: vi.fn(),
    })
    mockUsePropertyHistorySeries.mockReturnValue({
      data: {
        data: [{ key: 'temperature', totalPoints: 1, downsampled: false, stride: 1, points: [] }],
      },
      isLoading: false,
      isSuccess: true,
      isError: false,
      refetch: vi.fn(),
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Reported Data')

    // The chart is the default view; the history table stays one click away.
    // (A table IS present in this tab — Latest Properties — so the assertion
    // targets the history table's distinctive column header.)
    expect(screen.getByTestId('property-history-view-chart')).toBeInTheDocument()
    expect(screen.getByTestId('property-history-view-table')).toBeInTheDocument()
    expect(screen.getByTestId('property-chart-key-toggle-temperature')).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: 'Reported Time' })).not.toBeInTheDocument()
  })

  test('renders property history table with mock data after switching views', async () => {
    const user = userEvent.setup()
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
    await openTab(user, 'Reported Data')

    // Table view must stay reachable and behave exactly as before (the chart
    // is an additive view; the legacy table contract is pinned).
    await user.click(screen.getByTestId('property-history-view-table'))

    expect(screen.getByText('1')).toBeInTheDocument()
    // Check that property data is rendered (inside a <pre> block)
    expect(screen.getByText(/temperature/)).toBeInTheDocument()
    expect(screen.queryByTestId('property-chart-key-toggle-temperature')).not.toBeInTheDocument()
  })
})

describe('State & Configuration section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  // Sync mapping derived solely from the ShadowView.
  const syncCases: Array<{
    scenario: string
    shadow: ShadowView
    expectedLabel: string
  }> = [
    {
      scenario: 'desired lacks the key',
      shadow: makeShadow({
        reported: { brightness: { value: 50, time: '2025-01-01T10:00:00Z' } },
      }),
      expectedLabel: 'Target not set',
    },
    {
      scenario: 'desired has the key and delta does not',
      shadow: makeShadow({
        desired: { brightness: 80 },
        reported: { brightness: { value: 80, time: '2025-01-01T10:00:00Z' } },
      }),
      expectedLabel: 'In sync',
    },
    {
      scenario: 'desired has the key and delta still lists it',
      shadow: makeOutOfSyncShadow(),
      expectedLabel: 'Out of sync',
    },
  ]

  it.each(syncCases)('shows "$expectedLabel" when $scenario', async ({ shadow, expectedLabel }) => {
    const user = userEvent.setup()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: shadow, isLoading: false })

    renderWithProviders(<Page />)
    await openTab(user, 'State & Configuration')

    const row = within(screen.getByTestId('state-configuration-table'))
      .getByText('brightness')
      .closest('tr')
    expect(row).not.toBeNull()
    // "Target not set" appears in both the Target and Sync columns for an
    // untargeted key, so assert presence rather than a single match.
    expect(within(row as HTMLElement).getAllByText(expectedLabel).length).toBeGreaterThan(0)
  })

  test('applies a single target property via its Apply button', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makeOutOfSyncShadow(), isLoading: false })
    mockUseSetDesired.mockReturnValue({ mutate: mockMutate, isPending: false, error: null })

    renderWithProviders(<Page />)
    await openTab(user, 'State & Configuration')

    await user.click(screen.getByTestId('target-apply-button-brightness'))

    expect(mockMutate).toHaveBeenCalledWith(
      {
        product_id: 'product-a',
        device_id: 'test-device-001',
        desired: { brightness: 80 },
      },
      expect.objectContaining({ onError: expect.any(Function) })
    )
  })

  test('submits a replacement Target through the Update Target dialog', async () => {
    const user = userEvent.setup()
    const mockMutate = vi.fn()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makeOutOfSyncShadow(), isLoading: false })
    mockUseSetDesired.mockReturnValue({ mutate: mockMutate, isPending: false, error: null })

    renderWithProviders(<Page />)
    await openTab(user, 'State & Configuration')

    await user.click(screen.getByTestId('target-update-button'))
    // fireEvent.change avoids userEvent's special-character interpretation.
    fireEvent.change(screen.getByTestId('target-json-input'), {
      target: { value: '{"brightness": 100}' },
    })
    await user.click(screen.getByTestId('submit-button'))

    expect(mockMutate).toHaveBeenCalledWith(
      {
        product_id: 'product-a',
        device_id: 'test-device-001',
        desired: { brightness: 100 },
      },
      expect.objectContaining({ onSuccess: expect.any(Function) })
    )
  })
})

describe('Operations section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('renders one row per operation with its fixed type label', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseDeviceOperations.mockReturnValue({
      data: {
        data: [
          makeOperation({ operationId: 'property:1', operationType: 'targetSync' }),
          makeOperation({
            operationId: 'property:2',
            operationType: 'directPropertyWrite',
            name: 'Set properties',
            status: 'Sent',
          }),
          makeOperation({
            operationId: 'action:3',
            operationType: 'actionInvocation',
            name: 'reboot',
            status: 'Failed',
          }),
        ],
        pagination: { page: 1, page_size: 10, total: 3 },
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Operations')

    const table = screen.getByTestId('device-operations-table')
    expect(within(table).getByText('Target sync')).toBeInTheDocument()
    expect(within(table).getByText('Direct write')).toBeInTheDocument()
    expect(within(table).getByText('Action')).toBeInTheDocument()
  })

  test('passes type and status filters to useDeviceOperations', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)
    await openTab(user, 'Operations')

    expect(lastOperationsParams()).toEqual({
      product_id: 'product-a',
      device_id: 'test-device-001',
      operation_type: null,
      status: null,
      page: 1,
      page_size: 10,
    })

    fireEvent.change(screen.getByTestId('operation-type-filter'), {
      target: { value: 'targetSync' },
    })
    expect(lastOperationsParams()).toMatchObject({ operation_type: 'targetSync', page: 1 })

    fireEvent.change(screen.getByTestId('operation-status-filter'), {
      target: { value: 'Failed' },
    })
    expect(lastOperationsParams()).toMatchObject({
      operation_type: 'targetSync',
      status: 'Failed',
      page: 1,
    })
  })

  test('passes pagination changes to useDeviceOperations', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseDeviceOperations.mockReturnValue({
      data: {
        data: [makeOperation()],
        pagination: { page: 1, page_size: 10, total: 25 },
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Operations')

    // The two pagination buttons are the only buttons inside the table
    // container: [previous, next].
    const [, nextButton] = within(screen.getByTestId('device-operations-table')).getAllByRole(
      'button'
    )
    await user.click(nextButton)

    expect(lastOperationsParams()).toMatchObject({ page: 2, page_size: 10 })
  })
})

// Design G3: a successful device reply does not imply Target convergence —
// the two regions carry independent semantics and must coexist.
describe('operation success coexisting with Out of sync (G3)', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('shows Out of sync in State & Configuration while the operation shows Success', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makeOutOfSyncShadow(), isLoading: false })
    mockUseDeviceOperations.mockReturnValue({
      data: {
        data: [makeOperation({ operationType: 'targetSync', status: 'Success' })],
        pagination: { page: 1, page_size: 10, total: 1 },
      },
      isLoading: false,
    })

    renderWithProviders(<Page />)

    await openTab(user, 'State & Configuration')
    expect(
      within(screen.getByTestId('state-configuration-table')).getByText('Out of sync')
    ).toBeInTheDocument()

    await openTab(user, 'Operations')
    const table = screen.getByTestId('device-operations-table')
    expect(within(table).getByText('Success')).toBeInTheDocument()
    expect(within(table).getByText('Target sync')).toBeInTheDocument()
  })
})

describe('Direct property write conflict warning', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  const conflictCases: Array<{
    scenario: string
    input: string
    expectWarning: boolean
  }> = [
    {
      scenario: 'the value differs from the Target value',
      input: '{"brightness": 50}',
      expectWarning: true,
    },
    {
      scenario: 'the value equals the Target value',
      input: '{"brightness": 80}',
      expectWarning: false,
    },
    {
      scenario: 'the key is not in Target',
      input: '{"temperature": 22}',
      expectWarning: false,
    },
  ]

  it.each(conflictCases)('warns only when $scenario', async ({ input, expectWarning }) => {
    const user = userEvent.setup()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({
      data: makeShadow({ desired: { brightness: 80 } }),
      isLoading: false,
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Operations')
    await user.click(screen.getByTestId('more-actions-button'))
    await user.click(screen.getByTestId('direct-property-write-button'))

    const dialog = screen.getByTestId('direct-property-write-dialog')
    fireEvent.change(within(dialog).getByTestId('command-json-input'), {
      target: { value: input },
    })

    const warning = within(dialog).queryByTestId('target-conflict-warning')
    if (expectWarning) {
      expect(warning).toHaveTextContent(
        'This one-time write does not change Target and may leave the device out of sync.'
      )
    } else {
      expect(warning).not.toBeInTheDocument()
    }
  })
})

describe('section-level error isolation', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('keeps the page shell and other sections usable when Operations fails to load', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUsePropertyShadow.mockReturnValue({ data: makeOutOfSyncShadow(), isLoading: false })
    mockUseDeviceOperations.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('boom'),
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Operations')

    // The failure is scoped to the Operations section: the page shell and the
    // tab bar stay usable instead of replacing the whole detail page.
    expect(screen.getByTestId('device-operations-error')).toBeInTheDocument()
    expect(screen.getByText('Device Detail')).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'State & Configuration' })).toBeInTheDocument()

    await openTab(user, 'State & Configuration')
    expect(screen.getByTestId('state-configuration-table')).toBeInTheDocument()
    expect(screen.queryByTestId('device-operations-error')).not.toBeInTheDocument()
  })
})

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
 * Build device-level factory metadata with sensible defaults (a serial entry
 * plus a QC report attachment). Override any field per-test to exercise the
 * device-level data-driven block (`metadata: null`, `fileAttachments: []`,
 * `updatedAt: null`).
 */
function makeFactoryDeviceMetadataView(
  overrides: Partial<FactoryDeviceMetadataView> = {}
): FactoryDeviceMetadataView {
  return {
    metadata: { serial: 'SN-DEV-001' },
    fileAttachments: [
      {
        fileKey: 'reports/qc.pdf',
        fileName: 'qc.pdf',
        contentType: 'application/pdf',
        sizeBytes: 12345,
      },
    ],
    updatedAt: '2026-07-23T08:00:00Z',
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
 * (`created_at`); `before: null` represents the initial
 * report (rendered as "Initial report" in the drawer).
 */
function makeChangeLogEntry(
  overrides: Partial<FactoryMetadataChangeLog> = {}
): FactoryMetadataChangeLog {
  return {
    id: 1,
    sn: 'comp-camera-001',
    before: null,
    after: { firmware: '1.2.3' },
    actor: 'factory',
    created_at: '2026-07-18T10:00:00Z',
    ...overrides,
  }
}

describe('Factory metadata section', () => {
  const Page = (globalThis as Record<string, unknown>).__devicesShowComponent as React.ComponentType

  test('renders section container', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    expect(screen.getByTestId('factory-metadata-section')).toBeInTheDocument()
  })

  test('hides device-level metadata block when metadata is absent', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({ deviceMetadata: null }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    expect(screen.queryByTestId('factory-device-metadata-block')).not.toBeInTheDocument()
    expect(screen.queryByText(/not available/i)).not.toBeInTheDocument()
  })

  test('renders device-level metadata content when deviceMetadata is present', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        deviceMetadata: makeFactoryDeviceMetadataView(),
      }),
      isLoading: false,
      isError: false,
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    // Narrow to the device-level block (avoids catching the section container,
    // which also includes component-level rows / fallbacks).
    const block = screen.getByTestId('factory-device-metadata-block')
    expect(block.textContent).toContain('SN-DEV-001')
    expect(block.textContent).not.toContain('not available')
    expect(block.textContent).toContain('qc.pdf')
  })

  test('opens change log drawer for device-level metadata and queries with deviceSn', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        deviceMetadata: makeFactoryDeviceMetadataView(),
      }),
      isLoading: false,
      isError: false,
    })
    // The drawer uses useComponentChangeLog(sn, page); mock the hook so that
    // when it is called with the device SN it returns a recognisable entry.
    mockUseComponentChangeLog.mockImplementation((sn: string) => ({
      data: {
        data:
          sn === 'test-device-001'
            ? [makeChangeLogEntry({ sn: 'test-device-001', after: { serial: 'SN-DEV-001' } })]
            : [],
        pagination: undefined,
      },
      isLoading: false,
    }))

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    await user.click(screen.getByTestId('factory-device-changes-btn'))

    const drawer = await screen.findByTestId('component-change-log-drawer')
    // The device-level entry must drive the drawer with the device SN, not a
    // component SN (the two triggers share one drawer via `drawerSn`).
    expect(drawer).toHaveTextContent('test-device-001')
    expect(mockUseComponentChangeLog).toHaveBeenCalledWith('test-device-001', 1)
  })

  test('keeps component-level change log button independent of device-level drawer', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: makeFactoryDeviceView({
        deviceMetadata: makeFactoryDeviceMetadataView(),
        components: [makeFactoryComponentView({ componentSn: 'comp-camera-001' })],
      }),
      isLoading: false,
      isError: false,
    })
    // Return a recognisable entry per SN so each drawer instance is identifiable.
    mockUseComponentChangeLog.mockImplementation((sn: string) => ({
      data: {
        data: [makeChangeLogEntry({ sn, after: { [sn]: true } })],
        pagination: undefined,
      },
      isLoading: false,
    }))

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    // Component-level entry: drawer queries with the component SN.
    await user.click(screen.getByTestId('factory-component-changes-btn-comp-camera-001'))
    let drawer = await screen.findByTestId('component-change-log-drawer')
    expect(drawer).toHaveTextContent('comp-camera-001')

    // Close the drawer (resets both selectedSn and deviceLogOpen via onClose).
    await user.click(screen.getByTestId('component-change-log-close'))
    await waitFor(() => {
      expect(screen.queryByTestId('component-change-log-drawer')).not.toBeInTheDocument()
    })

    await user.click(screen.getByTestId('factory-device-changes-btn'))
    drawer = await screen.findByTestId('component-change-log-drawer')
    expect(drawer).toHaveTextContent('test-device-001')
    expect(mockUseComponentChangeLog).toHaveBeenLastCalledWith('test-device-001', 1)
  })

  test('renders one row per associated component', async () => {
    const user = userEvent.setup()
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
    await openTab(user, 'Metadata')

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

  it.each(nullFieldCases)(
    'renders null-field fallback for $label',
    async ({ override, expected }) => {
      const user = userEvent.setup()
      setupMocks()
      mockUseFactoryMetadata.mockReturnValue({
        data: makeFactoryDeviceView({
          components: [makeFactoryComponentView(override)],
        }),
        isLoading: false,
        isError: false,
      })

      renderWithProviders(<Page />)
      await openTab(user, 'Metadata')

      expect(screen.getByTestId('factory-component-row-comp-camera-001')).toBeInTheDocument()
      // The fallback text must appear somewhere inside the section.
      expect(screen.getByTestId('factory-metadata-section').textContent).toContain(expected)
    }
  )

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
    await openTab(user, 'Metadata')

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
    await openTab(user, 'Metadata')

    await user.click(screen.getByTestId('factory-component-changes-btn-comp-camera-001'))

    expect(await screen.findByText('Initial report')).toBeInTheDocument()
  })

  test('shows error message when hook returns isError (non-404)', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseFactoryMetadata.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('boom'),
    })

    renderWithProviders(<Page />)
    await openTab(user, 'Metadata')

    expect(screen.getByTestId('factory-metadata-error')).toBeInTheDocument()
    // Non-404 errors surface via a sonner toast.
    const { toast } = await import('sonner')
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'Failed to load factory metadata',
        expect.objectContaining({ description: 'boom' })
      )
    })
  })

  test('renders no empty-state description when hook returns 404 error', async () => {
    const user = userEvent.setup()
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
    await openTab(user, 'Metadata')

    expect(screen.queryByTestId('factory-metadata-empty')).not.toBeInTheDocument()
    expect(screen.queryByText('This device has no factory metadata')).not.toBeInTheDocument()
    // No error card and no toast on the 404 branch.
    expect(screen.queryByTestId('factory-metadata-error')).not.toBeInTheDocument()
    expect(toast.error).not.toHaveBeenCalled()
  })
})
