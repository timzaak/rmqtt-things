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
      ;(globalThis as Record<string, unknown>).__certsCreateComponent = options.component
      return { options }
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
    useNavigate: () => mockNavigate,
    useBlocker: () => ({ status: 'idle' }),
  }
})

const mockUseProducts = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProducts: (...args: unknown[]) => mockUseProducts(...args),
}))

const mockMutate = vi.fn()
const mockUseIssueCert = vi.fn()
vi.mock('@/hooks/useCerts', () => ({
  useIssueCert: () => mockUseIssueCert(),
}))

// Import the module to trigger createRoute and capture the component
import '../create'

import { mockProducts } from '@/test/fixtures'

const fakeIssuedCert = {
  cert_pem: '-----BEGIN CERTIFICATE-----\nFAKE\n-----END CERTIFICATE-----',
  key_pem: '-----BEGIN PRIVATE KEY-----\nFAKE\n-----END PRIVATE KEY-----',
}

describe('CertsCreatePage', () => {
  const Page = (globalThis as Record<string, unknown>).__certsCreateComponent as React.ComponentType

  function setupMocks() {
    mockUseProducts.mockReturnValue({
      data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 1 } },
      isLoading: false,
    })
    mockUseIssueCert.mockReturnValue({ mutate: mockMutate, isPending: false })
  }

  function setupMocksWithAutoSuccess() {
    mockUseProducts.mockReturnValue({
      data: { data: mockProducts, pagination: { page: 1, page_size: 10, total: 1 } },
      isLoading: false,
    })
    mockMutate.mockImplementation(
      (_data: unknown, options: { onSuccess: (data: unknown) => void }) => {
        options.onSuccess(fakeIssuedCert)
      }
    )
    mockUseIssueCert.mockReturnValue({ mutate: mockMutate, isPending: false })
  }

  test('renders form with all 5 fields', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByText('Issue Certificate')).toBeInTheDocument()
    // Product dropdown
    expect(screen.getByLabelText(/product/i)).toBeInTheDocument()
    // Device ID text input
    expect(screen.getByLabelText(/device id/i)).toBeInTheDocument()
    // Force checkbox
    expect(screen.getByLabelText(/force re-issue/i)).toBeInTheDocument()
    // Start At datetime-local
    expect(screen.getByLabelText(/start at/i)).toBeInTheDocument()
    // End At datetime-local
    expect(screen.getByLabelText(/end at/i)).toBeInTheDocument()
  })

  test('renders submit and cancel buttons', () => {
    setupMocks()

    renderWithProviders(<Page />)

    expect(screen.getByRole('button', { name: /issue/i })).toBeInTheDocument()
    expect(screen.getByText('Cancel')).toBeInTheDocument()
  })

  describe('after successful issuance', () => {
    async function submitForm() {
      const user = userEvent.setup()
      setupMocksWithAutoSuccess()

      renderWithProviders(<Page />)

      await user.selectOptions(screen.getByLabelText(/product/i), 'SN-100')
      await user.type(screen.getByLabelText(/device id/i), 'device-007')
      await user.click(screen.getByRole('button', { name: /^issue$/i }))
    }

    test('shows success panel with heading and download buttons', async () => {
      await submitForm()

      expect(screen.getByText('Certificate Issued Successfully')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /download certificate/i })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /download private key/i })).toBeInTheDocument()
    })

    test('hides the form after success', async () => {
      await submitForm()

      expect(screen.queryByLabelText(/product/i)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(/device id/i)).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: /^issue$/i })).not.toBeInTheDocument()
    })

    test('shows one-time key download warning', async () => {
      await submitForm()

      expect(screen.getByText(/private key is shown only once/i)).toBeInTheDocument()
      expect(screen.getByText(/will not be stored on the server/i)).toBeInTheDocument()
    })

    function mockDownloadApi() {
      const mockClick = vi.fn()
      const mockAnchor = { href: '', download: '', click: mockClick, remove: vi.fn() }
      const createElementSpy = vi
        .spyOn(document, 'createElement')
        .mockReturnValue(mockAnchor as unknown as HTMLAnchorElement)
      const createObjectURLSpy = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fake')
      const revokeObjectURLSpy = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
      return {
        mockAnchor,
        restore() {
          createElementSpy.mockRestore()
          createObjectURLSpy.mockRestore()
          revokeObjectURLSpy.mockRestore()
        },
      }
    }

    async function submitAndGetUser() {
      const user = userEvent.setup()
      setupMocksWithAutoSuccess()
      renderWithProviders(<Page />)
      await user.selectOptions(screen.getByLabelText(/product/i), 'SN-100')
      await user.type(screen.getByLabelText(/device id/i), 'device-007')
      await user.click(screen.getByRole('button', { name: /^issue$/i }))
      return user
    }

    test('download certificate button triggers download with correct filename', async () => {
      const user = await submitAndGetUser()
      const dl = mockDownloadApi()

      await user.click(screen.getByRole('button', { name: /download certificate/i }))

      expect(dl.mockAnchor.download).toBe('device-007.pem')
      expect(dl.mockAnchor.click).toHaveBeenCalled()
      dl.restore()
    })

    test('download private key button triggers download with correct filename', async () => {
      const user = await submitAndGetUser()
      const dl = mockDownloadApi()

      await user.click(screen.getByRole('button', { name: /download private key/i }))

      expect(dl.mockAnchor.download).toBe('device-007.key')
      expect(dl.mockAnchor.click).toHaveBeenCalled()
      dl.restore()
    })

    test('Back to Certificates link points to /certs', async () => {
      await submitForm()

      const backLink = screen.getByText('Back to Certificates')
      expect(backLink).toBeInTheDocument()
      expect(backLink.closest('a')).toHaveAttribute('href', '/certs')
    })
  })
})
