/**
 * Shared form sections for alarm rule create/edit pages.
 * Extracted as named exports for reuse across FE-D03 (create) and FE-D04 (edit).
 */

const inputClass =
  'w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100'
const labelClass = 'mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TriggerConfigFormState {
  trigger_type: string
  trigger_config: Record<string, unknown>
}

export interface ConditionFormState {
  operator: string
  value?: unknown
  min?: number
  max?: number
}

export interface ActionFormState {
  type: string
  level?: string
  message?: string
  url?: string
}

export interface FormState {
  product_id: string
  name: string
  description: string
  trigger_type: string
  trigger_config: Record<string, unknown>
  condition: ConditionFormState
  actions: ActionFormState[]
  throttle_minutes: number
}

const TRIGGER_TYPES = [
  { value: 'property', label: 'Property' },
  { value: 'event', label: 'Event' },
  { value: 'device_online', label: 'Device Online' },
  { value: 'device_offline', label: 'Device Offline' },
] as const

const OPERATORS = [
  { value: '>', label: '>' },
  { value: '>=', label: '>=' },
  { value: '<', label: '<' },
  { value: '<=', label: '<=' },
  { value: '==', label: '==' },
  { value: '!=', label: '!=' },
  { value: 'between', label: 'between' },
  { value: 'contains', label: 'contains' },
  { value: 'always', label: 'always' },
] as const

const ALARM_LEVELS = [
  { value: 'info', label: 'Info' },
  { value: 'warning', label: 'Warning' },
  { value: 'critical', label: 'Critical' },
] as const

// ---------------------------------------------------------------------------
// TriggerConfigSection
// ---------------------------------------------------------------------------

interface TriggerConfigSectionProps {
  trigger_type: string
  trigger_config: Record<string, unknown>
  onTriggerTypeChange: (type: string) => void
  onTriggerConfigChange: (config: Record<string, unknown>) => void
}

