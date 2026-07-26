// Shared status rendering for Property Commands and Action Invocations.
// Both tables render the same command/invocation lifecycle statuses with the
// same colors; centralize the map + markup so they cannot drift.
const STATUS_COLORS: Record<string, string> = {
  Pending: '#d97706',
  Sent: 'var(--color-accent)',
  Success: '#059669',
  Failed: '#dc2626',
  Deleted: 'var(--color-text-muted)',
}

export function StatusBadge({ status }: { status: string }) {
  return (
    <span
      className="text-[12px] font-semibold"
      style={{ color: STATUS_COLORS[status] ?? 'var(--color-text-secondary)' }}
    >
      {status}
    </span>
  )
}
