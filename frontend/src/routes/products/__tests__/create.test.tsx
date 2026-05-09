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
      ;(globalThis as Record<string, unknown>).__productsCreateComponent = options.component
      return { options }
    },
    Link: ({ to, children, ...props }: { to: string; children: React.ReactNode; [k: string]: unknown }) => (
      <a href={to} {...props}>{children}</a>
    ),
    useNavigate: () => mockNavigate,
    useBlocker: () => ({ status: 'idle' }),
  }
})

// Mock useCreateProduct hook
const mockMutate = vi.fn()
const mockUseCreateProduct = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useCreateProduct: () => mockUseCreateProduct(),
}))

// Import the module to trigger createRoute and capture the component
import '../create'

describe('ProductsCreatePage', () => {
  const Page = (globalThis as Record<string, unknown>).__productsCreateComponent as React.ComponentType

  test('renders form fields', () => {
    mockUseCreateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('Create Product')).toBeInTheDocument()
    expect(screen.getByLabelText(/name/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/model number/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/description/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /create/i })).toBeInTheDocument()
  })

  test('validates required fields on submit', async () => {
    const user = userEvent.setup()
    mockUseCreateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    const submitButton = screen.getByRole('button', { name: /create/i })
    await user.click(submitButton)

    expect(mockMutate).not.toHaveBeenCalled()
  })

  test('submit calls createProduct mutation with form values', async () => {
    const user = userEvent.setup()
    mockUseCreateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await user.type(screen.getByLabelText(/name/i), 'New Sensor')
    await user.type(screen.getByLabelText(/model number/i), 'NS-001')
    await user.type(screen.getByLabelText(/description/i), 'A new sensor device')
    await user.click(screen.getByRole('button', { name: /create/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      { name: 'New Sensor', model_no: 'NS-001', description: 'A new sensor device' },
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      }),
    )
  })
})
