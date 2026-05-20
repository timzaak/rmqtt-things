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
      ;(globalThis as Record<string, unknown>).__productsEditComponent = options.component
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

// Mock product hooks
const mockMutate = vi.fn()
const mockUseProduct = vi.fn()
const mockUseUpdateProduct = vi.fn()
vi.mock('@/hooks/useProducts', () => ({
  useProduct: (...args: unknown[]) => mockUseProduct(...args),
  useUpdateProduct: () => mockUseUpdateProduct(),
}))

// Import the module to trigger createRoute and capture the component
import '../edit.$id'

import { mockProduct } from '@/test/fixtures'

describe('ProductsEditPage', () => {
  const Page = (globalThis as Record<string, unknown>).__productsEditComponent as React.ComponentType

  test('shows loading state', () => {
    mockUseProduct.mockReturnValue({ data: undefined, isLoading: true })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  test('loads and displays product data', async () => {
    mockUseProduct.mockReturnValue({ data: mockProduct, isLoading: false })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Product')).toBeInTheDocument()
    })

    const nameInput = screen.getByLabelText(/name/i) as HTMLInputElement
    expect(nameInput.value).toBe('Sensor A')

    const modelInput = document.getElementById('model_no') as HTMLInputElement
    expect(modelInput.value).toBe('SN-100')

    const descInput = screen.getByLabelText(/description/i) as HTMLTextAreaElement
    expect(descInput.value).toBe('Temperature sensor')
  })

  test('model number field is disabled', async () => {
    mockUseProduct.mockReturnValue({ data: mockProduct, isLoading: false })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Product')).toBeInTheDocument()
    })

    const modelInput = document.getElementById('model_no') as HTMLInputElement
    expect(modelInput.disabled).toBe(true)
  })

  test('submit calls updateProduct mutation', async () => {
    const user = userEvent.setup()
    mockUseProduct.mockReturnValue({ data: mockProduct, isLoading: false })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Product')).toBeInTheDocument()
    })

    const nameInput = screen.getByLabelText(/name/i)
    await user.clear(nameInput)
    await user.type(nameInput, 'Updated Sensor')

    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      { id: 1, name: 'Updated Sensor', description: 'Temperature sensor', auto_provisioning: false },
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      }),
    )
  })

  test('auto_provisioning checkbox reflects product data', async () => {
    mockUseProduct.mockReturnValue({ data: mockProduct, isLoading: false })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Product')).toBeInTheDocument()
    })

    const checkbox = screen.getByRole('checkbox') as HTMLInputElement
    expect(checkbox.checked).toBe(false)
  })

  test('submit includes auto_provisioning in mutation payload', async () => {
    const user = userEvent.setup()
    const enabledProduct = { ...mockProduct, auto_provisioning: true }
    mockUseProduct.mockReturnValue({ data: enabledProduct, isLoading: false })
    mockUseUpdateProduct.mockReturnValue({ mutate: mockMutate, isPending: false })

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Product')).toBeInTheDocument()
    })

    const checkbox = screen.getByRole('checkbox') as HTMLInputElement
    expect(checkbox.checked).toBe(true)

    await user.click(checkbox)
    expect(checkbox.checked).toBe(false)

    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      { id: 1, name: 'Sensor A', description: 'Temperature sensor', auto_provisioning: false },
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      }),
    )
  })
})
