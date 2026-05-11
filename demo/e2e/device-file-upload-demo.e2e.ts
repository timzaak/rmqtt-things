/**
 * Device File Upload Demo Test [US-DV-007]
 *
 * Verifies the device file upload MQTT protocol:
 * - Scenario 1: Device requests upload to allowed directory, receives presigned URL (or 503 if S3 not configured)
 * - Scenario 2: Device requests upload to disallowed directory, receives no success response
 *
 * MQTT protocol:
 *   Request topic:  {productId}/{deviceId}/thing/file/upload
 *   Response topic: {productId}/{deviceId}/thing/file/upload_reply
 *   Request payload: { id, ack: 1, params: { fileName, directory, useOriginName, fileType } }
 *   Success response: { id, code: 200, data: { url, fields } }
 *   S3 unavailable:   { id, code: 503, data: "do not support file upload" }
 *
 * Directory whitelist is configured in backend config.demo.toml:
 *   directories = ["${productId}/${deviceId}/*", "public/*"]
 *
 * When directory is not allowed, the webhook handler returns HTTP 400
 * and no MQTT response is published back to the device.
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice } from './helpers/mqtt-device'
import { verifyTestEnvironment } from './helpers/environment-setup'

const PRODUCT_ID = 'demo_product'
const RESPONSE_TIMEOUT = 15_000

test.describe('[US-DV-007] Device File Upload', () => {

  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('Scenario 1: device requests upload to own directory and receives response', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `file-upload-own-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      await device.subscribeFileUploadReply()
      const responsePromise = device.waitForFileUploadResponse(RESPONSE_TIMEOUT)

      // config allows "${productId}/${deviceId}/*"
      const requestId = await device.publishFileUploadRequest({
        fileName: 'test-data.bin',
        directory: `${PRODUCT_ID}/${deviceId}`,
        useOriginName: false,
        fileType: 'application/octet-stream',
      })

      const response = await responsePromise
      expect(response.id).toBe(requestId)

      if (response.code === 200) {
        // S3 is configured and directory is allowed
        const data = response.data as { url: string; fields: Record<string, string> }
        expect(data.url).toBeDefined()
        expect(typeof data.url).toBe('string')
        expect(data.fields).toBeDefined()
      } else if (response.code === 503) {
        // S3 is not configured in the test environment
        expect(response.data).toBeDefined()
      } else {
        throw new Error(`Unexpected response code: ${response.code}`)
      }
    } finally {
      await device.disconnect()
    }
  })

  test('Scenario 1: device requests upload to public directory (globally allowed)', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `file-upload-pub-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      await device.subscribeFileUploadReply()
      const responsePromise = device.waitForFileUploadResponse(RESPONSE_TIMEOUT)

      // config allows "public/*"
      const requestId = await device.publishFileUploadRequest({
        fileName: 'shared-file.txt',
        directory: 'public',
        useOriginName: true,
        fileType: 'text/plain',
      })

      const response = await responsePromise
      expect(response.id).toBe(requestId)

      // Either 200 (S3 available) or 503 (S3 not configured)
      expect([200, 503]).toContain(response.code)
    } finally {
      await device.disconnect()
    }
  })

  test('Scenario 2: device requests upload to disallowed directory, receives no response', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `file-upload-denied-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      await device.subscribeFileUploadReply()
      // Short timeout since we expect NO response
      const responsePromise = device.waitForFileUploadResponse(5_000)

      // "restricted/secret" does not match any whitelist pattern
      await device.publishFileUploadRequest({
        fileName: 'malicious.txt',
        directory: 'restricted/secret',
        useOriginName: false,
        fileType: 'text/plain',
      })

      // The webhook returns HTTP 400 for disallowed directories,
      // so no MQTT response is published back to the device
      const result = await responsePromise.catch(() => null)
      expect(result).toBeNull()
    } finally {
      await device.disconnect()
    }
  })

  test('Scenario 2: device requests upload to another device directory, receives no response', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `file-upload-cross-${Date.now()}`
    const otherDeviceId = `file-upload-other-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      await device.subscribeFileUploadReply()
      const responsePromise = device.waitForFileUploadResponse(5_000)

      // Attempt upload to another device's directory
      // Whitelist resolves "${productId}/${deviceId}/*" to the requesting device's own directory
      await device.publishFileUploadRequest({
        fileName: 'cross-device.txt',
        directory: `${PRODUCT_ID}/${otherDeviceId}`,
        useOriginName: false,
        fileType: 'text/plain',
      })

      // Directory does not match the requesting device's whitelist
      const result = await responsePromise.catch(() => null)
      expect(result).toBeNull()
    } finally {
      await device.disconnect()
    }
  })
})
