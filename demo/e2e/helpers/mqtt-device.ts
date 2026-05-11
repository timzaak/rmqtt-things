import { createHmac, randomBytes } from 'node:crypto'
import mqtt, { type IClientOptions, type MqttClient } from 'mqtt'

export interface DemoMqttDeviceOptions {
  productId: string
  deviceId: string
  brokerUrl?: string
  authSuffix?: string
}

export interface PropertyCommandMessage {
  id: string | number
  ids: number[]
  data: Record<string, unknown>
  raw: unknown
}

export interface OtaUpgradeMessage {
  id: string
  params: Array<{
    key: string
    file_url: string
    version: number
    log: unknown
  }>
}

export interface FileUploadResponse {
  id: string
  code: number
  data?: {
    url?: string
    fields?: Record<string, string>
  } | string
}

export function generateHmacPassword(deviceId: string, authSuffix: string): string {
  const nonce = randomBytes(3).toString('hex')
  const timestamp = Math.floor(Date.now() / 1000)
  const toSign = `${deviceId}.${nonce}.${timestamp}.${authSuffix}`
  const hash = createHmac('sha1', authSuffix).update(toSign).digest('hex')
  return `${nonce}.${timestamp}.${hash}`
}

export async function connectRawMqttClient(
  clientId: string,
  username: string,
  password: string,
  brokerUrl?: string,
): Promise<MqttClient> {
  const url = brokerUrl || process.env.MQTT_URL || 'mqtt://127.0.0.1:1883'
  return new Promise<MqttClient>((resolve, reject) => {
    const client = mqtt.connect(url, {
      clientId,
      username,
      password,
      clean: true,
      reconnectPeriod: 0,
      connectTimeout: 10_000,
    })

    const cleanup = () => {
      client.removeAllListeners('connect')
      client.removeAllListeners('error')
    }

    const onError = (err: Error) => {
      cleanup()
      reject(err)
    }

    client.once('connect', () => {
      cleanup()
      client.on('error', () => {})
      resolve(client)
    })

    client.once('error', onError)
  })
}

export async function disconnectRawClient(client: MqttClient): Promise<void> {
  if (!client.connected || client.disconnected) {
    return
  }
  await new Promise<void>((resolve) => client.end(false, {}, () => resolve()))
}

export class DemoMqttDevice {
  readonly productId: string
  readonly deviceId: string
  readonly setTopic: string
  readonly setReplyTopic: string
  readonly propertyPostTopic: string
  readonly eventPostTopic: string
  readonly otaUpgradeTopic: string
  readonly otaVersionReportTopic: string
  readonly fileUploadTopic: string
  readonly fileUploadReplyTopic: string

  private readonly brokerUrl: string
  private readonly authSuffix: string
  private client?: MqttClient
  private commandWaiters: Array<(message: PropertyCommandMessage) => void> = []
  private otaUpgradeWaiters: Array<(message: OtaUpgradeMessage) => void> = []
  private fileUploadWaiters: Array<(response: FileUploadResponse) => void> = []

  constructor(options: DemoMqttDeviceOptions) {
    this.productId = options.productId
    this.deviceId = options.deviceId
    this.brokerUrl = options.brokerUrl || process.env.MQTT_URL || 'mqtt://127.0.0.1:1883'
    this.authSuffix = options.authSuffix || process.env.MQTT_AUTH_SUFFIX || 'suffix_go'

    this.setTopic = `${this.productId}/${this.deviceId}/thing/service/property/set`
    this.setReplyTopic = `${this.productId}/${this.deviceId}/thing/service/property/set_reply`
    this.propertyPostTopic = `${this.productId}/${this.deviceId}/thing/event/property/post`
    this.eventPostTopic = `${this.productId}/${this.deviceId}/thing/event/test/post`
    this.otaUpgradeTopic = `/${this.productId}/${this.deviceId}/ota/upgrade`
    this.otaVersionReportTopic = `${this.productId}/${this.deviceId}/ota/version`
    this.fileUploadTopic = `${this.productId}/${this.deviceId}/thing/file/upload`
    this.fileUploadReplyTopic = `${this.productId}/${this.deviceId}/thing/file/upload_reply`
  }

  async connect(): Promise<void> {
    const client = mqtt.connect(this.brokerUrl, this.buildClientOptions())
    this.client = client

    client.on('message', (topic, payload) => {
      if (topic === this.setTopic) {
        const command = this.parseCommand(payload.toString())
        const waiters = this.commandWaiters.splice(0)
        for (const resolve of waiters) {
          resolve(command)
        }
      } else if (topic === this.otaUpgradeTopic) {
        const upgrade = JSON.parse(payload.toString()) as OtaUpgradeMessage
        const waiters = this.otaUpgradeWaiters.splice(0)
        for (const resolve of waiters) {
          resolve(upgrade)
        }
      } else if (topic === this.fileUploadReplyTopic) {
        const response = JSON.parse(payload.toString()) as FileUploadResponse
        const waiters = this.fileUploadWaiters.splice(0)
        for (const resolve of waiters) {
          resolve(response)
        }
      }
    })

    await new Promise<void>((resolve, reject) => {
      client.once('connect', () => resolve())
      client.once('error', reject)
    })

    client.on('error', () => {
      // prevent unhandled exception on connection drop
    })

    await this.subscribe(this.setTopic)
  }

