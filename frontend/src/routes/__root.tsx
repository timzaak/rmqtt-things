import { createRootRouteWithContext, Outlet } from '@tanstack/react-router'
import { ReactQueryDevtools } from '@tanstack/react-query-devtools'
import { Toaster } from '@/components/ui/sonner'
import { ThemeProvider } from '@/components/theme/theme-provider'
import { AppLayout } from '@/components/layout/app-layout'
import { checkAuth, handle401 } from '@/lib/auth'
import type { QueryClient } from '@tanstack/react-query'

type RouterContext = {
  queryClient: QueryClient
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
  beforeLoad: async ({ location }) => {
    if (location.pathname === '/auth/callback') {
      return
    }

    const authed = await checkAuth()
    if (!authed) {
      handle401()
      throw new Error('unauthenticated')
    }
  },
  component: RootComponent,
})

export const Route = rootRoute

function RootComponent() {
  return (
    <ThemeProvider>
      <AppLayout>
        <Outlet />
      </AppLayout>
      <Toaster />
      {import.meta.env.DEV && <ReactQueryDevtools />}
    </ThemeProvider>
  )
}
