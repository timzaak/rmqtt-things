import type { Product } from '@/lib/api-generated/types.gen'
import type { EventValidTemplate } from '@/lib/api-generated/types.gen'

export const mockProduct: Product = {
  id: 1,
  name: 'Sensor A',
  model_no: 'SN-100',
  description: 'Temperature sensor',
  status: 'Online',
  auto_provisioning: false,
  created_at: '2025-01-01T00:00:00Z',
  updated_at: '2025-01-02T00:00:00Z',
}

export const mockProducts: Product[] = [
  mockProduct,
  {
    id: 2,
    name: 'Actuator B',
    model_no: 'AC-200',
    description: null,
    status: 'Offline',
    auto_provisioning: false,
    created_at: '2025-01-03T00:00:00Z',
    updated_at: '2025-01-04T00:00:00Z',
  },
]

export const mockDraftValidTemplate: EventValidTemplate = {
  id: 1,
  product_id: 'SN-100',
  event: 'temperature_report',
  description: 'Temperature reading schema',
  status: 'Draft',
  schema: {
    type: 'object',
    properties: { temperature: { type: 'number', description: 'Celsius' } },
    required: ['temperature'],
  },
  created_at: '2025-01-01T00:00:00Z',
  updated_at: '2025-01-02T00:00:00Z',
}

export const mockActiveValidTemplate: EventValidTemplate = {
  ...mockDraftValidTemplate,
  id: 2,
  status: 'Active',
}

// --- Property history chart fixtures (property-history-visualization) ---

import type {
  PropertyChartKeysResponse,
  PropertySeriesListResponse,
} from '@/lib/api-generated/types.gen'

/** Six discovered numeric keys so tests can exercise the 5-key cap. */
export const mockPropertyChartKeys: PropertyChartKeysResponse = {
  data: [
    { key: 'temperature', sampleCount: 231 },
    { key: 'humidity', sampleCount: 87 },
    { key: 'voltage', sampleCount: 40 },
    { key: 'brightness', sampleCount: 12 },
    { key: 'pressure', sampleCount: 5 },
    { key: 'extra', sampleCount: 1 },
  ],
}

export const mockPropertyChartKeysEmpty: PropertyChartKeysResponse = { data: [] }

/** One populated series plus one empty series (chip "无数据" badge case). */
export const mockPropertySeries: PropertySeriesListResponse = {
  data: [
    {
      key: 'temperature',
      totalPoints: 3,
      downsampled: false,
      stride: 1,
      points: [
        { time: '2026-09-01T08:00:00Z', value: 20 },
        { time: '2026-09-01T09:00:00Z', value: 21.5 },
        { time: '2026-09-01T10:00:00Z', value: 23 },
      ],
    },
    {
      key: 'humidity',
      totalPoints: 0,
      downsampled: false,
      stride: 1,
      points: [],
    },
  ],
}

/** Every series empty in range: the chart-area empty state (not an error). */
export const mockPropertySeriesAllEmpty: PropertySeriesListResponse = {
  data: [{ key: 'temperature', totalPoints: 0, downsampled: false, stride: 1, points: [] }],
}

/** Downsampled series: the visible sampling note must be rendered. */
export const mockPropertySeriesDownsampled: PropertySeriesListResponse = {
  data: [
    {
      key: 'temperature',
      totalPoints: 2500,
      downsampled: true,
      stride: 3,
      points: [
        { time: '2026-09-01T00:00:00Z', value: 20 },
        { time: '2026-09-01T03:00:00Z', value: 22 },
      ],
    },
  ],
}
