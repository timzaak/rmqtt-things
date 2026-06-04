import { useEffect, useState } from 'react'
import { createRoute, Link, useNavigate } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { useAlarmRule, useUpdateAlarmRule } from '@/hooks/useAlarmRules'
import { useProducts } from '@/hooks/useProducts'
import { PageHeader } from '@/components/ui/page-header'
import { UnsavedGuard } from '@/components/ui/unsaved-guard'
import { toast } from '@/components/ui/sonner'
import {
  TriggerConfigSection,
  ConditionEditor,
  ActionsEditor,
  DurationSection,
  ClearConditionSection,
  TRIGGER_TYPE_OPTIONS,
  type FormState,
  type ActionFormState,
  type ConditionFormState,
  validateClearCondition,
} from '@/components/alarm-rule/form-sections'
import type { AlarmRule } from '@/lib/api-generated/types.gen'

export const alarmRulesEditIdRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/alarm-rules/edit/$id',
  component: AlarmRuleEditPage,
})

export const Route = alarmRulesEditIdRoute

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
const disabledStyle: React.CSSProperties = {
  width: '100%',
  borderRadius: '6px',
  border: '1px solid var(--color-border)',
  padding: '8px 12px',
  fontSize: '13px',
  background: 'var(--color-surface-2)',
  color: 'var(--color-text-muted)',
}

const emptyForm: FormState = {
  product_id: '',
  name: '',
  description: '',
  trigger_type: '',
  trigger_config: {},
  condition: { operator: '' },
  actions: [],
  throttle_minutes: 0,
  duration_minutes: 0,
  clear_condition: null,
}

function alarmRuleToForm(rule: AlarmRule): FormState {
  return {
    product_id: rule.product_id,
    name: rule.name,
    description: rule.description ?? '',
    trigger_type: rule.trigger_type,
    trigger_config: (rule.trigger_config as Record<string, unknown>) ?? {},
    condition: (rule.condition as ConditionFormState) ?? { operator: '' },
    actions: (rule.actions as ActionFormState[]) ?? [],
    throttle_minutes: rule.throttle_minutes ?? 0,
    duration_minutes: rule.duration_minutes ?? 0,
    clear_condition: (rule.clear_condition as ConditionFormState | null) ?? null,
  }
}

