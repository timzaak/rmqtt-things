import type { CSSProperties } from 'react'

export const sectionHeading: CSSProperties = {
  color: 'var(--color-text-primary)',
  fontSize: '15px',
  fontWeight: 600,
  marginBottom: '16px',
}

export const labelStyle: CSSProperties = {
  color: 'var(--color-text-muted)',
  fontSize: '11px',
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.05em',
}

export const valueStyle: CSSProperties = {
  color: 'var(--color-text-primary)',
  fontSize: '13px',
  fontWeight: 500,
  fontFamily: "'JetBrains Mono', monospace",
}

export const cardStyle: CSSProperties = {
  background: 'var(--color-surface-1)',
  border: '1px solid var(--color-border)',
  borderRadius: '12px',
  padding: '16px',
}

// Shared shell for the small controls across device-detail sections (selects,
// inputs, toggle buttons).
export const controlBaseStyle: CSSProperties = {
  height: '30px',
  borderRadius: '8px',
  border: '1px solid var(--color-border)',
  color: 'var(--color-text-primary)',
  fontSize: '13px',
}

// Form controls (filters/selects) on top of the shell. Buttons deliberately
// keep only the base: they need their own padding/background/cursor and must
// not take `outline: 'none'`, which would drop the keyboard focus ring.
export const selectControlStyle: CSSProperties = {
  ...controlBaseStyle,
  background: 'var(--color-surface-1)',
  padding: '0 10px',
  outline: 'none',
}
