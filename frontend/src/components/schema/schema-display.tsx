import type { JSONSchema } from './schema-editor'

interface SchemaDisplayProps {
  schema: JSONSchema | null | undefined
}

/** Render validation limits as a single comma-separated string. */
function renderValueLimits(property: JSONSchema): string {
  const limits: string[] = []
  if (property.minLength !== undefined) limits.push(`MIN LENGTH: ${property.minLength}`)
  if (property.maxLength !== undefined) limits.push(`MAX LENGTH: ${property.maxLength}`)
  if (property.minimum !== undefined) limits.push(`MIN: ${property.minimum}`)
  if (property.maximum !== undefined) limits.push(`MAX: ${property.maximum}`)
  if (property.pattern !== undefined) limits.push(`PATTERN: ${property.pattern}`)
  return limits.join(', ')
}

function PropertyItem({
  name,
  property,
  isRequired,
  level,
}: {
  name: string
  property: JSONSchema
  isRequired: boolean
  level: number
}) {
  const limit = renderValueLimits(property)

  return (
    <li style={{ paddingLeft: level * 20 }}>
      <div className="py-1">
        <span className="font-medium text-slate-800 dark:text-slate-200">
          {name}
        </span>
        {isRequired && (
          <span className="ml-0.5 text-red-600 dark:text-red-400">*</span>
        )}

        <span className="ml-2 text-sm text-slate-500 dark:text-slate-400">
          {property.type?.toUpperCase() ?? 'UNKNOWN'}
        </span>

        {limit && (
          <div className="text-sm">
            <span className="text-slate-500 dark:text-slate-400">Limit: </span>
            <span className="text-slate-700 dark:text-slate-300">{limit}</span>
          </div>
        )}

        {property.type === 'object' && property.properties && (
          <div className="mt-1">
            <PropertyList schema={property} level={level + 1} />
          </div>
        )}

        {property.type === 'array' && property.items && (
          <div className="mt-1">
            <span className="text-sm text-slate-500 dark:text-slate-400">
              Items:{' '}
            </span>
            {property.items.type === 'object' && property.items.properties ? (
              <PropertyList schema={property.items} level={level + 1} />
            ) : (
              <span className="text-sm text-slate-700 dark:text-slate-300">
                {property.items.type?.toUpperCase() ?? 'UNKNOWN'}
              </span>
            )}
          </div>
        )}
      </div>
    </li>
  )
}

function PropertyList({ schema, level }: { schema: JSONSchema; level: number }) {
  if (!schema.properties) return null

  const keys = Object.keys(schema.properties)
  const required = schema.required ?? []

  return (
    <ul className="space-y-0.5">
      {keys.map((key) => (
        <PropertyItem
          key={key}
          name={key}
          property={schema.properties![key]}
          isRequired={required.includes(key)}
          level={level}
        />
      ))}
    </ul>
  )
}

export function SchemaDisplay({ schema }: SchemaDisplayProps) {
  if (!schema) {
    return (
      <p className="text-sm text-slate-500 dark:text-slate-400">
        No schema defined.
      </p>
    )
  }

  return <PropertyList schema={schema} level={0} />
}
