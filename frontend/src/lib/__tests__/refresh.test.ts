import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { client } from '@/lib/api-generated/client.gen'
import { listProducts } from '@/lib/api-generated/sdk.gen'
import { installAutoRefreshInterceptor, refreshAccessToken } from '@/lib/refresh'

// handle401() navigates the browser (window.location.href), which hangs jsdom.
// Mock the auth helpers so a failed refresh doesn't try to navigate; capture
// the redirect call instead.
const handle401Mock = vi.hoisted(() => vi.fn())
vi.mock('@/lib/auth', () => ({
  handle401: handle401Mock,
  resetAuthCheck: vi.fn(),
  getLoginUrl: () => '/api/auth/oauth/start',
  buildLoginRedirectUrl: () => '/api/auth/oauth/start',
}))

const BASE = 'http://localhost:3000'

/**
 * Build a fetch mock that returns a canned response per URL pattern. Each
 * pattern maps to an ordered list of responses so a route can answer
 * differently on the first vs. replayed call.
 */
function mockFetchSequenced(routes: Record<string, Array<{ status: number; body?: string }>>) {
  const cursors: Record<string, number> = {}
  const callUrls: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url =
        typeof input === 'string' ? input : input instanceof Request ? input.url : input.toString()
      callUrls.push(url)
      for (const [pattern, seq] of Object.entries(routes)) {
        if (url.includes(pattern)) {
          const i = cursors[pattern] ?? 0
          cursors[pattern] = i + 1
          const cfg = seq[Math.min(i, seq.length - 1)]
          return Promise.resolve(new Response(cfg.body ?? null, { status: cfg.status }))
        }
      }
      return Promise.resolve(new Response(null, { status: 404 }))
    })
  )
  return { callUrls }
}

describe('auto-refresh interceptor', () => {
  beforeEach(() => {
    client.setConfig({ baseUrl: BASE })
    installAutoRefreshInterceptor()
    handle401Mock.mockClear()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    // remove the interceptor so re-install in the next test doesn't stack up
    client.interceptors.response.fns = client.interceptors.response.fns.map(() => null)
  })

  test('401 triggers one refresh then replays the original request', async () => {
    // admin/product: 401 first, then 200 on the replay. refresh: 200 once.
    const { callUrls } = mockFetchSequenced({
      '/api/admin/product': [
        { status: 401, body: JSON.stringify({ error: 'unauthorized' }) },
        { status: 200, body: JSON.stringify({ data: [], pagination: {} }) },
      ],
      '/api/auth/refresh': [{ status: 200, body: JSON.stringify({ expiresIn: 900 }) }],
    })

    const res = (await listProducts({ query: { page: 1, page_size: 1 } })) as any

    expect(res.response.status).toBe(200)
    // Exactly one refresh call, plus the original 401 and its replay.
    expect(callUrls.filter((u) => u.includes('/api/auth/refresh'))).toHaveLength(1)
    expect(callUrls.filter((u) => u.includes('/api/admin/product'))).toHaveLength(2)
  })

  test('concurrent 401s share a single in-flight refresh (no family revocation)', async () => {
    // The critical Herald invariant: a second concurrent refresh would present
    // an already-rotated refresh token and revoke the whole token family. The
    // interceptor must coalesce every 401 in the same window onto ONE refresh.
    const { callUrls } = mockFetchSequenced({
      '/api/admin/product': [
        { status: 401 },
        { status: 401 },
        { status: 200, body: JSON.stringify({ data: [], pagination: {} }) },
        { status: 200, body: JSON.stringify({ data: [], pagination: {} }) },
      ],
      '/api/auth/refresh': [{ status: 200, body: JSON.stringify({ expiresIn: 900 }) }],
    })

    // Fire two admin requests simultaneously against the same expired token.
    const [a, b] = await Promise.all([
      listProducts({ query: { page: 1, page_size: 1 } }),
      listProducts({ query: { page: 1, page_size: 1 } }),
    ])

    expect((a as any).response.status).toBe(200)
    expect((b as any).response.status).toBe(200)
    // Exactly one refresh across both concurrent 401s — the anti-revocation guard.
    expect(callUrls.filter((u) => u.includes('/api/auth/refresh'))).toHaveLength(1)
  })

  test('refresh failure (refresh 401) does not retry and triggers login redirect', async () => {
    mockFetchSequenced({
      '/api/admin/product': [{ status: 401 }],
      '/api/auth/refresh': [{ status: 401 }],
    })

    const ok = await refreshAccessToken()
    expect(ok).toBe(false)
    // The failed refresh should hand off to the login redirect, not retry.
    expect(handle401Mock).toHaveBeenCalledTimes(1)
  })

  test('a 401 on /api/auth/refresh itself does not recursively refresh', async () => {
    // If refresh returned 401 and the interceptor tried to refresh it, we would
    // loop forever. The NO_REFRESH_PREFIXES guard must short-circuit it.
    const { callUrls } = mockFetchSequenced({
      '/api/auth/refresh': [{ status: 401 }],
    })

    const ok = await refreshAccessToken()
    expect(ok).toBe(false)
    // No second refresh call beyond the original one issued by refreshAccessToken.
    expect(callUrls.filter((u) => u.includes('/api/auth/refresh'))).toHaveLength(1)
    expect(handle401Mock).toHaveBeenCalledTimes(1)
  })
})
