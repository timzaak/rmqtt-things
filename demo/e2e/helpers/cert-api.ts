import { expect, type APIRequestContext } from '@playwright/test'

export async function issueCert(request: APIRequestContext, deviceId: string): Promise<void> {
  const now = new Date()
  const oneYearLater = new Date(now)
  oneYearLater.setFullYear(oneYearLater.getFullYear() + 1)

  const response = await request.post('/api/admin/ca/cert', {
    data: {
      product_id: 'demo_product',
      device_id: deviceId,
      force: true,
      start_at: now.toISOString(),
      end_at: oneYearLater.toISOString(),
    },
  })

  expect(response.ok(), `Issue cert API should succeed, got ${response.status()}`).toBeTruthy()
}

export async function issueCertAndGetId(request: APIRequestContext, deviceId: string): Promise<number> {
  await issueCert(request, deviceId)

  const listResponse = await request.get(`/api/admin/ca/cert?device_id=${deviceId}&page=1&page_size=1`)
  expect(listResponse.ok(), `List certs API should succeed, got ${listResponse.status()}`).toBeTruthy()

  const listData = (await listResponse.json()) as { data: Array<{ id: number }> }
  expect(listData.data.length, 'Should find the issued cert in list').toBeGreaterThan(0)
  return listData.data[0].id
}

export async function updateCertStatus(
  request: APIRequestContext,
  productId: string,
  deviceId: string,
  status: number,
): Promise<void> {
  const response = await request.patch('/api/admin/ca/cert/status', {
    data: {
      product_id: productId,
      device_id: deviceId,
      status,
    },
  })

  expect(
    response.ok(),
    `Update cert status API should succeed, got ${response.status()}`,
  ).toBeTruthy()
}
