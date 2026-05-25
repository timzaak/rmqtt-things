let authCheckPromise: Promise<boolean> | null = null
let authConfigPromise: Promise<AuthConfig> | null = null
let cachedAuthConfig: AuthConfig | null = null
let isRedirecting = false

interface AuthConfig {
  enabled: boolean
  login_url: string | null
  herald_login_url?: string | null
}

async function getAuthConfig(): Promise<AuthConfig> {
  if (cachedAuthConfig) return cachedAuthConfig
  if (!authConfigPromise) {
    authConfigPromise = fetch('/api/auth/config')
      .then((res) => res.json())
      .then((config: AuthConfig) => {
        cachedAuthConfig = config
        return config
      })
      .catch((err) => {
        authConfigPromise = null
        throw err
      })
  }
  return authConfigPromise
}

export function getLoginUrl(): string {
  return cachedAuthConfig?.login_url || '/'
}

export function resetAuthCheck(): void {
  authCheckPromise = null
  authConfigPromise = null
  cachedAuthConfig = null
  isRedirecting = false
}

export function buildLoginRedirectUrl(currentHref: string = window.location.href): string {
  const loginUrl = new URL(getLoginUrl(), window.location.origin)
  if (!loginUrl.searchParams.has('redirect')) {
    loginUrl.searchParams.set('redirect', currentHref)
  }
  return loginUrl.toString()
}

export function handle401(): void {
  if (isRedirecting) return
  isRedirecting = true
  const redirectUrl = buildLoginRedirectUrl()
  resetAuthCheck()
  window.location.href = redirectUrl
}

export async function checkAuth(): Promise<boolean> {
  const config = await getAuthConfig()
  if (!config.enabled) return true

  if (!authCheckPromise) {
    authCheckPromise = fetch('/api/admin/product?page=1&page_size=1', {
      credentials: 'include',
    })
      .then((response) => {
        if (response.status === 401) {
          return false
        }

        if (!response.ok) {
          throw new Error(`auth probe failed with status ${response.status}`)
        }

        return true
      })
      .catch((err) => {
        authCheckPromise = null
        throw err
      })
  }

  return authCheckPromise
}
