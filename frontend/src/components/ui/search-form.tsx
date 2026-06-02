import { useState, type FormEvent } from 'react'
import { Search } from 'lucide-react'

export interface SearchField {
  name: string
  label: string
  type?: 'text' | 'select'
  options?: { label: string; value: string }[]
  placeholder?: string
}

interface SearchFormProps {
  fields: SearchField[]
  onSearch: (values: Record<string, string>) => void
}

const inputStyle = {
  height: '34px',
  borderRadius: '8px',
  border: '1px solid var(--color-border)',
  background: 'var(--color-surface-1)',
  color: 'var(--color-text-primary)',
  padding: '0 12px',
  fontSize: '13px',
  outline: 'none',
} as React.CSSProperties

export function SearchForm({ fields, onSearch }: SearchFormProps) {
  const initialValues: Record<string, string> = {}
  for (const f of fields) {
    initialValues[f.name] = ''
  }
  const [values, setValues] = useState(initialValues)

  function handleSubmit(e: FormEvent) {
    e.preventDefault()
    onSearch(values)
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-wrap items-end gap-3 pb-4">
      {fields.map((field) => (
        <div key={field.name} className="flex flex-col gap-1.5">
          <label
            className="text-[11px] font-semibold uppercase tracking-wider"
            style={{ color: 'var(--color-text-muted)' }}
          >
            {field.label}
          </label>
          {field.type === 'select' ? (
            <select
              value={values[field.name]}
              onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              aria-label={field.label}
              style={inputStyle}
            >
              <option value="">All</option>
              {field.options?.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              value={values[field.name]}
              onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              placeholder={field.placeholder}
              aria-label={field.label}
              style={{ ...inputStyle, width: '180px' }}
            />
          )}
        </div>
      ))}
      <button
        type="submit"
        className="inline-flex h-[34px] items-center gap-1.5 rounded-lg px-4 text-[13px] font-medium transition-colors duration-150"
        style={{
          background: 'var(--color-accent)',
          color: '#fff',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.opacity = '0.9'
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.opacity = '1'
        }}
      >
        <Search className="h-3.5 w-3.5" />
        Search
      </button>
    </form>
  )
}
