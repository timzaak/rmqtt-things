import { describe, test, expect, vi } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type { ActionInvocationView } from '@/lib/api-generated/types.gen'

// Hook mocks: the section never touches the network in tests. Each mock keeps
// the `(...args) => mockFn(...args)` forwarding shape used by show.test.tsx so
// per-test `mockReturnValue` overrides take effect.
const mockUseActionInvocations = vi.fn()
const mockUseCreateActionInvocation = vi.fn()
const mockUseDeleteActionInvocations = vi.fn()

vi.mock('@/hooks/useActionInvocations', () => ({
  useActionInvocations: (...args: unknown[]) => mockUseActionInvocations(...args),
  useCreateActionInvocation: () => mockUseCreateActionInvocation(),
  useDeleteActionInvocations: () => mockUseDeleteActionInvocations(),
}))

import { ActionInvocationsSection } from '../ActionInvocationsSection'

function makeInvocation(overrides: Partial<ActionInvocationView> = {}): ActionInvocationView {
  return {
    id: 1,
    serviceType: 'reboot',
    params: { delaySeconds: 5 },
    status: 'Pending',
    createdTime: '2025-01-01T10:00:00Z',
    updatedTime: '2025-01-01T10:00:00Z',
    ...overrides,
  }
}

function setupMocks() {
  mockUseActionInvocations.mockReturnValue({
    data: { data: [], pagination: undefined },
    isLoading: false,
  })
  mockUseCreateActionInvocation.mockReturnValue({ mutate: vi.fn(), isPending: false })
  mockUseDeleteActionInvocations.mockReturnValue({ mutate: vi.fn(), isPending: false })
}

