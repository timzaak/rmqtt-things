import { describe, test, expect, vi } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__certsIndexComponent = options.component
      return { options }
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => vi.fn(),
  }
})

const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

const mockUseCerts = vi.fn()
const mockUseUpdateCertStatus = vi.fn()
const mockUseCaCert = vi.fn()
vi.mock('@/hooks/useCerts', () => ({
  useCerts: (...args: unknown[]) => mockUseCerts(...args),
  useUpdateCertStatus: () => mockUseUpdateCertStatus(),
  useCaCert: (...args: unknown[]) => mockUseCaCert(...args),
}))

// Import the module to trigger createRoute and capture the component
import '../index'

import type { Product, CertIssue } from '@/lib/api-generated/types.gen'

const mockProducts: Product[] = [
  {
    id: 1,
    name: 'Sensor A',
    model_no: 'SN-100',
    description: 'Temperature sensor',
    status: 'Online',
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-02T00:00:00Z',
  },
]

const mockCerts: CertIssue[] = [
  {
    id: 1,
    product_id: 'SN-100',
    device_id: 'device-001',
    created_at: '2025-01-01T10:00:00Z',
    end_at: '2026-01-01T10:00:00Z',
    start_at: '2025-01-01T10:00:00Z',
    pub_cert: '-----BEGIN CERT-----',
    status: 'Normal',
  },
  {
    id: 2,
    product_id: 'SN-100',
    device_id: 'device-002',
    created_at: '2025-02-01T10:00:00Z',
    end_at: '2026-02-01T10:00:00Z',
    start_at: '2025-02-01T10:00:00Z',
    pub_cert: '-----BEGIN CERT-----',
    status: 'Revoked',
  },
]

describe('CertsIndexPage', () => {
  const Page = (globalThis as Record<string, unknown>).__certsIndexComponent as React.ComponentType

  function setupMocks() {
    mockUseProducts.mockReturnValue({ data: mockProducts, isLoading: false })
    mockUseCerts.mockReturnValue({ data: { data: mockCerts }, isLoading: false })
    mockUseUpdateCertStatus.mockReturnValue({ mutate: vi.fn() })
    mockUseCaCert.mockReturnValue({ data: { ca_pem: '-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----' } })
  }

  test('renders PageHeader with title "Certificates"', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Certificates')).toBeInTheDocument()
  })

  test('renders SearchForm with product dropdown and device ID search', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // SearchForm renders a select for product and a text input for device ID
    // "Product" and "Device ID" also appear as table headers, so use getAllByText
    expect(screen.getAllByText('Product').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText('Device ID')).toBeInTheDocument()
    // Verify the search button exists
    expect(screen.getByRole('button', { name: /search/i })).toBeInTheDocument()
  })

  test('renders DataTable with cert data', () => {
    setupMocks()

    renderWithProviders(<Page />)

    // Table headers — "Product" appears in both SearchForm label and table header
    expect(screen.getByText('ID')).toBeInTheDocument()
    expect(screen.getAllByText('Product').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText('Device')).toBeInTheDocument()
    expect(screen.getByText('Status')).toBeInTheDocument()
    expect(screen.getByText('Actions')).toBeInTheDocument()
    // Cert data
    expect(screen.getByText('device-001')).toBeInTheDocument()
    expect(screen.getByText('device-002')).toBeInTheDocument()
    // Status labels
    expect(screen.getByText('Active')).toBeInTheDocument()
    expect(screen.getByText('Revoked')).toBeInTheDocument()
  })

  test('shows empty state when no certs', () => {
    mockUseProducts.mockReturnValue({ data: mockProducts, isLoading: false })
    mockUseCerts.mockReturnValue({ data: { data: [] }, isLoading: false })
    mockUseUpdateCertStatus.mockReturnValue({ mutate: vi.fn() })
    mockUseCaCert.mockReturnValue({ data: { ca_pem: '-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----' } })

    renderWithProviders(<Page />)

    expect(screen.getByText('No certificates found')).toBeInTheDocument()
  })

  test('renders Issue Certificate link', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Issue Certificate')).toBeInTheDocument()
  })
})
