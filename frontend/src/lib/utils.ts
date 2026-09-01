import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

const pad = (n: number) => String(n).padStart(2, '0')

export function formatDatetime(iso: string): string {
  const d = new Date(iso)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export function toDatetimeLocal(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/**
 * Convert a property key to kebab-case for dynamic data-testid attributes.
 * e.g. `colorTemp` -> `color-temp`, `brightness` -> `brightness`.
 *
 * The e2e mirror in demo/e2e/selectors.ts must stay aligned with this rule.
 * StateConfigurationSection.tsx still carries an equivalent private copy that
 * predates this shared home and should migrate to it on its next edit.
 */
export function toKebabKey(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase()
}

/**
 * Extract a human-readable message from an unknown error value (e.g. a React
 * Query mutation `onError` payload). Falls back to String(error) when no
 * `message` field is available.
 */
export function extractErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

/**
 * Render an arbitrary value (often a JSON payload) as a compact string for
 * table cells: `null`/`undefined` -> '-', objects -> JSON, else `String(value)`.
 */
export function formatValue(value: unknown): string {
  if (value === undefined || value === null) {
    return '-'
  }
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value)
    } catch {
      return String(value)
    }
  }
  return String(value)
}
