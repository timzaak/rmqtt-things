/**
 * Alarm API Helpers
 *
 * Typed API helpers for alarm rule management and alarm record operations.
 * Uses APIRequestContext (shared auth with page) consistent with project patterns.
 */

import type { APIRequestContext } from '@playwright/test'
import { getJson } from './api'

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

export interface AlarmRuleResponse {
  id: number
  product_id: string
  name: string
  description: string | null
  trigger_type: string
  trigger_config: Record<string, unknown>
  condition: Record<string, unknown>
  actions: Record<string, unknown>[]
  enabled: boolean
  throttle_minutes: number
  created_at: string
  updated_at: string
}

export interface AlarmRecordResponse {
  id: number
  rule_id: number
  rule_name: string
  product_id: string
  device_id: string
  level: string
  message: string | null
  trigger_value: Record<string, unknown> | null
  acknowledged: boolean
  status: string
  cleared_at: string | null
  webhook_status: string | null
  created_at: string
}

export interface PaginatedAlarmRulesResponse {
  data: AlarmRuleResponse[]
  pagination: { page: number; page_size: number; total: number }
}

export interface PaginatedAlarmRecordsResponse {
  data: AlarmRecordResponse[]
  pagination: { page: number; page_size: number; total: number }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async function assertOk(response: import('@playwright/test').APIResponse): Promise<void> {
  if (!response.ok()) {
    const text = await response.text()
    throw new Error(`${response.url()} returned ${response.status()}: ${text}`)
  }
}

function buildPath(basePath: string, params?: Record<string, string | number | boolean | undefined>): string {
  if (!params) return basePath
  const query = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== '') query.set(key, String(value))
  }
  const qs = query.toString()
  return qs ? `${basePath}?${qs}` : basePath
}

// ---------------------------------------------------------------------------
// Alarm Rule helpers
// ---------------------------------------------------------------------------

export async function createAlarmRule(
  request: APIRequestContext,
  body: Record<string, unknown>,
): Promise<AlarmRuleResponse> {
  const response = await request.post('/api/admin/alarm-rule', { data: body })
  await assertOk(response)
  return (await response.json()).data
}

export async function deleteAlarmRule(request: APIRequestContext, id: number): Promise<void> {
  const response = await request.delete(`/api/admin/alarm-rule/${id}`)
  await assertOk(response)
}

export async function getAlarmRule(
  request: APIRequestContext,
  id: number,
): Promise<AlarmRuleResponse> {
  const json = await getJson<{ data: AlarmRuleResponse }>(request, `/api/admin/alarm-rule/${id}`)
  return json.data
}

export async function getAlarmRules(
  request: APIRequestContext,
  params?: { product_id?: string; enabled?: boolean; page?: number; page_size?: number },
): Promise<PaginatedAlarmRulesResponse> {
  const path = buildPath('/api/admin/alarm-rule', params as Record<string, string | number | boolean | undefined>)
  return getJson(request, path)
}

// ---------------------------------------------------------------------------
// Alarm Record helpers
// ---------------------------------------------------------------------------

export async function getAlarmRecords(
  request: APIRequestContext,
  params?: {
    product_id?: string
    device_id?: string
    level?: string
    acknowledged?: boolean
    page?: number
    page_size?: number
  },
): Promise<PaginatedAlarmRecordsResponse> {
  const path = buildPath('/api/admin/alarm', params as Record<string, string | number | boolean | undefined>)
  return getJson(request, path)
}

export async function acknowledgeAlarm(
  request: APIRequestContext,
  id: number,
): Promise<AlarmRecordResponse> {
  const response = await request.patch(`/api/admin/alarm/${id}/ack`)
  await assertOk(response)
  return (await response.json()).data
}

export async function clearAlarm(
  request: APIRequestContext,
  id: number,
): Promise<AlarmRecordResponse> {
  const response = await request.patch(`/api/admin/alarm/${id}/clear`)
  await assertOk(response)
  return (await response.json()).data
}
