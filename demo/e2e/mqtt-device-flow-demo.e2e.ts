/**
 * MQTT Device Flow Demo 测试
 *
 * 使用真实 MQTT client 连接 RMQTT，验证设备上报和属性命令下发闭环。
 */

import { test, expect } from './fixtures/demo-auth.fixtures'
import type { APIRequestContext } from '@playwright/test'
import { DemoMqttDevice } from './helpers/mqtt-device'

const PRODUCT_ID = 'demo_product'
const POLL_TIMEOUT = 15_000

interface ListResponse<T> {
  data?: T[]
}

interface PropertyLatestRow {
  properties?: {
    temperature?: number | { value?: number }
  }
}

interface EventHistoryRow {
  events?: { marker?: string }
}

interface PropertyCommandRow {
  id: number
  status?: string | number
}

interface DeviceStatusRow {
  status?: string
}

test.describe('MQTT device flow demo', () => {
  test('[US-DV-003] device posts properties and admin queries them', async ({ request }) => {
    const deviceId = `demo-e2e-prop-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      const temperature = 20 + Math.round(Math.random() * 1000) / 100

      await device.postProperties({ temperature, humidity: 51, power: true })

      await expect.poll(async () => {
        const body = await getJson<ListResponse<PropertyLatestRow>>(
          request,
          `/api/admin/property?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
        )
        const temperatureProperty = body.data?.[0]?.properties?.temperature
        return typeof temperatureProperty === 'number'
          ? temperatureProperty
          : temperatureProperty?.value
      }, { timeout: POLL_TIMEOUT }).toBeCloseTo(temperature, 1)
    } finally {
      await device.disconnect()
    }
  })

  test('[US-DV-005] device posts events and admin queries them', async ({ request }) => {
    const deviceId = `demo-e2e-evt-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      const eventMarker = `mqtt-e2e-${Date.now()}`

      await device.postEvent({ event: 'mqtt_e2e_boot', marker: eventMarker })

      await expect.poll(async () => {
        const body = await getJson<ListResponse<EventHistoryRow>>(
          request,
          `/api/admin/event?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
        )
        return body.data?.some(row => row.events?.marker === eventMarker) ?? false
      }, { timeout: POLL_TIMEOUT }).toBe(true)
    } finally {
      await device.disconnect()
    }
  })

  test('[US-DV-004] admin creates command, device receives and replies', async ({ request }) => {
    const deviceId = `demo-e2e-cmd-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    try {
      const commandPromise = device.waitForCommand()
      const commandValue = { power: false, brightness: 42 }
      const createResponse = await request.post('/api/admin/property/command', {
        data: { product_id: PRODUCT_ID, device_id: deviceId, command: commandValue },
      })
      expect(createResponse.status()).toBe(201)

      const command = await commandPromise
      expect(command.data).toMatchObject(commandValue)
      expect(command.ids.length).toBeGreaterThan(0)

      await device.replyCommand(command)
      await expect.poll(async () => {
        const body = await getJson<ListResponse<PropertyCommandRow>>(
          request,
          `/api/admin/property/command?product_id=${PRODUCT_ID}&page=1&page_size=20`,
        )
        const row = body.data?.find(item => command.ids.includes(item.id))
        if (!row) {
          throw new Error(`Command ${command.ids} not found in response`)
        }
        return row.status
      }, { timeout: POLL_TIMEOUT }).toBe('Success')
    } finally {
      await device.disconnect()
    }
  })

  test('[US-DV-009] admin creates command while device offline, command queued and delivered on connect', async ({ request }) => {
    const deviceId = `demo-e2e-queued-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    const commandValue = { power: true, brightness: 80 }
    const createResponse = await request.post('/api/admin/property/command', {
      data: { product_id: PRODUCT_ID, device_id: deviceId, command: commandValue },
    })
    expect(createResponse.status()).toBe(201)

    await expect.poll(async () => {
      const body = await getJson<ListResponse<PropertyCommandRow>>(
        request,
        `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=20`,
      )
      const row = body.data?.[0]
      if (!row) {
        throw new Error('Command not found in list')
      }
      return String(row.status)
    }, { timeout: POLL_TIMEOUT }).toBe('Pending')

    // Register command waiter BEFORE connecting to avoid race condition
    // (waitForCommand only registers a Promise resolver, no MQTT connection needed)
    const commandPromise = device.waitForCommand()

    // subscribe triggers RMQTT webhook to deliver queued commands
    await device.connect()
    try {
      const command = await commandPromise
      expect(command.data).toMatchObject(commandValue)
      expect(command.ids.length).toBeGreaterThan(0)

      await device.replyCommand(command)

      await expect.poll(async () => {
        const body = await getJson<ListResponse<PropertyCommandRow>>(
          request,
          `/api/admin/property/command?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=20`,
        )
        const row = body.data?.find(item => command.ids.includes(item.id))
        if (!row) {
          throw new Error(`Command ${command.ids} not found in response`)
        }
        return String(row.status)
      }, { timeout: POLL_TIMEOUT }).toBe('Success')
    } finally {
      await device.disconnect()
    }
  })

  test('[US-DV-008] device online/offline status is tracked', async ({ request }) => {
    const deviceId = `demo-e2e-status-${Date.now()}`
    const device = new DemoMqttDevice({ productId: PRODUCT_ID, deviceId })

    await device.connect()
    await expect.poll(async () => {
      const body = await getJson<ListResponse<DeviceStatusRow>>(
        request,
        `/api/admin/device/status?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
      )
      return body.data?.[0]?.status
    }, { timeout: POLL_TIMEOUT }).toBe('Online')

    await device.disconnect()

    await expect.poll(async () => {
      const body = await getJson<ListResponse<DeviceStatusRow>>(
        request,
        `/api/admin/device/status?product_id=${PRODUCT_ID}&device_id=${deviceId}&page=1&page_size=10`,
      )
      return body.data?.[0]?.status
    }, { timeout: POLL_TIMEOUT }).toBe('Offline')
  })
})

async function getJson<T>(request: APIRequestContext, path: string): Promise<T> {
  const response = await request.get(path)
  if (!response.ok()) {
    const text = await response.text()
    throw new Error(`GET ${path} returned ${response.status()}: ${text}`)
  }
  return response.json()
}
