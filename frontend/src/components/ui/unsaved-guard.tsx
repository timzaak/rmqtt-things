import { useBlocker } from '@tanstack/react-router'

interface UnsavedGuardProps {
  isDirty: boolean
}

export function UnsavedGuard({ isDirty }: UnsavedGuardProps) {
  const blocker = useBlocker({
    shouldBlockFn: () => true,
    withResolver: true as const,
    enableBeforeUnload: isDirty,
    disabled: !isDirty,
  })

  if (blocker.status === 'blocked') {
    if (window.confirm('You have unsaved changes. Are you sure you want to leave?')) {
      blocker.proceed()
    } else {
      blocker.reset()
    }
  }

  return null
}
