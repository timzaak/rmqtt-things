import { toDatetimeLocal } from '@/lib/utils'

// Shared chart-view model: colors, key cap and the time-range preset math.
// Kept out of the component files so they only export components.

// Palette shared by the chart lines/y-axes and the selector chips (the chips
// double as the legend). Order matches the request key order.
export const SERIES_COLORS = ['#3b82f6', '#f97316', '#10b981', '#8b5cf6', '#ef4444']

// Contract cap: the series endpoint accepts 1..=5 keys per query.
export const MAX_CHART_KEYS = 5

export type RangePreset = '1h' | '24h' | '7d' | '30d' | 'custom'

export interface ChartRange {
  preset: RangePreset
  // datetime-local (local timezone) strings, shared by presets and the custom
  // inputs so switching to Custom prefills the previous range
  start: string
  end: string
}

const PRESET_MS: Record<Exclude<RangePreset, 'custom'>, number> = {
  '1h': 3600 * 1000,
  '24h': 24 * 3600 * 1000,
  '7d': 7 * 24 * 3600 * 1000,
  '30d': 30 * 24 * 3600 * 1000,
}

// A preset resolves to concrete start/end at selection time; the range never
// drifts with the clock afterwards (no auto refresh by design).
export function presetRange(preset: Exclude<RangePreset, 'custom'>): ChartRange {
  const end = new Date()
  const start = new Date(end.getTime() - PRESET_MS[preset])
  return { preset, start: toDatetimeLocal(start), end: toDatetimeLocal(end) }
}
