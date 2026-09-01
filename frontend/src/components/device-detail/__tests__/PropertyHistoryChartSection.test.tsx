import { describe, test, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import {
  mockPropertyChartKeys,
  mockPropertyChartKeysEmpty,
  mockPropertySeries,
  mockPropertySeriesAllEmpty,
  mockPropertySeriesDownsampled,
} from '@/test/fixtures'
import type {
  PropertyChartKeysResponse,
  PropertySeriesListResponse,
} from '@/lib/api-generated/types.gen'

const mockUsePropertyHistory = vi.fn()
const mockUsePropertyHistoryKeys = vi.fn()
const mockUsePropertyHistorySeries = vi.fn()

vi.mock('@/hooks/useProperties', () => ({
  usePropertyHistory: (...args: unknown[]) => mockUsePropertyHistory(...args),
  usePropertyHistoryKeys: (...args: unknown[]) => mockUsePropertyHistoryKeys(...args),
  usePropertyHistorySeries: (...args: unknown[]) => mockUsePropertyHistorySeries(...args),
}))

// jsdom has no canvas 2d context; the chart canvas itself is covered by the
// Playwright demo, so the presentation component is stubbed here.
vi.mock('@/components/device-detail/PropertyChart', () => ({
  PropertyChart: () => <div data-testid="property-chart-container" />,
}))

import { PropertyHistorySection } from '../PropertyHistorySection'

function lastSeriesParams(): Record<string, unknown> {
  const calls = mockUsePropertyHistorySeries.mock.calls
  return calls[calls.length - 1][0] as Record<string, unknown>
}

function lastSeriesOptions(): { enabled?: boolean } | undefined {
  const calls = mockUsePropertyHistorySeries.mock.calls
  return calls[calls.length - 1][1] as { enabled?: boolean } | undefined
}

function setup({
  keys = mockPropertyChartKeys,
  series = mockPropertySeries,
  seriesError = false,
}: {
  keys?: PropertyChartKeysResponse
  series?: PropertySeriesListResponse
  seriesError?: boolean
} = {}) {
  mockUsePropertyHistory.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUsePropertyHistoryKeys.mockReturnValue({
    data: keys,
    isLoading: false,
    isSuccess: true,
    isError: false,
    refetch: vi.fn(),
  })
  mockUsePropertyHistorySeries.mockImplementation(() =>
    seriesError
      ? {
          data: undefined,
          isLoading: false,
          isSuccess: false,
          isError: true,
          error: new Error('boom'),
          refetch: vi.fn(),
        }
      : {
          data: series,
          isLoading: false,
          isSuccess: true,
          isError: false,
          refetch: vi.fn(),
        }
  )
}

describe('PropertyHistorySection dual view', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('defaults to the chart view and preselects the most active key', async () => {
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)

    // Chart view is the default; the table is one click away (additive dual
    // view keeps both side by side).
    expect(screen.getByTestId('property-history-view-chart')).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()

    // The keys are sorted by sampleCount desc, so data[0] (temperature) is
    // auto-selected and drives the first series query.
    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual(['temperature'])
    })
    expect(screen.getByTestId('property-chart-container')).toBeInTheDocument()
  })

  test('switching views keeps chart conditions and restores the table unchanged', async () => {
    const user = userEvent.setup()
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await user.click(screen.getByTestId('property-chart-key-toggle-humidity'))
    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual(['temperature', 'humidity'])
    })

    await user.click(screen.getByTestId('property-history-view-table'))
    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(screen.queryByTestId('property-chart-container')).not.toBeInTheDocument()

    await user.click(screen.getByTestId('property-history-view-chart'))
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    // Selection survived the round trip (FR1: switching never loses state).
    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual(['temperature', 'humidity'])
    })
  })
})

