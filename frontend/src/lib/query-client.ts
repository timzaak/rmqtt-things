import { QueryClient } from '@tanstack/react-query'

/**
 * Shared React Query client singleton, declared in its own module so that
 * components (e.g. the header's logout handler) can import it without pulling
 * in `main.tsx` — which would otherwise create a circular import
 * (main → routeTree → __root → AppLayout → Header → main).
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5 * 60 * 1000,
      retry: false,
    },
  },
})
