import { expect } from '@playwright/test'
import { BASE_URL } from './environment-setup'

export async function issueCert(deviceId: string): Promise<void> {
  const now = new Date()
  const oneYearLater = new Date(now)
  oneYearLater.setFullYear(oneYearLater.getFullYear() + 1)

  const response = await fetch(`${BASE_URL}/api/admin/ca/cert`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      product_id: 'demo_product',
      device_id: deviceId,
      force: true,
      start_at: now.toISOString(),
      end_at: oneYearLater.toISOString(),
    }),
  })

  expect(response.ok, `Issue cert API should succeed, got ${response.status}`).toBeTruthy()
}

export async function issueCertAndGetId(deviceId: string): Promise<number> {
  await issueCert(deviceId)

  const listResponse = await fetch(
    `${BASE_URL}/api/admin/ca/cert?device_id=${deviceId}&page=1&page_size=1`,
  )
  expect(listResponse.ok, `List certs API should succeed, got ${listResponse.status}`).toBeTruthy()

  const listData = (await listResponse.json()) as { data: Array<{ id: number }> }
  expect(listData.data.length, 'Should find the issued cert in list').toBeGreaterThan(0)
  return listData.data[0].id
}

export async function updateCertStatus(
  productId: string,
  deviceId: string,
  status: number,
): Promise<void> {
  const response = await fetch(`${BASE_URL}/api/admin/ca/cert/status`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      product_id: productId,
      device_id: deviceId,
      status,
    }),
  })

  expect(
    response.ok,
    `Update cert status API should succeed, got ${response.status}`,
  ).toBeTruthy()
}
