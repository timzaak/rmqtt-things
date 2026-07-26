import { createElement } from 'react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { client } from '@/lib/api-generated/client.gen'
import { useCreateActionInvocation, useDeleteActionInvocations } from '@/hooks/useActionInvocations'

// The generated client builds `new Request(url)`, which rejects relative URLs
// under jsdom. Pin an absolute baseUrl so the probe resolves; the fetch stub
// matches on the path substring regardless of host (mirrors auth.test.ts).
const BASE_URL = 'http://test.local'

/** Captured request: URL string + parsed JSON body (undefined for bodyless). */
interface CapturedCall {
  url: string
  body: unknown
}

/**
 * Stub global fetch so we observe the *real* request the shared `client`
 * emits. Returns a 200 JSON response by default and records every call for
 * later URL/body assertions. We never mock the hooks' internals — the request
 * goes through the real generated sdk functions.
 */
function stubFetch(
  respond: (
    url: string
  ) => { status: number; body?: string } | Promise<{ status: number; body?: string }>
): { calls: CapturedCall[] } {
  const calls: CapturedCall[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      // The client always passes a Request here; read `.url` directly because
      // Request.toString() does not yield its URL in this jsdom (auth.test.ts).
      const url =
        typeof input === 'string' ? input : input instanceof Request ? input.url : input.toString()
      const { status, body } = await respond(url)
      // Drain the request body once so it is readable; GET/DELETE have none.
      let parsedBody: unknown
      if (input instanceof Request) {
        const raw = await input.clone().text()
        parsedBody = raw ? JSON.parse(raw) : undefined
      }
      calls.push({ url, body: parsedBody })
      return new Response(body ?? null, {
        status,
        headers: body ? { 'Content-Type': 'application/json' } : undefined,
      })
    })
  )
  return { calls }
}

/** Build a fresh QueryClient per test so caches never bleed across cases. */
function createTestQueryClient(): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } })
}

/**
 * Wrapper that injects an isolated QueryClient into the hook under test.
 * A named function satisfies react/display-name (anonymous arrow components
 * would trip it).
 */
function makeWrapper(queryClient: QueryClient) {
  function Wrapper({ children }: { children: ReactNode }) {
    return createElement(QueryClientProvider, { client: queryClient }, children)
  }
  return Wrapper
}

describe('useCreateActionInvocation', () => {
  beforeEach(() => {
    client.setConfig({ baseUrl: BASE_URL })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  test('posts productId/deviceId/serviceType/params as JSON body', async () => {
    const { calls } = stubFetch(() => ({ status: 201, body: '{}' }))
    const queryClient = createTestQueryClient()

    const { result } = renderHook(() => useCreateActionInvocation(), {
      wrapper: makeWrapper(queryClient),
    })

    await result.current.mutateAsync({
      productId: 'product-a',
      deviceId: 'device-001',
      serviceType: 'reboot',
      params: { delaySeconds: 5 },
    })

    expect(calls).toHaveLength(1)
    expect(calls[0].url).toContain('/api/admin/service/command')
    // Body is the CreateActionCommandRequest shape, all four fields exact.
    expect(calls[0].body).toEqual({
      productId: 'product-a',
      deviceId: 'device-001',
      serviceType: 'reboot',
      params: { delaySeconds: 5 },
    })
  })

  test('omits params from body when not provided', async () => {
    const { calls } = stubFetch(() => ({ status: 201, body: '{}' }))
    const queryClient = createTestQueryClient()

    const { result } = renderHook(() => useCreateActionInvocation(), {
      wrapper: makeWrapper(queryClient),
    })

    await result.current.mutateAsync({
      productId: 'product-a',
      deviceId: 'device-001',
      serviceType: 'unlock',
    })

    expect(calls[0].body).toEqual({
      productId: 'product-a',
      deviceId: 'device-001',
      serviceType: 'unlock',
    })
    // The optional params key must not leak into the wire payload.
    expect(calls[0].body).not.toHaveProperty('params')
  })

  test('invalidates action-invocations queries on success', async () => {
    stubFetch(() => ({ status: 201, body: '{}' }))
    const queryClient = createTestQueryClient()
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries')

    const { result } = renderHook(() => useCreateActionInvocation(), {
      wrapper: makeWrapper(queryClient),
    })

    await result.current.mutateAsync({
      productId: 'product-a',
      deviceId: 'device-001',
      serviceType: 'reboot',
    })

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['action-invocations'] })
  })
})

describe('useDeleteActionInvocations', () => {
  beforeEach(() => {
    client.setConfig({ baseUrl: BASE_URL })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  test('sends ids as comma-separated query param', async () => {
    // The hook joins ids into "1,2" and types it through as number[] so the
    // serializer treats it as a primitive, producing `ids=1,2` rather than the
    // exploded `ids=1&ids=2` (mirrors useProperties delete convention).
    const { calls } = stubFetch(() => ({ status: 204 }))
    const queryClient = createTestQueryClient()

    const { result } = renderHook(() => useDeleteActionInvocations(), {
      wrapper: makeWrapper(queryClient),
    })

    await result.current.mutateAsync([1, 2])

    expect(calls).toHaveLength(1)
    expect(calls[0].url).toContain('/api/admin/service/command')
    expect(new URL(calls[0].url).searchParams.get('ids')).toBe('1,2')
  })

  test('invalidates action-invocations queries on success', async () => {
    stubFetch(() => ({ status: 204 }))
    const queryClient = createTestQueryClient()
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteActionInvocations(), {
      wrapper: makeWrapper(queryClient),
    })

    await result.current.mutateAsync([1])

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['action-invocations'] })
  })
})
