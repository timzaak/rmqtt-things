import { describe, test, expect, vi } from 'vitest'
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { render } from '@testing-library/react'
import { useState } from 'react'
import { SchemaEditor, type JSONSchema } from '../schema-editor'

/** Wrapper that manages controlled state so the component re-renders on onChange. */
function StatefulEditor({ initial, disabled }: { initial?: JSONSchema; disabled?: boolean }) {
  const [schema, setSchema] = useState<JSONSchema | undefined>(initial)
  return <SchemaEditor value={schema} onChange={setSchema} disabled={disabled} />
}

describe('SchemaEditor', () => {
  test('renders with default empty schema', () => {
    render(<SchemaEditor />)
    expect(screen.getByTestId('schema-editor')).toBeInTheDocument()
  })

  test('adding a field updates the schema value', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()

    const schema: JSONSchema = {
      type: 'object',
      properties: {
        field1: { type: 'string' },
      },
    }

    render(<SchemaEditor value={schema} onChange={onChange} />)

    const addButton = screen.getByTestId('schema-add-child-button')
    await user.click(addButton)

    expect(onChange).toHaveBeenCalledTimes(1)
    const calledSchema = onChange.mock.calls[0][0]
    expect(calledSchema.properties).toHaveProperty('field2')
    expect(calledSchema.properties.field2).toEqual({ type: 'string' })
  })

  test('changing field type from string to number hides string validation, shows number validation', async () => {
    const user = userEvent.setup()

    const schema: JSONSchema = {
      type: 'object',
      properties: {
        temperature: { type: 'string' },
      },
    }

    render(<StatefulEditor initial={schema} />)

    // String validations should be visible for the field
    expect(screen.getByTestId('schema-field-minlength-input')).toBeInTheDocument()
    expect(screen.getByTestId('schema-field-maxlength-input')).toBeInTheDocument()
    expect(screen.getByTestId('schema-field-pattern-input')).toBeInTheDocument()

    // Change type to number
    const typeSelect = screen.getByTestId('schema-field-type-select')
    await user.selectOptions(typeSelect, 'number')

    // Number validations should now be visible
    await waitFor(() => {
      expect(screen.getByTestId('schema-field-minimum-input')).toBeInTheDocument()
    })
    expect(screen.getByTestId('schema-field-maximum-input')).toBeInTheDocument()

    // String validations should no longer be present
    expect(screen.queryByTestId('schema-field-minlength-input')).toBeNull()
    expect(screen.queryByTestId('schema-field-maxlength-input')).toBeNull()
    expect(screen.queryByTestId('schema-field-pattern-input')).toBeNull()
  })

  test('object type: add child field and set required fields', async () => {
    const user = userEvent.setup()

    const schema: JSONSchema = {
      type: 'object',
      properties: {
        parent: {
          type: 'object',
          properties: {},
        },
      },
    }

    render(<StatefulEditor initial={schema} />)

    // There are two "Add Child Field" buttons: one for root, one for nested parent
    const addButtons = screen.getAllByTestId('schema-add-child-button')
    // Click the second one (nested parent)
    await user.click(addButtons[1])

    // The parent object should now have a child field named "field1"
    // "field1" appears both as a span label and as an option in the required-select,
    // so use getAllByText and verify at least one match.
    await waitFor(() => {
      const field1Elements = screen.getAllByText('field1')
      expect(field1Elements.length).toBeGreaterThanOrEqual(1)
    })

    // Required select should exist for the parent object
    const requiredSelects = screen.getAllByTestId('schema-field-required-select')
    expect(requiredSelects.length).toBeGreaterThanOrEqual(1)
  })

  test('disabled prop prevents editing', () => {
    const schema: JSONSchema = {
      type: 'object',
      properties: {
        name: { type: 'string' },
      },
    }

    render(<SchemaEditor value={schema} disabled={true} />)

    // All interactive elements should be disabled
    const typeSelect = screen.getByTestId('schema-field-type-select')
    expect(typeSelect).toBeDisabled()

    const nameInput = screen.getByTestId('schema-field-name-input')
    expect(nameInput).toBeDisabled()

    const descriptionInput = screen.getByTestId('schema-field-description-input')
    expect(descriptionInput).toBeDisabled()

    const minLengthInput = screen.getByTestId('schema-field-minlength-input')
    expect(minLengthInput).toBeDisabled()

    // Add child button should be disabled
    const addButton = screen.getByTestId('schema-add-child-button')
    expect(addButton).toBeDisabled()

    // Remove button should be disabled
    const removeButton = screen.getByTestId('schema-field-remove-button')
    expect(removeButton).toBeDisabled()
  })
})
