/**
 * Access-token auto-refresh for the Herald 0.3.3 browser session model.
 *
 * Herald 0.3.3 issues short-lived access tokens (900s) alongside a long-lived
 * refresh token (carried in the `X-Auth-Refresh` cookie). Every admin API call
 * is authenticated by the `X-Auth` access cookie, which expires after 15 min —
 * so without refresh the user is bounced to the login page every 15 min.
 *
 * Strategy (401-driven, single in-flight): when any API response is a 401, we
 * call `POST /api/auth/refresh` ONCE and replay the original request. Crucially,
 * Herald's refresh is token-rotation with **reuse detection** — presenting an
 * already-rotated refresh token revokes the entire token family. Because many
 * concurrent requests can 401 in the same expiry window, all of them must share
 * a single in-flight refresh promise; a second concurrent refresh would reuse
 * the old (now-rotated) token and log the user out. On a refresh failure we do
 * NOT retry and instead hand off to `handle401()` (redirect to login).
 */
import { client } from '@/lib/api-generated/client.gen'
import { refreshToken } from '@/lib/api-generated/sdk.gen'
import { handle401, resetAuthCheck } from '@/lib/auth'

/** Single in-flight refresh promise; shared by every concurrent 401. */
let inflightRefresh: Promise<boolean> | null = null

/**
 * Refresh the access token via Herald. Serialized: concurrent callers await the
 * same promise so we never issue a second refresh while one is in flight (which
 * Herald would treat as refresh-token reuse and revoke the whole family).
 *
 * Returns true on success, false on any failure (and triggers the login redirect).
 */
export function refreshAccessToken(): Promise<boolean> {
  if (inflightRefresh) return inflightRefresh

  inflightRefresh = refreshToken()
    .then((result) => {
      // SDK defaults to throwOnError:false, so a 401 resolves as `{ error, ... }`
      // rather than rejecting. Treat an error payload as a failed refresh.
      if (result && typeof result === 'object' && 'error' in result) {
        throw new Error('refresh failed')
      }
      return true
    })
    .catch(() => {
      // Refresh rejected (refresh cookie missing/expired/revoked) — the session
      // is genuinely gone. Clear caches and bounce to login; do not retry.
      resetAuthCheck()
      handle401()
      return false
    })
    .finally(() => {
      inflightRefresh = null
    })

  return inflightRefresh
}

/** URL prefixes whose own 401 must NOT trigger a refresh (would recurse). */
const NO_REFRESH_PREFIXES = ['/api/auth/refresh', '/api/auth/logout', '/api/auth/oauth/']

function shouldSkipRefresh(url: string): boolean {
  // request.url is absolute in the browser (e.g. http://host/api/auth/refresh);
  // match on the path portion so the refresh/logout endpoints are excluded
  // regardless of host/baseUrl.
  return NO_REFRESH_PREFIXES.some((prefix) => url.includes(prefix))
}

/**
 * Install a response interceptor on the Hey Api client that, on a 401, refreshes
 * the access cookie once and replays the original request. Safe to call once at
 * startup (idempotent — re-installing is harmless, just redundant).
 */
export function installAutoRefreshInterceptor(): void {
  client.interceptors.response.use(async (response, request) => {
    if (response.status !== 401) return response
    // The refresh/logout/auth endpoints own their 401 semantics; refreshing on
    // their behalf would recurse (refresh 401 → refresh → ...).
    if (shouldSkipRefresh(request.url)) return response

    const ok = await refreshAccessToken()
    if (!ok) return response

    // Cookies were rotated by the server via Set-Cookie; the browser now sends
    // the fresh access token. Clone so the replayable body stream is intact.
    return fetch(request.clone())
  })
}
