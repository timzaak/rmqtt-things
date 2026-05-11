import type { APIRequestContext } from '@playwright/test'

export async function getJson<T>(request: APIRequestContext, path: string): Promise<T> {
  const response = await request.get(path)
  if (!response.ok()) {
    const text = await response.text()
    throw new Error(`GET ${path} returned ${response.status()}: ${text}`)
  }
  return response.json()
}
