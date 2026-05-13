let authCheckPromise: Promise<boolean> | null = null
let authConfigPromise: Promise<AuthConfig> | null = null
let cachedAuthConfig: AuthConfig | null = null
let isRedirecting = false

interface AuthConfig {
  enabled: boolean
  herald_url: string | null
}

async function getAuthConfig(): Promise<AuthConfig> {
  if (cachedAuthConfig) return cachedAuthConfig
  if (!authConfigPromise) {
    authConfigPromise = fetch('/api/auth/config')
      .then(res => res.json())
      .then((config: AuthConfig) => {
        cachedAuthConfig = config
        return config
      })
      .catch(err => {
        authConfigPromise = null
        throw err
      })
  }
  return authConfigPromise
}

function getAppBaseUrl(): string {
  return import.meta.env.VITE_APP_BASE_URL || window.location.origin
}

function getCallbackUrl(redirect: string): string {
  const callbackUrl = new URL('/auth/callback', getAppBaseUrl())
  callbackUrl.searchParams.set('redirect', redirect)
  return callbackUrl.toString()
}

export function getLoginUrl(redirect = window.location.href): string {
  const heraldBaseUrl = cachedAuthConfig?.herald_url
  if (!heraldBaseUrl) return '/'
  const loginUrl = new URL('/login', heraldBaseUrl)
  loginUrl.searchParams.set('redirect', getCallbackUrl(redirect))
  return loginUrl.toString()
}

export function resetAuthCheck(): void {
  authCheckPromise = null
  authConfigPromise = null
  cachedAuthConfig = null
  isRedirecting = false
}

export function handle401(): void {
  if (isRedirecting) return
  isRedirecting = true
  resetAuthCheck()
  window.location.href = getLoginUrl()
}

export function setAuthToken(token: string): void {
  document.cookie = `X-Auth=${encodeURIComponent(token)}; Path=/; SameSite=Lax`
  authCheckPromise = null
}

export async function checkAuth(): Promise<boolean> {
  const config = await getAuthConfig()
  if (!config.enabled) return true

  if (!authCheckPromise) {
    authCheckPromise = fetch('/api/admin/product?page=1&page_size=1', {
      credentials: 'include',
    }).then(response => {
      if (response.status === 401) {
        return false
      }

      if (!response.ok) {
        throw new Error(`auth probe failed with status ${response.status}`)
      }

      return true
    }).catch(err => {
      authCheckPromise = null
      throw err
    })
  }

  return authCheckPromise
}
