import { useState } from 'react'
import { ArrowDown, ArrowUp, Braces, ChevronDown, ChevronRight, Plus, Trash2 } from 'lucide-react'

export type JSONSchema = {
  type: string
  properties?: Record<string, JSONSchema>
  items?: JSONSchema
  required?: string[]
  description?: string
  minimum?: number
  maximum?: number
  minLength?: number
  maxLength?: number
  pattern?: string
}

const FIELD_TYPES = ['string', 'number', 'boolean', 'object', 'array'] as const

const inputClass =
  'h-9 w-full rounded-md border border-slate-300 bg-white px-3 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-100 dark:focus:border-slate-400 dark:focus:ring-slate-700'

const selectClass =
  'h-9 w-full rounded-md border border-slate-300 bg-white px-3 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-100 dark:focus:border-slate-400 dark:focus:ring-slate-700'

const disabledInputClass =
  'h-9 w-full rounded-md border border-slate-200 bg-slate-50 px-3 text-sm text-slate-500 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400'

const labelClass =
  'mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400'

const cardClass =
  'rounded-md border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-900'

const addButtonClass =
  'inline-flex h-9 items-center gap-1.5 rounded-md border border-dashed border-slate-300 px-3 text-sm font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800'

const iconButtonClass =
  'inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 disabled:cursor-not-allowed disabled:opacity-35 dark:hover:bg-slate-800 dark:hover:text-slate-200'

const removeButtonClass =
  'inline-flex h-8 w-8 items-center justify-center rounded-md text-red-600 transition hover:bg-red-50 hover:text-red-700 disabled:cursor-not-allowed disabled:opacity-40 dark:text-red-400 dark:hover:bg-red-950/40 dark:hover:text-red-300'

const typeBadgeClass =
  'rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5 text-xs font-medium uppercase text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300'

function RequiredToggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition disabled:cursor-not-allowed disabled:opacity-50 ${
        checked ? 'bg-slate-900 dark:bg-slate-100' : 'bg-slate-300 dark:bg-slate-700'
      }`}
      onClick={() => onChange(!checked)}
      disabled={disabled}
      aria-pressed={checked}
    >
      <span
        className={`mt-0.5 h-4 w-4 rounded-full bg-white shadow transition dark:bg-slate-900 ${
          checked ? 'translate-x-4' : 'translate-x-0.5'
        }`}
      />
    </button>
  )
}

function numericValue(value: string): number | undefined {
  return value === '' ? undefined : Number(value)
}

function FieldEditor({
  fieldKey,
  schema,
  onChange,
  onRemove,
  onKeyChange,
  isRequired,
  onRequiredChange,
  onMove,
  disabled,
}: {
  fieldKey?: string
  schema: JSONSchema
  onChange: (s: JSONSchema) => void
  onRemove?: () => void
  onKeyChange?: (k: string) => void
  isRequired?: boolean
  onRequiredChange?: (required: boolean) => void
  onMove?: (direction: 'up' | 'down') => void
  disabled?: boolean
}) {
  const [expanded, setExpanded] = useState(true)

  const propertyKeys = Object.keys(schema.properties ?? {})
  const requiredFields = schema.required ?? []
  const isNested = fieldKey !== undefined
  const isContainer = schema.type === 'object' || schema.type === 'array'

  const handleTypeChange = (type: string) => {
    const updated: JSONSchema = { type }
    if (type === 'object') updated.properties = {}
    if (type === 'array') updated.items = { type: 'string' }
    onChange(updated)
  }

  const moveChild = (key: string, direction: 'up' | 'down') => {
    const entries = Object.entries(schema.properties ?? {})
    const index = entries.findIndex(([name]) => name === key)
    const target = direction === 'up' ? index - 1 : index + 1
    if (index < 0 || target < 0 || target >= entries.length) return

    const nextEntries = [...entries]
    ;[nextEntries[index], nextEntries[target]] = [nextEntries[target], nextEntries[index]]
    onChange({ ...schema, properties: Object.fromEntries(nextEntries) })
  }

  const updateRequired = (key: string, required: boolean) => {
    const nextRequired = required
      ? Array.from(new Set([...requiredFields, key]))
      : requiredFields.filter((name) => name !== key)
    onChange({ ...schema, required: nextRequired.length > 0 ? nextRequired : undefined })
  }

  if (!isNested) {
    return (
      <div>
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="mb-4 inline-flex items-center gap-2 text-left"
        >
          {expanded ? (
            <ChevronDown className="h-5 w-5 text-slate-500" />
          ) : (
            <ChevronRight className="h-5 w-5 text-slate-500" />
          )}
          <span className="text-base font-semibold text-slate-900 dark:text-slate-100">
            Root Object
          </span>
          <span className={typeBadgeClass}>object</span>
        </button>

        {expanded && (
          <div className="flex">
            <div className="ml-2 w-0.5 shrink-0 rounded-full bg-slate-300 dark:bg-slate-700" />
            <div className="min-w-0 flex-1 space-y-3 pl-6">
              <ObjectChildren
                schema={schema}
                onChange={onChange}
                disabled={disabled}
                moveChild={moveChild}
                updateRequired={updateRequired}
                propertyKeys={propertyKeys}
                requiredFields={requiredFields}
              />
            </div>
          </div>
        )}
      </div>
    )
  }

  return (
    <div className={cardClass}>
      <div className={isContainer ? 'p-3' : 'p-4'}>
        <div className="flex items-start gap-3">
          <div className="flex shrink-0 flex-col gap-1 pt-5">
            <button
              type="button"
              onClick={() => onMove?.('up')}
              disabled={disabled || !onMove}
              className={iconButtonClass}
              title="Move up"
            >
              <ArrowUp className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={() => onMove?.('down')}
              disabled={disabled || !onMove}
              className={iconButtonClass}
              title="Move down"
            >
              <ArrowDown className="h-4 w-4" />
            </button>
          </div>

          {isContainer && (
            <button
              type="button"
              onClick={() => setExpanded((value) => !value)}
              className={`${iconButtonClass} mt-5`}
              title={expanded ? 'Collapse field' : 'Expand field'}
            >
              {expanded ? (
                <ChevronDown className="h-5 w-5" />
              ) : (
                <ChevronRight className="h-5 w-5" />
              )}
            </button>
          )}

          <div className="min-w-0 flex-1">
            <div className="grid gap-3 lg:grid-cols-[minmax(120px,1.4fr)_minmax(120px,0.9fr)_120px_minmax(160px,1.8fr)_auto]">
              <div>
                <label className={labelClass}>Name</label>
                <input
                  type="text"
                  defaultValue={fieldKey}
                  onBlur={(e) => onKeyChange?.(e.target.value)}
                  disabled={disabled}
                  className={disabled ? disabledInputClass : inputClass}
                  data-testid="schema-field-name-input"
                />
              </div>
              <div>
                <label className={labelClass}>Type</label>
                <select
                  value={schema.type}
                  onChange={(e) => handleTypeChange(e.target.value)}
                  disabled={disabled}
                  className={disabled ? disabledInputClass : selectClass}
                  data-testid="schema-field-type-select"
                >
                  {FIELD_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {t}
                    </option>
                  ))}
                </select>
              </div>
              <div className="flex items-center gap-2 pt-6">
                <span className="text-sm font-medium text-slate-700 dark:text-slate-300">
                  Required
                </span>
                <RequiredToggle
                  checked={Boolean(isRequired)}
                  onChange={(required) => onRequiredChange?.(required)}
                  disabled={disabled || !onRequiredChange}
                />
              </div>
              <div>
                <label className={labelClass}>Description</label>
                <input
                  type="text"
                  value={schema.description ?? ''}
                  onChange={(e) => onChange({ ...schema, description: e.target.value })}
                  disabled={disabled}
                  placeholder="Optional description"
                  className={disabled ? disabledInputClass : inputClass}
                  data-testid="schema-field-description-input"
                />
              </div>
              <div className="flex justify-end pt-6">
                {onRemove && (
                  <button
                    type="button"
                    onClick={onRemove}
                    disabled={disabled}
                    className={removeButtonClass}
                    data-testid="schema-field-remove-button"
                    aria-label={`Remove ${fieldKey}`}
                    title="Remove field"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                )}
              </div>
            </div>

            {schema.type === 'string' && (
              <div className="mt-3 grid gap-3 border-t border-slate-100 pt-3 sm:grid-cols-3 dark:border-slate-800">
                <div>
                  <label className={labelClass}>Min Length</label>
                  <input
                    type="number"
                    value={schema.minLength ?? ''}
                    onChange={(e) =>
                      onChange({ ...schema, minLength: numericValue(e.target.value) })
                    }
                    disabled={disabled}
                    className={disabled ? disabledInputClass : inputClass}
                    data-testid="schema-field-minlength-input"
                  />
                </div>
                <div>
                  <label className={labelClass}>Max Length</label>
                  <input
                    type="number"
                    value={schema.maxLength ?? ''}
                    onChange={(e) =>
                      onChange({ ...schema, maxLength: numericValue(e.target.value) })
                    }
                    disabled={disabled}
                    className={disabled ? disabledInputClass : inputClass}
                    data-testid="schema-field-maxlength-input"
                  />
                </div>
                <div>
                  <label className={labelClass}>Pattern</label>
                  <input
                    type="text"
                    value={schema.pattern ?? ''}
                    onChange={(e) => onChange({ ...schema, pattern: e.target.value || undefined })}
                    disabled={disabled}
                    placeholder="regex"
                    className={disabled ? disabledInputClass : inputClass}
                    data-testid="schema-field-pattern-input"
                  />
                </div>
              </div>
            )}

            {schema.type === 'number' && (
              <div className="mt-3 grid gap-3 border-t border-slate-100 pt-3 sm:max-w-md sm:grid-cols-2 dark:border-slate-800">
                <div>
                  <label className={labelClass}>Minimum</label>
                  <input
                    type="number"
                    value={schema.minimum ?? ''}
                    onChange={(e) => onChange({ ...schema, minimum: numericValue(e.target.value) })}
                    disabled={disabled}
                    className={disabled ? disabledInputClass : inputClass}
                    data-testid="schema-field-minimum-input"
                  />
                </div>
                <div>
                  <label className={labelClass}>Maximum</label>
                  <input
                    type="number"
                    value={schema.maximum ?? ''}
                    onChange={(e) => onChange({ ...schema, maximum: numericValue(e.target.value) })}
                    disabled={disabled}
                    className={disabled ? disabledInputClass : inputClass}
                    data-testid="schema-field-maximum-input"
                  />
                </div>
              </div>
            )}
          </div>
        </div>

        {schema.type === 'object' && expanded && (
          <div className="mt-4 flex">
            <div className="ml-12 w-0.5 shrink-0 rounded-full bg-slate-300 dark:bg-slate-700" />
            <div className="min-w-0 flex-1 space-y-3 pl-6">
              <ObjectChildren
                schema={schema}
                onChange={onChange}
                disabled={disabled}
                moveChild={moveChild}
                updateRequired={updateRequired}
                propertyKeys={propertyKeys}
                requiredFields={requiredFields}
              />
            </div>
          </div>
        )}

        {schema.type === 'array' && schema.items && expanded && (
          <div className="mt-4 flex">
            <div className="ml-12 w-0.5 shrink-0 rounded-full bg-slate-300 dark:bg-slate-700" />
            <div className="min-w-0 flex-1 pl-6">
              <FieldEditor
                fieldKey="items"
                schema={schema.items}
                disabled={disabled}
                onChange={(s) => onChange({ ...schema, items: s })}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function ObjectChildren({
  schema,
  onChange,
  disabled,
  moveChild,
  updateRequired,
  propertyKeys,
  requiredFields,
}: {
  schema: JSONSchema
  onChange: (s: JSONSchema) => void
  disabled?: boolean
  moveChild: (key: string, direction: 'up' | 'down') => void
  updateRequired: (key: string, required: boolean) => void
  propertyKeys: string[]
  requiredFields: string[]
}) {
  return (
    <>
      <select
        multiple
        value={requiredFields}
        onChange={(e) => {
          const selected = Array.from(e.target.selectedOptions, (o) => o.value)
          onChange({ ...schema, required: selected.length > 0 ? selected : undefined })
        }}
        disabled={disabled}
        className="sr-only"
        data-testid="schema-field-required-select"
      >
        {propertyKeys.map((k) => (
          <option key={k} value={k}>
            {k}
          </option>
        ))}
      </select>

      <button
        type="button"
        onClick={() => {
          const name = 'field' + (Object.keys(schema.properties ?? {}).length + 1)
          const newProps = { ...(schema.properties ?? {}), [name]: { type: 'string' } }
          onChange({ ...schema, properties: newProps })
        }}
        disabled={disabled}
        className={addButtonClass}
        data-testid="schema-add-child-button"
      >
        <Plus className="h-4 w-4" />
        Add Child Field
      </button>

      {schema.properties && propertyKeys.length > 0 ? (
        Object.entries(schema.properties).map(([k, v]) => (
          <FieldEditor
            key={k}
            fieldKey={k}
            schema={v}
            disabled={disabled}
            isRequired={requiredFields.includes(k)}
            onRequiredChange={(required) => updateRequired(k, required)}
            onMove={(direction) => moveChild(k, direction)}
            onChange={(s) => {
              const newProps = { ...(schema.properties ?? {}), [k]: s }
              onChange({ ...schema, properties: newProps })
            }}
            onKeyChange={(newKey) => {
              const nextKey = newKey.trim()
              if (!nextKey || nextKey === k) return

              const newProps: Record<string, JSONSchema> = {}
              for (const key in schema.properties) {
                if (key === k) {
                  newProps[nextKey] = schema.properties[key]
                } else {
                  newProps[key] = schema.properties[key]
                }
              }
              const newRequired = schema.required?.map((r) => (r === k ? nextKey : r))
              onChange({ ...schema, properties: newProps, required: newRequired })
            }}
            onRemove={() => {
              const newProps = { ...(schema.properties ?? {}) }
              delete newProps[k]
              const newRequired = schema.required?.filter((r) => r !== k)
              onChange({
                ...schema,
                properties: newProps,
                required: newRequired && newRequired.length > 0 ? newRequired : undefined,
              })
            }}
          />
        ))
      ) : (
        <div className="rounded-md border border-dashed border-slate-300 px-4 py-6 text-center text-sm text-slate-500 dark:border-slate-700 dark:text-slate-400">
          No child fields yet.
        </div>
      )}
    </>
  )
}

export function SchemaEditor({
  value,
  onChange,
  disabled,
}: {
  value?: JSONSchema
  onChange?: (s: JSONSchema) => void
  disabled?: boolean
}) {
  const [internalSchema, setInternalSchema] = useState<JSONSchema>({
    type: 'object',
    properties: {},
  })

  const currentSchema = value ?? internalSchema

  const handleChange = (s: JSONSchema) => {
    setInternalSchema(s)
    onChange?.(s)
  }

  return (
    <div
      className="rounded-md border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-700 dark:bg-slate-900"
      data-testid="schema-editor"
    >
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Braces className="h-5 w-5 text-slate-500" />
            <h4 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
              Schema Fields
            </h4>
          </div>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
            Edit the event payload as a nested field tree.
          </p>
        </div>
        {disabled && (
          <span className="rounded-full bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
            Read only
          </span>
        )}
      </div>
      <FieldEditor schema={currentSchema} onChange={handleChange} disabled={disabled} />
    </div>
  )
}