describe('ActionInvocationsSection', () => {
  test('renders section heading, invoke button and table testid', () => {
    setupMocks()
    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    expect(screen.getByRole('heading', { name: 'Action Invocations' })).toBeInTheDocument()
    expect(screen.getByTestId('action-invoke-button')).toBeInTheDocument()
    expect(screen.getByTestId('action-invocation-table')).toBeInTheDocument()
  })

  test('renders empty message when there are no invocations', () => {
    setupMocks()
    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    expect(screen.getByText('No action invocations')).toBeInTheDocument()
  })

  test('renders invocation rows using camelCase view fields', () => {
    setupMocks()
    mockUseActionInvocations.mockReturnValue({
      data: {
        data: [
          makeInvocation({
            id: 7,
            serviceType: 'unlock',
            params: { token: 'abc' },
            status: 'Success',
            createdTime: '2025-01-02T03:04:00Z',
            updatedTime: '2025-01-02T03:05:00Z',
          }),
        ],
        pagination: { page: 1, page_size: 10, total: 1 },
      },
      isLoading: false,
    })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    expect(screen.getByText('7')).toBeInTheDocument()
    expect(screen.getByText('unlock')).toBeInTheDocument()
    // Params render inside a <pre> with pretty-printed JSON.
    expect(screen.getByText(/token/)).toBeInTheDocument()
    // Status text is rendered for the Success row.
    expect(screen.getByText('Success')).toBeInTheDocument()
  })

  test('shows Delete only on Pending rows and triggers delete mutation on click', async () => {
    const user = userEvent.setup()
    const deleteMutate = vi.fn()
    setupMocks()
    mockUseDeleteActionInvocations.mockReturnValue({ mutate: deleteMutate, isPending: false })
    mockUseActionInvocations.mockReturnValue({
      data: {
        data: [
          makeInvocation({ id: 1, status: 'Pending' }),
          makeInvocation({ id: 2, status: 'Success' }),
        ],
        pagination: undefined,
      },
      isLoading: false,
    })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    // Exactly one Delete button (only the Pending row).
    const deleteButtons = screen.getAllByRole('button', { name: 'Delete' })
    expect(deleteButtons).toHaveLength(1)

    await user.click(deleteButtons[0])

    expect(deleteMutate).toHaveBeenCalledWith([1])
  })

  test('opens the invoke dialog when Invoke Action is clicked', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    expect(screen.queryByTestId('action-invoke-dialog')).not.toBeInTheDocument()

    await user.click(screen.getByTestId('action-invoke-button'))

    expect(screen.getByTestId('action-invoke-dialog')).toBeInTheDocument()
    expect(screen.getByTestId('service-type-input')).toBeInTheDocument()
    expect(screen.getByTestId('params-input')).toBeInTheDocument()
    expect(screen.getByTestId('submit-button')).toBeInTheDocument()
    expect(screen.getByTestId('cancel-button')).toBeInTheDocument()
  })

  test('closes the dialog on Cancel', async () => {
    const user = userEvent.setup()
    setupMocks()

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))
    await user.click(screen.getByTestId('cancel-button'))

    expect(screen.queryByTestId('action-invoke-dialog')).not.toBeInTheDocument()
  })

  test('rejects invalid serviceType with inline error and does not call create', async () => {
    const user = userEvent.setup()
    const createMutate = vi.fn()
    setupMocks()
    mockUseCreateActionInvocation.mockReturnValue({ mutate: createMutate, isPending: false })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))

    // "/" is not in the allowed charset.
    const input = screen.getByTestId('service-type-input')
    fireEvent.change(input, { target: { value: 're/boot' } })
    await user.click(screen.getByTestId('submit-button'))

    expect(
      screen.getByText('Allowed: letters, digits, "_" and "-", 1-32 chars')
    ).toBeInTheDocument()
    expect(createMutate).not.toHaveBeenCalled()

    // A value containing a space is also rejected.
    fireEvent.change(input, { target: { value: 're boot' } })
    await user.click(screen.getByTestId('submit-button'))
    expect(createMutate).not.toHaveBeenCalled()

    // A 33-character value is rejected (max length 32).
    const longValue = 'a'.repeat(33)
    fireEvent.change(input, { target: { value: longValue } })
    await user.click(screen.getByTestId('submit-button'))
    expect(createMutate).not.toHaveBeenCalled()
  })

  test('shows Invalid JSON for malformed params and does not call create', async () => {
    const user = userEvent.setup()
    const createMutate = vi.fn()
    setupMocks()
    mockUseCreateActionInvocation.mockReturnValue({ mutate: createMutate, isPending: false })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))

    fireEvent.change(screen.getByTestId('service-type-input'), {
      target: { value: 'reboot' },
    })
    fireEvent.change(screen.getByTestId('params-input'), {
      target: { value: 'not valid json' },
    })
    await user.click(screen.getByTestId('submit-button'))

    expect(screen.getByText('Invalid JSON')).toBeInTheDocument()
    expect(createMutate).not.toHaveBeenCalled()
  })

  test('submits valid request with parsed params and closes dialog on success', async () => {
    const user = userEvent.setup()
    const createMutate = vi.fn()
    setupMocks()
    mockUseCreateActionInvocation.mockReturnValue({ mutate: createMutate, isPending: false })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))

    fireEvent.change(screen.getByTestId('service-type-input'), {
      target: { value: 'reboot' },
    })
    fireEvent.change(screen.getByTestId('params-input'), {
      target: { value: '{"delaySeconds": 5}' },
    })
    await user.click(screen.getByTestId('submit-button'))

    expect(createMutate).toHaveBeenCalledTimes(1)
    expect(createMutate).toHaveBeenCalledWith(
      {
        productId: 'product-a',
        deviceId: 'device-001',
        serviceType: 'reboot',
        params: { delaySeconds: 5 },
      },
      expect.objectContaining({ onSuccess: expect.any(Function) })
    )

    // Drive the registered onSuccess: the section closes the dialog when the
    // create mutation reports success.
    const onSuccess = createMutate.mock.calls[0][1].onSuccess as () => void
    onSuccess()

    await waitFor(() => {
      expect(screen.queryByTestId('action-invoke-dialog')).not.toBeInTheDocument()
    })
  })

  test('omits params field when params box is left empty', async () => {
    const user = userEvent.setup()
    const createMutate = vi.fn()
    setupMocks()
    mockUseCreateActionInvocation.mockReturnValue({ mutate: createMutate, isPending: false })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))

    fireEvent.change(screen.getByTestId('service-type-input'), {
      target: { value: 'unlock' },
    })
    // params left empty on purpose
    await user.click(screen.getByTestId('submit-button'))

    expect(createMutate).toHaveBeenCalledWith(
      {
        productId: 'product-a',
        deviceId: 'device-001',
        serviceType: 'unlock',
      },
      expect.objectContaining({ onSuccess: expect.any(Function) })
    )
  })

  test('submit button is disabled while a create mutation is pending', async () => {
    const user = userEvent.setup()
    setupMocks()
    mockUseCreateActionInvocation.mockReturnValue({ mutate: vi.fn(), isPending: true })

    renderWithProviders(<ActionInvocationsSection productId="product-a" deviceId="device-001" />)

    await user.click(screen.getByTestId('action-invoke-button'))

    const submit = screen.getByTestId('submit-button') as HTMLButtonElement
    expect(submit).toBeDisabled()
  })
})
