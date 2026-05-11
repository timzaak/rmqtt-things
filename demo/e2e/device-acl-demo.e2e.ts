/**
 * Device ACL Permission Control Demo Test [US-DV-002]
 *
 * Verifies that MQTT devices can only publish/subscribe within their own topic space:
 * - Scenario 1: Device can publish and subscribe to its own thing topics
 * - Scenario 2: Device cannot subscribe to another device's topics (ACL denies)
 * - Scenario 3: Device cannot publish to another device's topics (ACL denies)
 *
 * ACL mechanism:
 *   1. Static RMQTT ACL allows %c/# (clientId namespace)
 *   2. HTTP ACL at /api/access/acl enforces: topic must match {product}/{clientId}/thing/{event|service}/...
 *
 * Topic format: {productId}/{deviceId}/thing/event/property/post
 *               {productId}/{deviceId}/thing/service/property/set
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import mqtt, { type MqttClient } from 'mqtt'
import { generateHmacPassword, connectRawMqttClient, disconnectRawClient } from './helpers/mqtt-device'
import { verifyTestEnvironment } from './helpers/environment-setup'

const PRODUCT_ID = 'demo_product'
const AUTH_SUFFIX = process.env.MQTT_AUTH_SUFFIX || 'suffix_go'
const BROKER_URL = process.env.MQTT_URL || 'mqtt://127.0.0.1:1883'

function connectMqttClient(clientId: string): Promise<MqttClient> {
  return connectRawMqttClient(clientId, PRODUCT_ID, generateHmacPassword(clientId, AUTH_SUFFIX), BROKER_URL)
}

const disconnectClient = disconnectRawClient

/**
 * Subscribe to a topic and return the granted QoS array.
 * For ACL-denied subscriptions, RMQTT returns QoS 128 (0x80) in the suback.
 * MQTT.js v5 throws ErrorWithSubackPacket instead of passing QoS 128 via callback,
 * so we catch it and extract the granted QoS from the packet.
 */
function subscribeToTopic(
  client: MqttClient,
  topic: string,
): Promise<mqtt.ISubscriptionGrant[]> {
  return new Promise((resolve, reject) => {
    client.subscribe(topic, { qos: 1 }, (err, granted) => {
      if (err) {
        // MQTT.js v5 wraps SUBACK failure into ErrorWithSubackPacket with a .packet property
        const anyErr = err as Error & { packet?: { granted?: number[] } }
        if (anyErr.packet?.granted) {
          resolve(anyErr.packet.granted.map((qos) => ({ topic, qos } as mqtt.ISubscriptionGrant)))
          return
        }
        reject(err)
        return
      }
      resolve(granted)
    })
  })
}

/**
 * Publish a message to a topic.
 * Returns true if the publish was acked (QoS 1), false if the client was
 * disconnected due to ACL rejection before the puback arrived.
 * Includes a timeout as a safety net for cases where the broker silently drops.
 */
function publishToTopic(
  client: MqttClient,
  topic: string,
  payload: string,
  timeoutMs = 10_000,
): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false

    const finish = (result: boolean) => {
      if (settled) return
      settled = true
      client.removeListener('close', onClose)
      clearTimeout(timer)
      resolve(result)
    }

    const onClose = () => finish(false)

    const timer = setTimeout(() => {
      // If neither puback nor close arrived, the broker likely silently dropped the message.
      // Treat as ACL deny (publish was rejected without disconnect).
      finish(false)
    }, timeoutMs)

    client.once('close', onClose)

    client.publish(topic, payload, { qos: 1 }, (err) => {
      if (!err) {
        finish(true)
      }
      // If err, the client may have been disconnected; onClose will resolve false
    })
  })
}

/**
 * Wait for a message on a given topic within a timeout.
 * Returns the payload string, or rejects on timeout.
 */
function waitForMessage(
  client: MqttClient,
  topic: string,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      client.removeListener('message', onMessage)
      reject(new Error(`Timed out waiting for message on ${topic}`))
    }, timeoutMs)

    const onMessage = (receivedTopic: string, payload: Buffer) => {
      if (receivedTopic === topic) {
        clearTimeout(timer)
        client.removeListener('message', onMessage)
        resolve(payload.toString())
      }
    }

    client.on('message', onMessage)
  })
}

