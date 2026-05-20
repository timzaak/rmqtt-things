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
        <div key={field.name} className="flex flex-col gap-1">
          <label className="text-xs font-medium text-slate-500 dark:text-slate-400">
            {field.label}
          </label>
          {field.type === 'select' ? (
            <select
              value={values[field.name]}
              onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              aria-label={field.label}
              className="h-9 rounded-md border border-slate-300 bg-white px-3 text-sm dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
            >
              <option value="">All</option>
              {field.options?.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              value={values[field.name]}
              onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              placeholder={field.placeholder}
              aria-label={field.label}
              className="h-9 w-48 rounded-md border border-slate-300 bg-white px-3 text-sm dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
            />
          )}
        </div>
      ))}
      <button
        type="submit"
        className="inline-flex h-9 items-center gap-1.5 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
      >
        <Search className="h-4 w-4" />
        Search
      </button>
    </form>
  )
}
