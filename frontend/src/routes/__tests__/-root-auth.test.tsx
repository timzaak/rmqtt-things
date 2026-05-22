import { beforeEach, describe, expect, test, vi } from 'vitest'

const { checkAuth, handle401 } = vi.hoisted(() => ({
  checkAuth: vi.fn(),
  handle401: vi.fn(),
}))

vi.mock('@/lib/auth', () => ({
  checkAuth,
  handle401,
}))

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-router')>()
  return {
    ...actual,
    createRootRouteWithContext: () => (options: unknown) => {
      ;(globalThis as Record<string, unknown>).__rootRouteOptions = options
      return { options }
    },
    Outlet: () => null,
  }
})

vi.mock('@tanstack/react-query-devtools', () => ({
  ReactQueryDevtools: () => null,
}))

vi.mock('@/components/theme/theme-provider', () => ({
  ThemeProvider: ({ children }: { children: React.ReactNode }) => children,
}))

vi.mock('@/components/layout/app-layout', () => ({
  AppLayout: ({ children }: { children: React.ReactNode }) => children,
}))

vi.mock('@/components/ui/sonner', () => ({
  Toaster: () => null,
}))

import '../__root'

describe('root auth guard', () => {
  const options = (globalThis as Record<string, unknown>).__rootRouteOptions as {
    beforeLoad: () => Promise<void>
  }

  beforeEach(() => {
    checkAuth.mockReset()
    handle401.mockReset()
  })

  test('delegates unauthenticated navigations to shared 401 handling', async () => {
    checkAuth.mockResolvedValue(false)

    await expect(options.beforeLoad()).rejects.toThrow('unauthenticated')

    expect(checkAuth).toHaveBeenCalledTimes(1)
    expect(handle401).toHaveBeenCalledTimes(1)
  })

  test('allows authenticated navigations', async () => {
    checkAuth.mockResolvedValue(true)

    await options.beforeLoad()

    expect(checkAuth).toHaveBeenCalledTimes(1)
    expect(handle401).not.toHaveBeenCalled()
  })
})
