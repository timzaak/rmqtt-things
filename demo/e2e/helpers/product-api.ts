/**
 * Product API Helpers
 *
 * Typed API helpers for product CRUD operations.
 * Uses APIRequestContext (shared auth with page) consistent with project patterns.
 */

import type { APIRequestContext } from '@playwright/test'
import { expect } from '@playwright/test'
import { assertOk, getJson } from './api'

export const SEED_PRODUCT_MODEL_NO = 'demo_product'

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

export interface ProductResponse {
  id: number
  name: string
  model_no: string
  description: string | null
  auto_provisioning: boolean
  status: string
  created_at: string
  updated_at: string
}

// ---------------------------------------------------------------------------
// Product helpers
// ---------------------------------------------------------------------------

export async function findSeedProductId(request: APIRequestContext): Promise<number> {
  const response = await request.get('/api/admin/product')
  expect(response.ok(), `GET /api/admin/product should succeed, got ${response.status()}`).toBeTruthy()
  const body = await response.json()
  const products: Array<{ id: number; model_no: string }> = body.data ?? body
  const seed = products.find((p) => p.model_no === SEED_PRODUCT_MODEL_NO)
  expect(seed, `Seed product with model_no=${SEED_PRODUCT_MODEL_NO} should exist`).toBeDefined()
  return seed!.id
}

export async function createProduct(
  request: APIRequestContext,
  data: { name: string; model_no: string; description?: string },
): Promise<ProductResponse> {
  const response = await request.post('/api/admin/product', { data })
  await assertOk(response)
  return response.json()
}

export async function getProduct(
  request: APIRequestContext,
  id: number,
): Promise<ProductResponse> {
  return getJson<ProductResponse>(request, `/api/admin/product/${id}`)
}

export async function updateProduct(
  request: APIRequestContext,
  id: number,
  data: { name?: string; description?: string; auto_provisioning?: boolean },
): Promise<ProductResponse> {
  const current = await getProduct(request, id)
  const merged = {
    name: data.name ?? current.name,
    description: data.description ?? (current.description ?? ''),
    auto_provisioning: data.auto_provisioning ?? current.auto_provisioning,
  }
  const response = await request.patch(`/api/admin/product/${id}`, { data: merged })
  await assertOk(response)
  return response.json()
}
