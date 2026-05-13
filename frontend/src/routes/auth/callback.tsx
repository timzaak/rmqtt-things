import { useEffect } from 'react'
import { createRoute } from '@tanstack/react-router'
import { rootRoute } from '../__root'
import { setAuthToken } from '@/lib/auth'

export const authCallbackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/auth/callback',
  component: AuthCallbackPage,
})

export const Route = authCallbackRoute

export function completeAuthCallback(search = window.location.search): string {
  const params = new URLSearchParams(search)
  const token = params.get('token')
  const redirect = params.get('redirect') || '/'

  if (token) {
    setAuthToken(token)
  }

  return redirect
}

function AuthCallbackPage() {
  useEffect(() => {
    window.location.href = completeAuthCallback()
  }, [])

  return null
}
