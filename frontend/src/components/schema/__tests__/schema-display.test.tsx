import { describe, test, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SchemaDisplay } from '../schema-display'

describe('SchemaDisplay', () => {
  test('renders null schema gracefully', () => {
    render(<SchemaDisplay schema={null} />)
    expect(screen.getByText('No schema defined.')).toBeInTheDocument()
  })

  test('renders undefined schema gracefully', () => {
    render(<SchemaDisplay schema={undefined} />)
    expect(screen.getByText('No schema defined.')).toBeInTheDocument()
  })

  test('renders flat schema with field names, types, required markers', () => {
    const schema = {
      type: 'object' as const,
      properties: {
        name: { type: 'string' as const },
        age: { type: 'number' as const },
        active: { type: 'boolean' as const },
      },
      required: ['name', 'age'],
    }

    render(<SchemaDisplay schema={schema} />)

    // Field names should be rendered
    expect(screen.getByText('name')).toBeInTheDocument()
    expect(screen.getByText('age')).toBeInTheDocument()
    expect(screen.getByText('active')).toBeInTheDocument()

    // Types should be rendered in uppercase
    expect(screen.getByText('STRING')).toBeInTheDocument()
    expect(screen.getByText('NUMBER')).toBeInTheDocument()
    expect(screen.getByText('BOOLEAN')).toBeInTheDocument()

    // Required markers: name and age are required, active is not
    // The required marker is a span with "*" after the field name
    const asterisks = screen.getAllByText('*')
    expect(asterisks).toHaveLength(2)
  })

  test('renders nested object schema with indentation', () => {
    const schema = {
      type: 'object' as const,
      properties: {
        address: {
          type: 'object' as const,
          properties: {
            street: { type: 'string' as const },
            city: { type: 'string' as const },
          },
        },
      },
    }

    render(<SchemaDisplay schema={schema} />)

    // Top-level field
    expect(screen.getByText('address')).toBeInTheDocument()

    // Nested fields
    expect(screen.getByText('street')).toBeInTheDocument()
    expect(screen.getByText('city')).toBeInTheDocument()

    // Check indentation: nested items have paddingLeft
    const streetItem = screen.getByText('street').closest('li')
    expect(streetItem).not.toBeNull()
    // Level 1 means paddingLeft = 1 * 20 = 20px
    expect(streetItem!.style.paddingLeft).toBe('20px')
  })

  test('renders array schema with items type', () => {
    const schema = {
      type: 'object' as const,
      properties: {
        tags: {
          type: 'array' as const,
          items: { type: 'string' as const },
        },
        users: {
          type: 'array' as const,
          items: {
            type: 'object' as const,
            properties: {
              id: { type: 'number' as const },
              username: { type: 'string' as const },
            },
          },
        },
      },
    }

    render(<SchemaDisplay schema={schema} />)

    // Array field names
    expect(screen.getByText('tags')).toBeInTheDocument()
    expect(screen.getByText('users')).toBeInTheDocument()

    // "Items:" label appears for array fields -- text is split across elements
    // so we match with a regex
    const itemsLabels = screen.getAllByText(/Items:/)
    expect(itemsLabels).toHaveLength(2)

    // Simple array items type: STRING is shown for tags items
    const stringTypes = screen.getAllByText('STRING')
    expect(stringTypes.length).toBeGreaterThanOrEqual(1)

    // Object array items: nested fields rendered
    expect(screen.getByText('id')).toBeInTheDocument()
    expect(screen.getByText('username')).toBeInTheDocument()
    expect(screen.getByText('NUMBER')).toBeInTheDocument()
  })

  test('renders validation limits for fields', () => {
    const schema = {
      type: 'object' as const,
      properties: {
        name: {
          type: 'string' as const,
          minLength: 1,
          maxLength: 100,
          pattern: '^[a-z]+$',
        },
        score: {
          type: 'number' as const,
          minimum: 0,
          maximum: 100,
        },
      },
    }

    render(<SchemaDisplay schema={schema} />)

    // String limits are rendered in a comma-separated string inside a single span
    // The limit text is: "MIN LENGTH: 1, MAX LENGTH: 100, PATTERN: ^[a-z]+$"
    const limitSpan = screen.getByText(/MIN LENGTH: 1/)
    expect(limitSpan.textContent).toContain('MAX LENGTH: 100')
    expect(limitSpan.textContent).toContain('PATTERN: ^[a-z]+$')

    // Number limits
    const numberLimitSpan = screen.getByText(/MIN: 0/)
    expect(numberLimitSpan.textContent).toContain('MAX: 100')
  })
})