function AlarmRuleEditPage() {
  const { id: idStr } = alarmRulesEditIdRoute.useParams()
  const id = Number(idStr)
  const navigate = useNavigate()
  const { data: alarmRule, isLoading } = useAlarmRule(id)
  const updateMutation = useUpdateAlarmRule()
  const { data: products } = useProducts()

  const [form, setForm] = useState<FormState>({ ...emptyForm })
  const [prevData, setPrevData] = useState<typeof alarmRule>(undefined)
  const [initialForm, setInitialForm] = useState<FormState | null>(null)
  const [justSaved, setJustSaved] = useState(false)

  useEffect(() => {
    if (justSaved) {
      navigate({ to: '/alarm-rules' })
    }
  }, [justSaved, navigate])

  // prevData pattern: initialize form once when data loads
  if (alarmRule && alarmRule !== prevData) {
    setPrevData(alarmRule)
    const initialized = alarmRuleToForm(alarmRule)
    setForm(initialized)
    setInitialForm(initialized)
  }

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])
  const productName = alarmRule
    ? (productMap.get(alarmRule.product_id) ?? alarmRule.product_id)
    : ''

  const triggerTypeLabel =
    TRIGGER_TYPE_OPTIONS.find((t) => t.value === form.trigger_type)?.label ?? form.trigger_type

  const isDirty =
    !justSaved &&
    initialForm !== null &&
    (form.name !== initialForm.name ||
      form.description !== initialForm.description ||
      JSON.stringify(form.trigger_config) !== JSON.stringify(initialForm.trigger_config) ||
      JSON.stringify(form.condition) !== JSON.stringify(initialForm.condition) ||
      JSON.stringify(form.actions) !== JSON.stringify(initialForm.actions) ||
      form.throttle_minutes !== initialForm.throttle_minutes ||
      form.duration_minutes !== initialForm.duration_minutes ||
      JSON.stringify(form.clear_condition) !== JSON.stringify(initialForm.clear_condition))

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!form.name) {
      toast.error('Name is required')
      return
    }

    // Trigger config required for property / event
    if (
      form.trigger_type === 'property' &&
      !(form.trigger_config as Record<string, unknown>).property_name
    ) {
      toast.error('Property name is required')
      return
    }
    if (
      form.trigger_type === 'event' &&
      !(form.trigger_config as Record<string, unknown>).event_identifier
    ) {
      toast.error('Event identifier is required')
      return
    }

    // Condition operator required
    if (!form.condition.operator) {
      toast.error('Condition operator is required')
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

    // Duration validation (only for property trigger)
    if (form.trigger_type === 'property' && form.duration_minutes < 0) {
      toast.error('Duration must be >= 0')
      return
    }

    // Clear condition completeness (only when enabled for property trigger)
    if (form.trigger_type === 'property' && form.clear_condition) {
      const err = validateClearCondition(form.clear_condition)
      if (err) {
        toast.error(err)
        return
      }
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

    updateMutation.mutate(
      {
        id,
        name: form.name,
        description: form.description || null,
        trigger_config: form.trigger_config,
        condition: form.condition,
        actions: form.actions,
        throttle_minutes: form.throttle_minutes,
        ...(form.trigger_type === 'property'
          ? {
              duration_minutes: form.duration_minutes > 0 ? form.duration_minutes : undefined,
              clear_condition:
                form.clear_condition && form.clear_condition.operator !== ''
                  ? form.clear_condition
                  : null,
            }
          : {}),
      },
      {
        onSuccess: () => {
          toast.success('Alarm rule updated')
          setJustSaved(true)
        },
        onError: (error) => {
          toast.error('Failed to update alarm rule', { description: error.message })
        },
      }
    )
  }

  if (isLoading) {
    return <div style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>Loading...</div>
  }

  if (!alarmRule) {
    return <div style={{ fontSize: '13px', color: '#dc2626' }}>Alarm rule not found</div>
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit Alarm Rule" />
      <form onSubmit={handleSubmit} className="max-w-4xl space-y-4">
        {/* Product (disabled) + Name */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label style={labelStyle}>Product</label>
            <input
              type="text"
              disabled
              value={productName}
              style={disabledStyle}
              data-testid="product-input-disabled"
            />
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

        {/* Trigger Type (disabled) */}
        <div>
          <label style={labelStyle}>Trigger Type</label>
          <input
            type="text"
            disabled
            value={triggerTypeLabel}
            style={disabledStyle}
            data-testid="trigger-type-input-disabled"
          />
        </div>

        {/* Trigger config section (read-only trigger_type, editable config) */}
        <TriggerConfigSection
          trigger_type={form.trigger_type}
          trigger_config={form.trigger_config}
          onTriggerTypeChange={() => {}}
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

        {/* Duration section */}
        <DurationSection
          duration_minutes={form.duration_minutes}
          onDurationChange={(v) => setForm((f) => ({ ...f, duration_minutes: v }))}
          visible={form.trigger_type === 'property'}
        />

        {/* Clear condition section */}
        <ClearConditionSection
          clear_condition={form.clear_condition}
          onClearConditionChange={(c) => setForm((f) => ({ ...f, clear_condition: c }))}
          visible={form.trigger_type === 'property'}
        />

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
            disabled={updateMutation.isPending}
            style={{
              borderRadius: '6px',
              padding: '8px 16px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'var(--color-accent)',
              color: '#fff',
              opacity: updateMutation.isPending ? 0.5 : 1,
            }}
            data-testid="submit-button"
          >
            {updateMutation.isPending ? 'Saving...' : 'Save'}
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