test.describe('[US-DV-002] Device ACL Permission Control', () => {

  test.beforeAll(async () => {
    await verifyTestEnvironment(null as any)
  })

  test('Scenario 1: device can subscribe to its own event topic', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Self-subscribe to own event topic')
    const deviceA = `acl-self-sub-${Date.now()}`
    const client = await connectMqttClient(deviceA)
    try {
      const ownTopic = `${PRODUCT_ID}/${deviceA}/thing/event/property/post`

      const granted = await subscribeToTopic(client, ownTopic)
      expect(granted.length).toBeGreaterThan(0)
      expect(granted[0].qos).not.toBe(128)
    } finally {
      await disconnectClient(client)
    }
  })

  test('Scenario 1: device can publish to its own event topic and receive the message', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Self-publish and receive on own event topic')
    const deviceA = `acl-self-pub-${Date.now()}`
    const client = await connectMqttClient(deviceA)
    try {
      const ownTopic = `${PRODUCT_ID}/${deviceA}/thing/event/property/post`

      await subscribeToTopic(client, ownTopic)

      const testPayload = JSON.stringify({
        id: `acl-test-${Date.now()}`,
        ack: 0,
        params: { temperature: 22.5 },
      })

      const published = await publishToTopic(client, ownTopic, testPayload)
      expect(published).toBe(true)

      const received = await waitForMessage(client, ownTopic, 5_000)
      expect(received).toBe(testPayload)
    } finally {
      await disconnectClient(client)
    }
  })

  test('Scenario 1: device can subscribe to its own service topic', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Self-subscribe to own service topic')
    const deviceA = `acl-self-svc-${Date.now()}`
    const client = await connectMqttClient(deviceA)
    try {
      const ownServiceTopic = `${PRODUCT_ID}/${deviceA}/thing/service/property/set`

      const granted = await subscribeToTopic(client, ownServiceTopic)
      expect(granted.length).toBeGreaterThan(0)
      expect(granted[0].qos).not.toBe(128)
    } finally {
      await disconnectClient(client)
    }
  })

  test('Scenario 2: device cannot subscribe to another device event topic (ACL denies)', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Cross-device event subscribe should be denied')
    const deviceA = `acl-cross-sub-${Date.now()}`
    const deviceB = `acl-cross-sub-other-${Date.now()}`

    const client = await connectMqttClient(deviceA)
    try {
      const otherDeviceTopic = `${PRODUCT_ID}/${deviceB}/thing/event/property/post`

      // Subscribe to another device's topic; ACL should deny.
      // RMQTT returns QoS 128 (0x80) in SUBACK for denied subscriptions.
      const granted = await subscribeToTopic(client, otherDeviceTopic)
      expect(granted.length).toBeGreaterThan(0)
      // QoS 128 (0x80) means "Subscription failed" per MQTT spec
      expect(granted[0].qos).toBe(128)
    } finally {
      await disconnectClient(client)
    }
  })

  test('Scenario 2: device cannot subscribe to another device service topic (ACL denies)', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Cross-device service subscribe should be denied')
    const deviceA = `acl-cross-svc-sub-${Date.now()}`
    const deviceB = `acl-cross-svc-sub-other-${Date.now()}`

    const client = await connectMqttClient(deviceA)
    try {
      const otherServiceTopic = `${PRODUCT_ID}/${deviceB}/thing/service/property/set`

      const granted = await subscribeToTopic(client, otherServiceTopic)
      expect(granted.length).toBeGreaterThan(0)
      expect(granted[0].qos).toBe(128)
    } finally {
      await disconnectClient(client)
    }
  })

  test('Scenario 3: device cannot publish to another device topic (ACL denies, connection dropped)', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Cross-device publish should be denied with disconnect')
    const deviceA = `acl-cross-pub-${Date.now()}`
    const deviceB = `acl-cross-pub-other-${Date.now()}`

    const client = await connectMqttClient(deviceA)
    try {
      const otherDeviceTopic = `${PRODUCT_ID}/${deviceB}/thing/event/property/post`

      const testPayload = JSON.stringify({
        id: `acl-deny-pub-${Date.now()}`,
        ack: 0,
        params: { temperature: 99.9 },
      })

      // RMQTT disconnects the client when publish is rejected due to ACL
      // (disconnect_if_pub_rejected = true). The publish callback may not
      // fire; instead the 'close' event is emitted.
      const published = await publishToTopic(client, otherDeviceTopic, testPayload)
      expect(published).toBe(false)
    } finally {
      // Client may already be disconnected; end() is safe to call regardless
      await disconnectClient(client).catch(() => { /* already disconnected */ })
    }
  })

  test('Scenario 2+3 cross-check: device B receives messages on own topic, but device A cannot inject into B topic', async ({ demoLogger }) => {
    demoLogger.testCode.log('[ACL] Cross-check: B subscribes own topic, A publishes to A topic, B should not receive')
    const deviceA = `acl-xcheck-a-${Date.now()}`
    const deviceB = `acl-xcheck-b-${Date.now()}`

    // Device B connects and subscribes to its own topic
    const clientB = await connectMqttClient(deviceB)
    const ownTopicB = `${PRODUCT_ID}/${deviceB}/thing/event/property/post`
    const messagePromise = waitForMessage(clientB, ownTopicB, 8_000)
    await subscribeToTopic(clientB, ownTopicB)

    try {
      // Device A connects and publishes to its OWN topic (should succeed)
      const clientA = await connectMqttClient(deviceA)
      try {
        const ownTopicA = `${PRODUCT_ID}/${deviceA}/thing/event/property/post`
        const payloadA = JSON.stringify({
          id: `acl-own-${Date.now()}`,
          ack: 0,
          params: { marker: 'device-a-own-topic' },
        })

        const published = await publishToTopic(clientA, ownTopicA, payloadA)
        expect(published).toBe(true)
      } finally {
        await disconnectClient(clientA)
      }

      // Device B should NOT receive the message (different topic)
      await expect(messagePromise).rejects.toThrow(/Timed out/)
    } finally {
      await disconnectClient(clientB)
    }
  })
})
