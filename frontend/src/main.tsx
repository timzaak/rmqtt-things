/**
 * React application entrypoint.
 *
 * This wires the generated TanStack Router tree and a shared React Query
 * client into the DOM node from index.html.
 */
import './styles.css'
import { StrictMode } from 'react'
import { createRouter, RouterProvider } from '@tanstack/react-router'
import { QueryClientProvider } from '@tanstack/react-query'
import ReactDOM from 'react-dom/client'
import { routeTree } from './routes/-route-tree'
import { installAutoRefreshInterceptor } from './lib/refresh'
import { queryClient } from './lib/query-client'

// Register the 401-driven access-token refresh on the API client before any
// request is made. Herald 0.3.3 access tokens expire every 15 min; without this
// the user is logged out at the first stale-token response.
installAutoRefreshInterceptor()

export const router = createRouter({
  routeTree,
  context: {
    queryClient,
  },
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

ReactDOM.createRoot(document.getElementById('app')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>
)
