import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  buildLoginRedirectUrl,
  checkAuth,
  getLoginUrl,
  handle401,
  resetAuthCheck,
} from '@/lib/auth'

function mockFetch(responses: Record<string, { body?: string; status: number }>) {
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string) => {
      for (const [pattern, config] of Object.entries(responses)) {
        if (url.startsWith(pattern)) {
          return Promise.resolve(new Response(config.body ?? null, { status: config.status }))
        }
      }
      return Promise.resolve(new Response(null, { status: 404 }))
    })
  )
}

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