export function TriggerConfigSection({
  trigger_type,
  trigger_config,
  onTriggerTypeChange,
  onTriggerConfigChange,
}: TriggerConfigSectionProps) {
  const showPropertyName = trigger_type === 'property'
  const showEventIdentifier = trigger_type === 'event'

  return (
    <div className="space-y-4">
      {/* Trigger type */}
      <div>
        <label htmlFor="trigger_type" className={labelClass}>
          Trigger Type <span className="text-red-500">*</span>
        </label>
        <select
          id="trigger_type"
          required
          value={trigger_type}
          onChange={(e) => onTriggerTypeChange(e.target.value)}
          className={inputClass}
          data-testid="trigger-type-select"
        >
          <option value="">Select trigger type</option>
          {TRIGGER_TYPES.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </select>
      </div>

      {/* Dynamic trigger config */}
      {showPropertyName && (
        <div>
          <label htmlFor="property_name" className={labelClass}>
            Property Name <span className="text-red-500">*</span>
          </label>
          <input
            id="property_name"
            type="text"
            required
            value={(trigger_config.property_name as string) ?? ''}
            onChange={(e) =>
              onTriggerConfigChange({ ...trigger_config, property_name: e.target.value })
            }
            className={inputClass}
            data-testid="property-name-input"
          />
        </div>
      )}

      {showEventIdentifier && (
        <div>
          <label htmlFor="event_identifier" className={labelClass}>
            Event Identifier <span className="text-red-500">*</span>
          </label>
          <input
            id="event_identifier"
            type="text"
            required
            value={(trigger_config.event_identifier as string) ?? ''}
            onChange={(e) =>
              onTriggerConfigChange({ ...trigger_config, event_identifier: e.target.value })
            }
            className={inputClass}
            data-testid="event-identifier-input"
          />
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// ConditionEditor
// ---------------------------------------------------------------------------

interface ConditionEditorProps {
  condition: ConditionFormState
  onConditionChange: (condition: ConditionFormState) => void
}

export function ConditionEditor({ condition, onConditionChange }: ConditionEditorProps) {
  const showValue = condition.operator && condition.operator !== 'between' && condition.operator !== 'always'
  const showBetween = condition.operator === 'between'

  return (
    <div className="space-y-4">
      <div>
        <label htmlFor="condition_operator" className={labelClass}>
          Condition Operator <span className="text-red-500">*</span>
        </label>
        <select
          id="condition_operator"
          required
          value={condition.operator}
          onChange={(e) => {
            const next: ConditionFormState = { operator: e.target.value }
            // Preserve relevant fields when switching
            if (e.target.value === 'between') {
              next.min = condition.min
              next.max = condition.max
            } else if (e.target.value !== 'always') {
              next.value = condition.value
            }
            onConditionChange(next)
          }}
          className={inputClass}
          data-testid="condition-operator-select"
        >
          <option value="">Select operator</option>
          {OPERATORS.map((op) => (
            <option key={op.value} value={op.value}>
              {op.label}
            </option>
          ))}
        </select>
      </div>

      {showValue && (
        <div>
          <label htmlFor="condition_value" className={labelClass}>
            Value <span className="text-red-500">*</span>
          </label>
          <input
            id="condition_value"
            type="text"
            required
            value={String(condition.value ?? '')}
            onChange={(e) =>
              onConditionChange({ ...condition, value: e.target.value })
            }
            className={inputClass}
            data-testid="condition-value-input"
          />
        </div>
      )}

      {showBetween && (
        <div className="flex gap-4">
          <div className="flex-1">
            <label htmlFor="condition_min" className={labelClass}>
              Min <span className="text-red-500">*</span>
            </label>
            <input
              id="condition_min"
              type="number"
              required
              value={condition.min ?? ''}
              onChange={(e) =>
                onConditionChange({ ...condition, min: e.target.value === '' ? undefined : Number(e.target.value) })
              }
              className={inputClass}
              data-testid="condition-min-input"
            />
          </div>
          <div className="flex-1">
            <label htmlFor="condition_max" className={labelClass}>
              Max <span className="text-red-500">*</span>
            </label>
            <input
              id="condition_max"
              type="number"
              required
              value={condition.max ?? ''}
              onChange={(e) =>
                onConditionChange({ ...condition, max: e.target.value === '' ? undefined : Number(e.target.value) })
              }
              className={inputClass}
              data-testid="condition-max-input"
            />
          </div>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// ActionsEditor
// ---------------------------------------------------------------------------

interface ActionsEditorProps {
  actions: ActionFormState[]
  onActionsChange: (actions: ActionFormState[]) => void
}

export function ActionsEditor({ actions, onActionsChange }: ActionsEditorProps) {
  const updateAction = (index: number, patch: Partial<ActionFormState>) => {
    const next = actions.map((a, i) => (i === index ? { ...a, ...patch } : a))
    onActionsChange(next)
  }

  const addAction = (type: string) => {
    if (type === 'alarm') {
      onActionsChange([...actions, { type: 'alarm', level: 'warning', message: '' }])
    } else if (type === 'webhook') {
      onActionsChange([...actions, { type: 'webhook', url: '' }])
    }
  }

  const removeAction = (index: number) => {
    // Prevent removing last alarm action
    const action = actions[index]
    if (action.type === 'alarm') {
      const remainingAlarmCount = actions.filter((a, i) => i !== index && a.type === 'alarm').length
      if (remainingAlarmCount === 0) return
    }
    onActionsChange(actions.filter((_, i) => i !== index))
  }

  return (
    <div data-testid="actions-editor" className="space-y-4">
      {actions.map((action, index) => (
        <div
          key={index}
          className="rounded-md border border-slate-200 p-4 dark:border-slate-700 space-y-3"
          data-testid={`action-item-${index}`}
        >
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-slate-700 dark:text-slate-300">
              {action.type === 'alarm' ? 'Alarm Action' : 'Webhook Action'}
            </span>
            <button
              type="button"
              onClick={() => removeAction(index)}
              disabled={action.type === 'alarm' && actions.filter((a) => a.type === 'alarm').length <= 1}
              className="text-sm text-red-600 hover:underline disabled:opacity-30 disabled:cursor-not-allowed dark:text-red-400"
              data-testid={`action-remove-button-${index}`}
            >
              Remove
            </button>
          </div>

          {action.type === 'alarm' && (
            <div className="space-y-3">
              <div>
                <label htmlFor={`action-${index}-level`} className={labelClass}>
                  Level <span className="text-red-500">*</span>
                </label>
                <select
                  id={`action-${index}-level`}
                  value={action.level ?? 'warning'}
                  onChange={(e) => updateAction(index, { level: e.target.value })}
                  className={inputClass}
                  data-testid={`action-level-select-${index}`}
                >
                  {ALARM_LEVELS.map((l) => (
                    <option key={l.value} value={l.value}>
                      {l.label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label htmlFor={`action-${index}-message`} className={labelClass}>
                  Message <span className="text-red-500">*</span>
                </label>
                <input
                  id={`action-${index}-message`}
                  type="text"
                  required
                  value={action.message ?? ''}
                  onChange={(e) => updateAction(index, { message: e.target.value })}
                  className={inputClass}
                  data-testid={`action-message-input-${index}`}
                />
              </div>
            </div>
          )}

          {action.type === 'webhook' && (
            <div>
              <label htmlFor={`action-${index}-url`} className={labelClass}>
                URL <span className="text-red-500">*</span>
              </label>
              <input
                id={`action-${index}-url`}
                type="url"
                required
                value={action.url ?? ''}
                onChange={(e) => updateAction(index, { url: e.target.value })}
                className={inputClass}
                data-testid={`action-url-input-${index}`}
              />
            </div>
          )}
        </div>
      ))}

      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => addAction('alarm')}
          className="rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          data-testid="add-alarm-action-button"
        >
          + Add Alarm Action
        </button>
        <button
          type="button"
          onClick={() => addAction('webhook')}
          className="rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          data-testid="add-webhook-action-button"
        >
          + Add Webhook Action
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Exported constants for reuse
// ---------------------------------------------------------------------------

// eslint-disable-next-line react-refresh/only-export-components
export const INITIAL_CONDITION: ConditionFormState = {
  operator: '',
}

// eslint-disable-next-line react-refresh/only-export-components
export const INITIAL_ACTIONS: ActionFormState[] = [
  { type: 'alarm', level: 'warning', message: '' },
]

export const TRIGGER_TYPE_OPTIONS = TRIGGER_TYPES
export const OPERATOR_OPTIONS = OPERATORS
export const ALARM_LEVEL_OPTIONS = ALARM_LEVELS
