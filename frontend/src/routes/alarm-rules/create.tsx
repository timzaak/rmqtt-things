import { useEffect, useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useCreateAlarmRule } from '@/hooks/useAlarmRules'
import { useProducts } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'
import {
  TriggerConfigSection,
  ConditionEditor,
  ActionsEditor,
  INITIAL_CONDITION,
  INITIAL_ACTIONS,
  OPERATORS_BY_TRIGGER_MAP,
  type FormState,
} from '@/components/alarm-rule/form-sections'

export const alarmRulesCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/alarm-rules/create',
  component: AlarmRuleCreatePage,
})

export const Route = alarmRulesCreateRoute

// ---------------------------------------------------------------------------
// FormState initial value
// ---------------------------------------------------------------------------

const initialForm: FormState = {
  product_id: '',
  name: '',
  description: '',
  trigger_type: '',
  trigger_config: {},
  condition: { ...INITIAL_CONDITION },
  actions: INITIAL_ACTIONS.map((a) => ({ ...a })),
  throttle_minutes: 0,
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  borderRadius: '6px',
  border: '1px solid var(--color-border)',
  padding: '8px 12px',
  fontSize: '13px',
  background: 'var(--color-surface-1)',
  color: 'var(--color-text-primary)',
}
const labelStyle: React.CSSProperties = {
  display: 'block',
  marginBottom: '4px',
  fontSize: '13px',
  fontWeight: 500,
  color: 'var(--color-text-secondary)',
}

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

