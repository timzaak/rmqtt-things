import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  buildLoginRedirectUrl,
  checkAuth,
  getLoginUrl,
  handle401,
  resetAuthCheck,
} from '@/lib/auth'
import { client } from '@/lib/api-generated/client.gen'

function mockFetch(responses: Record<string, { body?: string; status: number }>) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      // The SDK client calls `fetch(new Request(...))`; Request.toString() does
      // not yield its URL in this jsdom, so read `.url` directly. getAuthConfig
      // still passes a plain string URL.
      const url =
        typeof input === 'string' ? input : input instanceof Request ? input.url : input.toString()
      for (const [pattern, config] of Object.entries(responses)) {
        if (url.includes(pattern)) {
          return Promise.resolve(new Response(config.body ?? null, { status: config.status }))
        }
      }
      return Promise.resolve(new Response(null, { status: 404 }))
    })
  )
}

/**
 * The SDK client builds `new Request(url)` which rejects relative URLs in jsdom.
 * Give it an absolute base so the probe resolves; the mock fetch matches on the
 * path substring regardless of host.
 */
const TEST_BASE_URL = 'http://localhost:3000'

const authEnabled = {
  body: JSON.stringify({
    enabled: true,
    login_url: '/api/auth/oauth/start',
    herald_login_url: 'https://herald.example.com/default/auth/login',
  }),
  status: 200,
}
const authDisabled = { body: JSON.stringify({ enabled: false }), status: 200 }
const ok = { status: 200 }

describe('auth helpers', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_APP_BASE_URL', 'https://app.example.com')
    window.history.replaceState({}, '', '/devices?status=Online')
    resetAuthCheck()
    client.setConfig({ baseUrl: TEST_BASE_URL })
  })

  test('skips auth probe when backend reports auth disabled', async () => {
    mockFetch({ '/api/auth/config': authDisabled })

    await expect(checkAuth()).resolves.toBe(true)
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  test('performs auth probe when backend reports auth enabled', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': ok })

    await expect(checkAuth()).resolves.toBe(true)
    expect(fetch).toHaveBeenCalledTimes(2)
  })

  test('treats only a 401 auth probe response as unauthenticated', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': { status: 401 } })

    await expect(checkAuth()).resolves.toBe(false)
  })

  test('surfaces a transient auth probe server error', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': { status: 503 } })

    await expect(checkAuth()).rejects.toThrow('Auth probe failed with HTTP 503')
  })

  test('returns the login_url from config', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': ok })

    await checkAuth()

    expect(getLoginUrl()).toBe('/api/auth/oauth/start')
  })

  test('returns / for login URL when auth is disabled', async () => {
    mockFetch({ '/api/auth/config': authDisabled })

    await checkAuth()
    expect(getLoginUrl()).toBe('/')
  })

  test('clears the cached auth probe on 401 handling', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': ok })

    await expect(checkAuth()).resolves.toBe(true)
    handle401()
    await expect(checkAuth()).resolves.toBe(true)

    // config fetch x2 (handle401 resets) + probe x2
    expect(fetch).toHaveBeenCalledTimes(4)
  })

  test('adds the current page as redirect to the login URL', async () => {
    mockFetch({
      '/api/auth/config': {
        body: JSON.stringify({
          enabled: true,
          login_url: '/api/auth/oauth/start',
          herald_login_url: 'http://127.0.0.1:13000/rmqtt/auth/login',
        }),
        status: 200,
      },
      '/api/admin/product': ok,
    })

    await expect(checkAuth()).resolves.toBe(true)

    expect(buildLoginRedirectUrl()).toBe(
      'http://localhost:3000/api/auth/oauth/start?redirect=http%3A%2F%2Flocalhost%3A3000%2Fdevices%3Fstatus%3DOnline'
    )
  })

  test('caches the auth probe result during one navigation pass', async () => {
    mockFetch({ '/api/auth/config': authEnabled, '/api/admin/product': ok })

    await expect(checkAuth()).resolves.toBe(true)
    await expect(checkAuth()).resolves.toBe(true)

    expect(fetch).toHaveBeenCalledTimes(2)
  })
})
