import { useState } from 'react'
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
  TRIGGER_TYPE_OPTIONS,
  type FormState,
  type ActionFormState,
  type ConditionFormState,
} from '@/components/alarm-rule/form-sections'
import type { AlarmRule } from '@/lib/api-generated/types.gen'

export const alarmRulesEditIdRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/alarm-rules/edit/$id',
  component: AlarmRuleEditPage,
})

export const Route = alarmRulesEditIdRoute

const inputClass =
  'w-full rounded-md border border-slate-300 px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100'
const labelClass = 'mb-1 block text-sm font-medium text-slate-700 dark:text-slate-300'
const disabledClass =
  'w-full rounded-md border border-slate-300 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-400'

const emptyForm: FormState = {
  product_id: '',
  name: '',
  description: '',
  trigger_type: '',
  trigger_config: {},
  condition: { operator: '' },
  actions: [],
  throttle_minutes: 0,
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

  // prevData pattern: initialize form once when data loads
  if (alarmRule && alarmRule !== prevData) {
    setPrevData(alarmRule)
    const initialized = alarmRuleToForm(alarmRule)
    setForm(initialized)
    setInitialForm(initialized)
  }

  const productMap = new Map(products?.data?.map((p) => [p.model_no, p.name]) ?? [])
  const productName = alarmRule ? (productMap.get(alarmRule.product_id) ?? alarmRule.product_id) : ''

  const triggerTypeLabel =
    TRIGGER_TYPE_OPTIONS.find((t) => t.value === form.trigger_type)?.label ?? form.trigger_type

  const isDirty =
    initialForm !== null &&
    (form.name !== initialForm.name ||
      form.description !== initialForm.description ||
      JSON.stringify(form.trigger_config) !== JSON.stringify(initialForm.trigger_config) ||
      JSON.stringify(form.condition) !== JSON.stringify(initialForm.condition) ||
      JSON.stringify(form.actions) !== JSON.stringify(initialForm.actions) ||
      form.throttle_minutes !== initialForm.throttle_minutes)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!form.name) {
      toast.error('Name is required')
      return
    }

    // Trigger config required for property / event
    if (form.trigger_type === 'property' && !(form.trigger_config as Record<string, unknown>).property_name) {
      toast.error('Property name is required')
      return
    }
    if (form.trigger_type === 'event' && !(form.trigger_config as Record<string, unknown>).event_identifier) {
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
      },
      {
        onSuccess: () => {
          toast.success('Alarm rule updated')
          navigate({ to: '/alarm-rules' })
        },
        onError: (error) => {
          toast.error('Failed to update alarm rule', { description: error.message })
        },
      },
    )
  }

  if (isLoading) {
    return <div className="text-sm text-slate-500">Loading...</div>
  }

  if (!alarmRule) {
    return <div className="text-sm text-red-500">Alarm rule not found</div>
  }

  return (
    <div>
      <UnsavedGuard isDirty={isDirty} />
      <PageHeader title="Edit Alarm Rule" />
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        {/* Product (disabled) */}
        <div>
          <label className={labelClass}>Product</label>
          <input
            type="text"
            disabled
            value={productName}
            className={disabledClass}
            data-testid="product-input-disabled"
          />
        </div>

        {/* Name */}
        <div>
          <label htmlFor="name" className={labelClass}>
            Name <span className="text-red-500">*</span>
          </label>
          <input
            id="name"
            type="text"
            required
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            className={inputClass}
            data-testid="name-input"
          />
        </div>

        {/* Description */}
        <div>
          <label htmlFor="description" className={labelClass}>
            Description
          </label>
          <textarea
            id="description"
            value={form.description}
            onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
            rows={3}
            className={inputClass}
            data-testid="description-input"
          />
        </div>

        {/* Trigger Type (disabled) */}
        <div>
          <label className={labelClass}>Trigger Type</label>
          <input
            type="text"
            disabled
            value={triggerTypeLabel}
            className={disabledClass}
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
        <div className="rounded-md border border-slate-200 p-4 dark:border-slate-700 space-y-3">
          <h3 className="text-sm font-medium text-slate-700 dark:text-slate-300">Condition</h3>
          <ConditionEditor
            condition={form.condition}
            onConditionChange={(condition) => setForm((f) => ({ ...f, condition }))}
          />
        </div>

        {/* Actions editor */}
        <div className="rounded-md border border-slate-200 p-4 dark:border-slate-700 space-y-3">
          <h3 className="text-sm font-medium text-slate-700 dark:text-slate-300">Actions</h3>
          <ActionsEditor
            actions={form.actions}
            onActionsChange={(actions) => setForm((f) => ({ ...f, actions }))}
          />
        </div>

        {/* Throttle */}
        <div>
          <label htmlFor="throttle_minutes" className={labelClass}>
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
            className={inputClass}
            data-testid="throttle-minutes-input"
          />
          <p className="mt-1 text-xs text-slate-500">Dedup interval in minutes. 0 means no dedup.</p>
        </div>

        {/* Submit / Cancel */}
        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            disabled={updateMutation.isPending}
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200"
            data-testid="submit-button"
          >
            {updateMutation.isPending ? 'Saving...' : 'Save'}
          </button>
          <Link
            to="/alarm-rules"
            className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
            data-testid="cancel-button"
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  )
}
