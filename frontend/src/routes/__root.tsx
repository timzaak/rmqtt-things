import { createRootRouteWithContext, Outlet } from '@tanstack/react-router'
import { ReactQueryDevtools } from '@tanstack/react-query-devtools'
import { Toaster } from '@/components/ui/sonner'
import { ThemeProvider } from '@/components/theme/theme-provider'
import { AppLayout } from '@/components/layout/app-layout'
import type { QueryClient } from '@tanstack/react-query'

type RouterContext = {
  queryClient: QueryClient
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
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
