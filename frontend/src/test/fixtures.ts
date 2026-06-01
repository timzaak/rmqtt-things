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
