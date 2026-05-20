/**
 * Device Auto-Registration Demo Tests [US-DV-010]
 *
 * Verifies device auto-registration behavior:
 * - S1: Auto-provisioning ON -> device auto-registers on first HMAC connect
 * - S2: Auto-provisioning OFF -> unregistered device is denied
 * - S3: Already-registered device can always connect regardless of toggle
 * - S4: Cert issuance creates Manual registration record
 * - S5: Auto-registered device shows correct registration_source via API
 *
 * Operates at MQTT protocol level with API verification.
 * Uses the seed product with model_no `demo_product`.
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import { DemoMqttDevice, connectRawMqttClient, generateHmacPassword } from './helpers/mqtt-device'
import { findSeedProductId, getProduct, updateProduct, SEED_PRODUCT_MODEL_NO } from './helpers/product-api'
import { waitForDeviceRegistration } from './helpers/device-api'
import { issueCert } from './helpers/cert-api'
import { verifyTestEnvironment } from './helpers/environment-setup'

const AUTH_SUFFIX = process.env.MQTT_AUTH_SUFFIX || 'suffix_go'
const BROKER_URL = process.env.MQTT_URL || 'mqtt://127.0.0.1:1883'

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('[US-DV-010] Device Auto-Registration', () => {

  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('US-DV-010 S1: auto-provisioning ON - device auto-registers on first HMAC connect', async ({ request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      const deviceId = `auto-reg-${Date.now()}`
      const device = new DemoMqttDevice({ productId: SEED_PRODUCT_MODEL_NO, deviceId })

      await device.connect()
      try {
        await waitForDeviceRegistration(request, deviceId, 'Auto')
      } finally {
        await device.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  test('US-DV-010 S2: auto-provisioning OFF - unregistered device is denied', async ({ request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    try {
      await updateProduct(request, productId, { auto_provisioning: false })

      const deviceId = `denied-${Date.now()}`
      const password = generateHmacPassword(deviceId, AUTH_SUFFIX)

      await expect(
        connectRawMqttClient(deviceId, SEED_PRODUCT_MODEL_NO, password, BROKER_URL),
      ).rejects.toThrow()
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  test('US-DV-010 S3: already-registered device can always connect regardless of toggle', async ({ request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      const deviceId = `reconnect-${Date.now()}`
      const device = new DemoMqttDevice({ productId: SEED_PRODUCT_MODEL_NO, deviceId })

      await device.connect()
      try {
        await waitForDeviceRegistration(request, deviceId, 'Auto')
      } finally {
        await device.disconnect()
      }

      await updateProduct(request, productId, { auto_provisioning: false })

      const reconnectedDevice = new DemoMqttDevice({ productId: SEED_PRODUCT_MODEL_NO, deviceId })
      await reconnectedDevice.connect()
      try {
        await reconnectedDevice.postProperties({ test_reconnect: true })
      } finally {
        await reconnectedDevice.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })

  test('US-DV-010 S4: cert issuance creates Manual registration record', async ({ request, demoLogger: _demoLogger }) => {
    const deviceId = `cert-manual-${Date.now()}`

    await issueCert(request, deviceId)
    await waitForDeviceRegistration(request, deviceId, 'Manual')
  })

  test('US-DV-010 S5: auto-registered device shows correct registration_source via API', async ({ request, demoLogger: _demoLogger }) => {
    const productId = await findSeedProductId(request)
    const originalProduct = await getProduct(request, productId)
    const originalAutoProv = originalProduct.auto_provisioning

    try {
      await updateProduct(request, productId, { auto_provisioning: true })

      const deviceId = `filter-auto-${Date.now()}`
      const device = new DemoMqttDevice({ productId: SEED_PRODUCT_MODEL_NO, deviceId })

      await device.connect()
      try {
        await waitForDeviceRegistration(request, deviceId, 'Auto')

        const filterResponse = await request.get(
          `/api/admin/device/status?product_id=${SEED_PRODUCT_MODEL_NO}&registration_source=Auto&device_id=${deviceId}&page=1&page_size=10`,
        )
        expect(filterResponse.ok(), 'Filtered device status query should succeed').toBeTruthy()
        const filterBody = await filterResponse.json()

        const filteredDevices = filterBody.data ?? []
        const found = filteredDevices.some(
          (d: { device_id: string; registration_source: string }) =>
            d.device_id === deviceId && d.registration_source === 'Auto',
        )
        expect(found, `Device ${deviceId} should appear in Auto-filtered results`).toBe(true)
      } finally {
        await device.disconnect()
      }
    } finally {
      await updateProduct(request, productId, { auto_provisioning: originalAutoProv })
    }
  })
})