  async disconnect(): Promise<void> {
    if (!this.client) {
      return
    }
    const client = this.client
    this.client = undefined
    await new Promise<void>((resolve) => client.end(false, {}, () => resolve()))
  }

  async postProperties(params: Record<string, unknown>): Promise<void> {
    await this.publishJson(this.propertyPostTopic, {
      id: `property-${Date.now()}`,
      ack: 0,
      params,
    })
  }

  async postEvent(params: Record<string, unknown>): Promise<void> {
    await this.publishJson(this.eventPostTopic, {
      id: `event-${Date.now()}`,
      ack: 0,
      params,
    })
  }

  waitForCommand(timeoutMs = 15_000): Promise<PropertyCommandMessage> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.commandWaiters = this.commandWaiters.filter(waiter => waiter !== resolve)
        reject(new Error(`Timed out waiting for property command on ${this.setTopic}`))
      }, timeoutMs)

      this.commandWaiters.push((message) => {
        globalThis.clearTimeout(timeout)
        resolve(message)
      })
    })
  }

  waitForOtaUpgrade(timeoutMs = 15_000): Promise<OtaUpgradeMessage> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.otaUpgradeWaiters = this.otaUpgradeWaiters.filter(waiter => waiter !== resolve)
        reject(new Error(`Timed out waiting for OTA upgrade on ${this.otaUpgradeTopic}`))
      }, timeoutMs)

      this.otaUpgradeWaiters.push((message) => {
        globalThis.clearTimeout(timeout)
        resolve(message)
      })
    })
  }

  async subscribeOtaUpgrade(): Promise<void> {
    await this.subscribe(this.otaUpgradeTopic)
  }

  async subscribeFileUploadReply(): Promise<void> {
    await this.subscribe(this.fileUploadReplyTopic)
  }

  async publishFileUploadRequest(params: {
    fileName: string
    directory: string
    useOriginName: boolean
    fileType: string
  }): Promise<string> {
    const id = `file-upload-${Date.now()}`
    await this.publishJson(this.fileUploadTopic, {
      id,
      ack: 1,
      params,
    })
    return id
  }

  waitForFileUploadResponse(timeoutMs = 15_000): Promise<FileUploadResponse> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.fileUploadWaiters = this.fileUploadWaiters.filter(waiter => waiter !== resolve)
        reject(new Error(`Timed out waiting for file upload response on ${this.fileUploadReplyTopic}`))
      }, timeoutMs)

      this.fileUploadWaiters.push((response) => {
        globalThis.clearTimeout(timeout)
        resolve(response)
      })
    })
  }

  async publishOtaVersionReport(
    params: Array<{ key: string; version: number }>,
  ): Promise<void> {
    await this.publishJson(this.otaVersionReportTopic, {
      id: `ota-report-${Date.now()}`,
      ack: 0,
      params,
    })
  }

  async replyCommand(command: PropertyCommandMessage, code = 200): Promise<void> {
    await this.publishJson(this.setReplyTopic, {
      id: command.id,
      code,
      data: command.ids,
    })
  }

  private buildClientOptions(): IClientOptions {
    return {
      clientId: this.deviceId,
      username: this.productId,
      password: this.generatePassword(),
      clean: true,
      reconnectPeriod: 0,
      connectTimeout: 10_000,
    }
  }

  private generatePassword(): string {
    return generateHmacPassword(this.deviceId, this.authSuffix)
  }

  private async subscribe(topic: string): Promise<void> {
    const client = this.requireClient()
    await new Promise<void>((resolve, reject) => {
      client.subscribe(topic, { qos: 1 }, (error) => {
        if (error) {
          reject(error)
          return
        }
        resolve()
      })
    })
  }

  private async publishJson(topic: string, payload: unknown): Promise<void> {
    const client = this.requireClient()
    await new Promise<void>((resolve, reject) => {
      client.publish(topic, JSON.stringify(payload), { qos: 1 }, (error) => {
        if (error) {
          reject(error)
          return
        }
        resolve()
      })
    })
  }

  private parseCommand(payload: string): PropertyCommandMessage {
    const raw = JSON.parse(payload)
    const params = raw.params || {}
    return {
      id: raw.id,
      ids: Array.isArray(params.ids) ? params.ids : [],
      data: params.data || {},
      raw,
    }
  }

  private requireClient(): MqttClient {
    if (!this.client) {
      throw new Error('MQTT client is not connected')
    }
    return this.client
  }
}
