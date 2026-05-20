/**
 * Device API Helpers
 *
 * Typed helpers for device status polling and assertions.
 */

import type { APIRequestContext } from '@playwright/test'
import { expect } from '@playwright/test'
import { SEED_PRODUCT_MODEL_NO } from './product-api'

const POLL_TIMEOUT = 15_000

export async function waitForDeviceRegistration(
  request: APIRequestContext,
  deviceId: string,
  expectedSource: string,
  productId: string = SEED_PRODUCT_MODEL_NO,
  timeoutMs: number = POLL_TIMEOUT,
): Promise<void> {
  await expect.poll(async () => {
    const response = await request.get(
      `/api/admin/device/status?product_id=${productId}&device_id=${deviceId}&page=1&page_size=10`,
    )
    if (!response.ok()) {
      return null
    }
    const body = await response.json()
    return body.data?.[0]?.registration_source ?? null
  }, { timeout: timeoutMs }).toBe(expectedSource)
}