describe('PropertyHistoryChartSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('caps the selection at 5 keys and disables the remaining chips', async () => {
    const user = userEvent.setup()
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await user.click(screen.getByTestId('property-chart-key-toggle-humidity'))
    await user.click(screen.getByTestId('property-chart-key-toggle-voltage'))
    await user.click(screen.getByTestId('property-chart-key-toggle-brightness'))
    await user.click(screen.getByTestId('property-chart-key-toggle-pressure'))

    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual([
        'temperature',
        'humidity',
        'voltage',
        'brightness',
        'pressure',
      ])
    })

    // The 6th key is beyond the contract cap (1..=5) and must be inert.
    const extra = screen.getByTestId('property-chart-key-toggle-extra') as HTMLButtonElement
    expect(extra.disabled).toBe(true)
    await user.click(extra)
    expect(lastSeriesParams().keys).toHaveLength(5)
  })

  test('changing the range preset issues a new query with shifted bounds', async () => {
    const user = userEvent.setup()
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual(['temperature'])
    })
    const firstStart = lastSeriesParams().start_time as string

    // Presets resolve at selection time; a 7d window starts earlier than 24h.
    await user.selectOptions(screen.getByTestId('property-chart-range-select'), '7d')
    await vi.waitFor(() => {
      const secondStart = lastSeriesParams().start_time as string
      expect(new Date(secondStart).getTime()).toBeLessThan(new Date(firstStart).getTime())
    })
    expect(lastSeriesParams().end_time).toBeTruthy()
  })

  test('custom range with start >= end holds the query and shows a hint', async () => {
    const user = userEvent.setup()
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await vi.waitFor(() => {
      expect(lastSeriesParams().keys).toEqual(['temperature'])
    })

    await user.selectOptions(screen.getByTestId('property-chart-range-select'), 'custom')
    fireEvent.change(screen.getByTestId('property-chart-start-input'), {
      target: { value: '2026-09-01T12:00' },
    })
    fireEvent.change(screen.getByTestId('property-chart-end-input'), {
      target: { value: '2026-09-01T10:00' },
    })

    // Inverted custom bounds disable the query (enabled=false reaches the
    // hook options); the user gets inline guidance instead of an API call.
    await vi.waitFor(() => {
      expect(lastSeriesOptions()).toEqual({ enabled: false })
    })
    expect(screen.getByText('结束时间需晚于起始时间')).toBeInTheDocument()

    // Correcting the end time resumes querying automatically.
    fireEvent.change(screen.getByTestId('property-chart-end-input'), {
      target: { value: '2026-09-01T13:00' },
    })
    await vi.waitFor(() => {
      expect(lastSeriesOptions()).toEqual({ enabled: true })
    })
  })

  test('renders the empty states: no keys, empty range, and no selection', async () => {
    const user = userEvent.setup()

    // No numeric keys at all: guidance with a table switch, not an error.
    setup({ keys: mockPropertyChartKeysEmpty })
    const { unmount } = renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    expect(screen.getByTestId('property-chart-keys-empty')).toBeInTheDocument()
    expect(screen.queryByTestId('property-chart-error')).not.toBeInTheDocument()
    unmount()

    // All series empty in range: chart-area empty state.
    setup({ series: mockPropertySeriesAllEmpty })
    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await vi.waitFor(() => {
      expect(screen.getByTestId('property-chart-empty')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('property-chart-container')).not.toBeInTheDocument()
    expect(screen.queryByTestId('property-chart-error')).not.toBeInTheDocument()

    // Deselecting every key lands on the no-selection hint.
    await user.click(screen.getByTestId('property-chart-key-toggle-temperature'))
    expect(screen.getByTestId('property-chart-no-selection')).toBeInTheDocument()
  })

  test('renders the error state with retry and marks empty keys on chips', async () => {
    setup({ seriesError: true })

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    expect(await screen.findByTestId('property-chart-error')).toBeInTheDocument()
    expect(screen.queryByTestId('property-chart-container')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })

  test('chip shows the 无-data badge for series with totalPoints 0', async () => {
    setup()

    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await userClickChip('humidity')

    const humidityChip = screen.getByTestId('property-chart-key-toggle-humidity')
    expect(humidityChip).toHaveTextContent('无数据')
    // A populated series never gets the badge.
    expect(screen.getByTestId('property-chart-key-toggle-temperature')).not.toHaveTextContent(
      '无数据'
    )
  })

  test('renders the visible downsample note only when downsampled is true', async () => {
    setup({ series: mockPropertySeriesDownsampled })

    const { unmount } = renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    const note = await screen.findByTestId('property-chart-downsample-note-temperature')
    // The note must disclose totals, shown points and stride.
    expect(note).toHaveTextContent('2500')
    expect(note).toHaveTextContent('2')
    expect(note).toHaveTextContent('3')
    unmount()

    // Without downsampling there is no note at all — sampled points must
    // never render silently as if they were the full series.
    setup({ series: mockPropertySeries })
    renderWithProviders(<PropertyHistorySection productId="p1" deviceId="d1" />)
    await vi.waitFor(() => {
      expect(screen.getByTestId('property-chart-container')).toBeInTheDocument()
    })
    expect(
      screen.queryByTestId('property-chart-downsample-note-temperature')
    ).not.toBeInTheDocument()
  })
})

async function userClickChip(key: string) {
  const user = userEvent.setup()
  await user.click(screen.getByTestId(`property-chart-key-toggle-${key}`))
}
