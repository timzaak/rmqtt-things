import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import { mockDraftValidTemplate, mockActiveValidTemplate } from '@/test/fixtures'

const mockNavigate = vi.fn()
vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRoute: (options: { component?: React.ComponentType }) => {
      ;(globalThis as Record<string, unknown>).__validTemplatesEditComponent = options.component
      return {
        options,
        useParams: () => ({ id: '1' }),
      }
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

// Mock event hooks
const mockMutate = vi.fn()
const mockMutateStatus = vi.fn()
const mockUseEventValidTemplate = vi.fn()
const mockUseUpdateEventValidTemplate = vi.fn()
const mockUseUpdateEventValidTemplateStatus = vi.fn()
vi.mock('@/hooks/useEvents', () => ({
  useEventValidTemplate: (...args: unknown[]) => mockUseEventValidTemplate(...args),
  useUpdateEventValidTemplate: () => mockUseUpdateEventValidTemplate(),
  useUpdateEventValidTemplateStatus: () => mockUseUpdateEventValidTemplateStatus(),
}))

// Import the module to trigger createRoute and capture the component
import '../edit.$id'

describe('ValidTemplatesEditPage', () => {
  const Page = (globalThis as Record<string, unknown>)
    .__validTemplatesEditComponent as React.ComponentType

  function setupMocks(template: typeof mockDraftValidTemplate) {
    mockUseEventValidTemplate.mockReturnValue({ data: template, isLoading: false })
    mockUseUpdateEventValidTemplate.mockReturnValue({ mutate: mockMutate, isPending: false })
    mockUseUpdateEventValidTemplateStatus.mockReturnValue({
      mutate: mockMutateStatus,
      isPending: false,
    })
  }

  test('shows loading state', () => {
    mockUseEventValidTemplate.mockReturnValue({ data: undefined, isLoading: true })
    mockUseUpdateEventValidTemplate.mockReturnValue({ mutate: mockMutate, isPending: false })
    mockUseUpdateEventValidTemplateStatus.mockReturnValue({
      mutate: mockMutateStatus,
      isPending: false,
    })

    renderWithProviders(<Page />)

    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  test('loads and displays Draft template data with editable schema', async () => {
    setupMocks(mockDraftValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    // No active notice banner for Draft
    expect(screen.queryByTestId('template-edit-active-notice')).not.toBeInTheDocument()

    // Product and event are disabled
    const productInput = document.getElementById('productId') as HTMLInputElement
    expect(productInput.disabled).toBe(true)

    const eventInput = document.getElementById('event') as HTMLInputElement
    expect(eventInput.disabled).toBe(true)

    // Status select present and set to Draft
    const statusSelect = screen.getByTestId('template-edit-status-select') as HTMLSelectElement
    expect(statusSelect.value).toBe('Draft')

    // Description is editable
    const descInput = screen.getByTestId('template-edit-description-input') as HTMLTextAreaElement
    expect(descInput.value).toBe('Temperature reading schema')
  })

  test('P1-13: Active template schema is disabled (readonly)', async () => {
    setupMocks(mockActiveValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    // SchemaEditor should have the "Read only" badge when disabled
    expect(screen.getByText('Read only')).toBeInTheDocument()
    // "Add Child Field" button should be disabled
    const addChildBtn = screen.getByTestId('schema-add-child-button') as HTMLButtonElement
    expect(addChildBtn.disabled).toBe(true)
  })

  test('P1-14: status select dropdown has all three options', async () => {
    setupMocks(mockDraftValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    const statusSelect = screen.getByTestId('template-edit-status-select') as HTMLSelectElement
    const options = Array.from(statusSelect.options).map((o) => o.value)
    expect(options).toEqual(['Draft', 'Active', 'Inactive'])
  })

  test('P1-15: Active template shows notice banner instead of redirecting', async () => {
    setupMocks(mockActiveValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    const notice = screen.getByTestId('template-edit-active-notice')
    expect(notice).toBeInTheDocument()
    expect(notice.textContent).toContain('Active')
    expect(notice.textContent).toContain('read-only')
  })

  test('submit with changed status calls status mutation', async () => {
    const user = userEvent.setup()
    setupMocks(mockDraftValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    const statusSelect = screen.getByTestId('template-edit-status-select')
    await user.selectOptions(statusSelect, 'Active')

    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(mockMutateStatus).toHaveBeenCalledWith(
      { id: 1, status: 'Active' },
      expect.objectContaining({
        onError: expect.any(Function),
      })
    )
  })

  test('submit with changed content calls update mutation', async () => {
    const user = userEvent.setup()
    setupMocks(mockDraftValidTemplate)

    renderWithProviders(<Page />)

    await waitFor(() => {
      expect(screen.getByText('Edit Template')).toBeInTheDocument()
    })

    const descInput = screen.getByTestId('template-edit-description-input')
    await user.clear(descInput)
    await user.type(descInput, 'Updated description')

    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 1,
        description: 'Updated description',
      }),
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      })
    )
  })
})
