/**
 * Device HMAC Authentication Demo Test [US-DV-001]
 *
 * Verifies the HMAC-based MQTT authentication mechanism:
 * - Scenario 1: Correct credentials authenticate successfully
 * - Scenario 2: Malformed password is rejected
 * - Scenario 3: Expired timestamp is rejected
 * - Scenario 4: Wrong HMAC signature is rejected
 *
 * This test operates at the MQTT protocol level without UI interaction.
 * The HMAC password format is: nonce.timestamp.hash
 * where hash = HMAC-SHA1(authSuffix, "{deviceId}.{nonce}.{timestamp}.{authSuffix}")
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { MqttClient } from 'mqtt'
import { createHmac, randomBytes } from 'node:crypto'
import { DemoMqttDevice, generateHmacPassword, connectRawMqttClient, disconnectRawClient } from './helpers/mqtt-device'
import { verifyTestEnvironment } from './helpers/environment-setup'

const PRODUCT_ID = 'demo_product'
const AUTH_SUFFIX = process.env.MQTT_AUTH_SUFFIX || 'suffix_go'
const BROKER_URL = process.env.MQTT_URL || 'mqtt://127.0.0.1:1883'

function connectWithCredentials(
  clientId: string,
  username: string,
  password: string,
): Promise<MqttClient> {
  return connectRawMqttClient(clientId, username, password, BROKER_URL)
}

test.describe('[US-DV-001] Device HMAC Authentication', () => {

  test.beforeAll(async () => {
    await verifyTestEnvironment(null)
  })

  test('Scenario 1: correct HMAC credentials authenticate successfully', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-auth-ok-${Date.now()}`
    const password = generateHmacPassword(deviceId, AUTH_SUFFIX)

    const client = await connectWithCredentials(deviceId, PRODUCT_ID, password)
    try {
      // Verify the connection is functional by subscribing and publishing
      const testTopic = `${PRODUCT_ID}/${deviceId}/thing/event/property/post`
      const testPayload = JSON.stringify({
        id: `verify-${Date.now()}`,
        ack: 0,
        params: { temperature: 25.0 },
      })

      await new Promise<void>((resolve, reject) => {
        client.subscribe(testTopic, { qos: 1 }, (err) => {
          if (err) { reject(err); return }
          resolve()
        })
      })

      await new Promise<void>((resolve, reject) => {
        client.publish(testTopic, testPayload, { qos: 1 }, (err) => {
          if (err) { reject(err); return }
          resolve()
        })
      })
    } finally {
      await disconnectRawClient(client)
    }
  })

  test('Scenario 1 (alt): DemoMqttDevice helper authenticates and can post properties', async ({ request, demoLogger: _demoLogger }) => {
    const deviceId = `hmac-helper-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      const temperature = 30 + Math.round(Math.random() * 1000) / 100
      await device.postProperties({ temperature })

      await expect.poll(async () => {
        const response = await request.get(
          `/api/admin/property?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
        )
        if (!response.ok()) {
          throw new Error(`GET returned ${response.status()}`)
        }
        const body = await response.json()
        const temperatureProperty = body.data?.[0]?.properties?.temperature
        return typeof temperatureProperty === 'number'
          ? temperatureProperty
          : temperatureProperty?.value
      }, { timeout: 15_000 }).toBeCloseTo(temperature, 1)
    } finally {
      await device.disconnect()
    }
  })

  test('Scenario 2: malformed password (wrong segment count) is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-bad-fmt-${Date.now()}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, 'not-a-valid-password'),
    ).rejects.toThrow()
  })

  test('Scenario 2: empty password is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-empty-pw-${Date.now()}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, ''),
    ).rejects.toThrow()
  })

  test('Scenario 2: password with only two segments is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-two-seg-${Date.now()}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, 'abc123.1700000000'),
    ).rejects.toThrow()
  })

  test('Scenario 3: expired timestamp (older than 5 minutes) is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-expired-${Date.now()}`
    // 6 minutes ago = 360 seconds, exceeds the 300-second window
    const expiredTimestamp = Math.floor(Date.now() / 1000) - 360

    const nonce = randomBytes(3).toString('hex')
    const toSign = `${deviceId}.${nonce}.${expiredTimestamp}.${AUTH_SUFFIX}`
    const hash = createHmac('sha1', AUTH_SUFFIX).update(toSign).digest('hex')
    const password = `${nonce}.${expiredTimestamp}.${hash}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, password),
    ).rejects.toThrow()
  })

  test('Scenario 3: future timestamp (more than 5 minutes ahead) is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-future-${Date.now()}`
    // 6 minutes in the future
    const futureTimestamp = Math.floor(Date.now() / 1000) + 360

    const nonce = randomBytes(3).toString('hex')
    const toSign = `${deviceId}.${nonce}.${futureTimestamp}.${AUTH_SUFFIX}`
    const hash = createHmac('sha1', AUTH_SUFFIX).update(toSign).digest('hex')
    const password = `${nonce}.${futureTimestamp}.${hash}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, password),
    ).rejects.toThrow()
  })

  test('Scenario 4: wrong HMAC signature is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-wrong-sig-${Date.now()}`
    const nonce = randomBytes(3).toString('hex')
    const timestamp = Math.floor(Date.now() / 1000)

    // Use a completely wrong hash
    const wrongHash = 'deadbeef' + 'a'.repeat(32)
    const password = `${nonce}.${timestamp}.${wrongHash}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, password),
    ).rejects.toThrow()
  })

  test('Scenario 4: signature with wrong suffix is rejected', async ({ demoLogger: _demoLogger }) => {
    const deviceId = `hmac-wrong-key-${Date.now()}`
    const nonce = randomBytes(3).toString('hex')
    const timestamp = Math.floor(Date.now() / 1000)

    // Generate signature with the wrong suffix
    const wrongSuffix = 'wrong_suffix_value'
    const toSign = `${deviceId}.${nonce}.${timestamp}.${wrongSuffix}`
    const hash = createHmac('sha1', wrongSuffix).update(toSign).digest('hex')
    const password = `${nonce}.${timestamp}.${hash}`

    await expect(
      connectWithCredentials(deviceId, PRODUCT_ID, password),
    ).rejects.toThrow()
  })
})