function AlarmRuleCreatePage() {
  const navigate = useNavigate()
  const createAlarmRule = useCreateAlarmRule()
  const { data: products } = useProducts()
  const [form, setForm] = useState<FormState>({
    ...initialForm,
    actions: [{ ...INITIAL_ACTIONS[0] }],
  })
  const [justSaved, setJustSaved] = useState(false)

  useEffect(() => {
    if (justSaved) {
      navigate({ to: '/alarm-rules' })
    }
  }, [justSaved, navigate])

  const isDirty =
    !justSaved &&
    (form.product_id !== '' ||
      form.name !== '' ||
      form.description !== '' ||
      form.trigger_type !== '' ||
      Object.keys(form.trigger_config).length > 0 ||
      form.condition.operator !== '' ||
      form.condition.value !== undefined ||
      form.condition.min !== undefined ||
      form.condition.max !== undefined ||
      form.actions.some((a) => a.message !== '' || a.url !== '') ||
      form.throttle_minutes !== 0)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    // Required field validation
    if (!form.product_id || !form.name || !form.trigger_type || !form.condition.operator) {
      toast.error('Please fill in all required fields')
      return
    }

    // Trigger config required for property / event
    if (form.trigger_type === 'property' && !form.trigger_config.property_name) {
      toast.error('Property name is required')
      return
    }
    if (form.trigger_type === 'event' && !form.trigger_config.event_identifier) {
      toast.error('Event identifier is required')
      return
    }

    // Condition value required (except always / between)
    if (form.condition.operator === 'between') {
      if (form.condition.min === undefined || form.condition.max === undefined) {
        toast.error('Min and max are required for between operator')
        return
      }
    } else if (form.condition.operator !== 'always' && form.condition.value === undefined) {
      toast.error('Condition value is required')
      return
    }

    // At least one alarm action
    const hasAlarm = form.actions.some((a) => a.type === 'alarm')
    if (!hasAlarm) {
      toast.error('At least one alarm action is required')
      return
    }

    // Validate each action
    for (const action of form.actions) {
      if (action.type === 'alarm' && !action.message) {
        toast.error('Alarm action message is required')
        return
      }
      if (action.type === 'webhook' && !action.url) {
        toast.error('Webhook action URL is required')
        return
      }
    }

    // Build request body
    const body: Record<string, unknown> = {
      product_id: form.product_id,
      name: form.name,
      trigger_type: form.trigger_type,
      condition: form.condition,
      actions: form.actions,
    }

    if (form.description) {
      body.description = form.description
    }

    // Only include trigger_config for property / event
    if (form.trigger_type === 'property' || form.trigger_type === 'event') {
      body.trigger_config = form.trigger_config
    }

    if (form.throttle_minutes > 0) {
      body.throttle_minutes = form.throttle_minutes
    }

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    createAlarmRule.mutate(body as any, {
      onSuccess: () => {
        toast.success('Alarm rule created')
        setJustSaved(true)
      },
      onError: (error) => {
        toast.error('Failed to create alarm rule', { description: error.message })
      },
    })
  }

  // Trigger type change handler: reset trigger_config and condition
  const handleTriggerTypeChange = (type: string) => {
    const allowedOps = OPERATORS_BY_TRIGGER_MAP[type]
    const defaultOperator = allowedOps?.length === 1 ? allowedOps[0] : ''
    setForm((f) => ({
      ...f,
      trigger_type: type,
      trigger_config: {},
      condition: { operator: defaultOperator },
    }))
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Create Alarm Rule" />
      <form onSubmit={handleSubmit} className="max-w-4xl space-y-4">
        {/* Product + Name */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label htmlFor="product_id" style={labelStyle}>
              Product <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <select
              id="product_id"
              required
              value={form.product_id}
              onChange={(e) => setForm((f) => ({ ...f, product_id: e.target.value }))}
              style={inputStyle}
              data-testid="product-select"
            >
              <option value="">Select a product</option>
              {products?.data?.map((p) => (
                <option key={p.id} value={p.model_no}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label htmlFor="name" style={labelStyle}>
              Name <span style={{ color: '#dc2626' }}>*</span>
            </label>
            <input
              id="name"
              type="text"
              required
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              style={inputStyle}
              data-testid="name-input"
            />
          </div>
        </div>

        {/* Description */}
        <div>
          <label htmlFor="description" style={labelStyle}>
            Description
          </label>
          <textarea
            id="description"
            value={form.description}
            onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
            rows={3}
            style={inputStyle}
            data-testid="description-input"
          />
        </div>

        {/* Trigger config section */}
        <TriggerConfigSection
          trigger_type={form.trigger_type}
          trigger_config={form.trigger_config}
          onTriggerTypeChange={handleTriggerTypeChange}
          onTriggerConfigChange={(config) => setForm((f) => ({ ...f, trigger_config: config }))}
        />

        {/* Condition editor */}
        <div
          style={{ borderRadius: '6px', border: '1px solid var(--color-border)', padding: '16px' }}
          className="space-y-3"
        >
          <h3 style={{ fontSize: '13px', fontWeight: 500, color: 'var(--color-text-secondary)' }}>
            Condition
          </h3>
          <ConditionEditor
            condition={form.condition}
            onConditionChange={(condition) => setForm((f) => ({ ...f, condition }))}
            trigger_type={form.trigger_type}
          />
        </div>

        {/* Actions editor */}
        <div
          style={{ borderRadius: '6px', border: '1px solid var(--color-border)', padding: '16px' }}
          className="space-y-3"
        >
          <h3 style={{ fontSize: '13px', fontWeight: 500, color: 'var(--color-text-secondary)' }}>
            Actions
          </h3>
          <ActionsEditor
            actions={form.actions}
            onActionsChange={(actions) => setForm((f) => ({ ...f, actions }))}
          />
        </div>

        {/* Throttle */}
        <div>
          <label htmlFor="throttle_minutes" style={labelStyle}>
            Throttle (minutes)
          </label>
          <input
            id="throttle_minutes"
            type="number"
            min={0}
            value={form.throttle_minutes}
            onChange={(e) =>
              setForm((f) => ({
                ...f,
                throttle_minutes: e.target.value === '' ? 0 : Number(e.target.value),
              }))
            }
            style={inputStyle}
            data-testid="throttle-minutes-input"
          />
          <p style={{ marginTop: '4px', fontSize: '12px', color: 'var(--color-text-muted)' }}>
            Dedup interval in minutes. 0 means no dedup.
          </p>
        </div>

        {/* Submit / Cancel */}
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={createAlarmRule.isPending}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'var(--color-accent)',
              color: '#fff',
              opacity: createAlarmRule.isPending ? 0.5 : 1,
            }}
            data-testid="submit-button"
          >
            {createAlarmRule.isPending ? 'Creating...' : 'Create'}
          </button>
          <Link
            to="/alarm-rules"
            style={{
              borderRadius: '6px',
              border: '1px solid var(--color-border)',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              color: 'var(--color-text-secondary)',
            }}
            data-testid="cancel-button"
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
