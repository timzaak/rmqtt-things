import { createHmac, randomBytes } from 'node:crypto'
import mqtt, { type IClientOptions, type MqttClient } from 'mqtt'

export interface DemoMqttDeviceOptions {
  productId: string
  deviceId: string
  brokerUrl?: string
  authSuffix?: string
}

export interface PropertyCommandMessage {
  id: string
  params: Record<string, unknown>
  raw: unknown
}

/**
 * Action / service invocation message envelope (thing-model-extension design
 * §5.1 / §5.2). Mirrors `PropertyCommandMessage`: the platform publishes the
 * standard request `{id: "action:{db_id}", params, ack: 1}` and devices reply
 * with `{id, code, data?}` on the `.../set_reply` topic. `raw` keeps the full
 * decoded document for protocol assertions beyond `id` / `params`.
 */
export interface ActionCommandMessage {
  id: string
  params: Record<string, unknown>
  raw: unknown
}

export interface OtaUpgradeMessage {
  id: string
  ack: number
  params: Array<{
    key: string
    file_url: string
    version: string
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
  private actionWaiters: Array<(message: ActionCommandMessage) => void> = []
  private otaUpgradeWaiters: Array<(message: OtaUpgradeMessage) => void> = []
  private fileUploadWaiters: Array<(response: FileUploadResponse) => void> = []
  /**
   * service_type -> subscribed action set topics. Tracked so the message
   * handler can dispatch an incoming `thing/service/{serviceType}/set` to the
   * right waiters without re-parsing the topic string on every message.
   * Keys are the full set topics (`{productId}/{deviceId}/thing/service/{serviceType}/set`).
   */
  private subscribedActionTopics: Set<string> = new Set()

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

  /**
   * Connect the device and establish the subscriptions required to receive
   * property / action commands before returning.
   *
   * 离线投递竞态修复（DE-D05 缺陷 C，方案 1）：
   *
   * RMQTT auto-subscription（`+/${clientid}/thing/service/+/set`）在 connect
   * 握手期间触发 `client_subscribe` webhook -> `service_set_subscribe` handler
   * 原子排空所有 Pending property/action 命令并立即 publish（handlers.rs:634）。
   * 该 publish 可能发生在设备自身对相应 topic 的 SUBACK 完成之前；若设备没有
   * 已确认的订阅，消息因无匹配订阅者被丢弃（QoS-0 / 非 durable session）。
   *
   * 解决：在 `connect` 事件后、返回前，显式订阅 property/set（始终）以及调用方
   * 声明的 service_type set topic（`options.serviceTypes`），并 await mqtt.js 的
   * subscribe 回调（即 SUBACK 完成）。这样当 connect() 返回时，设备自身的订阅已
   * 确认就绪，broker 排空投递时即有匹配订阅者。同时把 service topic 记入
   * `subscribedActionTopics`，确保即使消息在 subscribeAction() 调用前到达，
   * message handler 的 action 分支也能正确 dispatch（否则 handler 因集合为空而
   * 丢弃消息——DE-D04 的 waitForAction 解决了 waiter 注册，但没解决 dispatch 与
   * 订阅本身的竞态）。
   *
   * 不回退 DE-D01 协议迁移（.params）与 DE-D02 action API；不破坏在线场景
   * （Scen1/Scen3 与其它在线用例的既有订阅顺序）：在线场景的命令由 UI/API 触发
   * 排空，此时 connect() 已返回、订阅已就绪，pre-subscribe 仅是更早建立同样的
   * 订阅，无副作用。
   *
   * @param options.serviceTypes 离线投递场景下设备预先订阅的 action service_type
   *   列表。在线场景无需传（订阅仍由 subscribeAction() 按需建立）。
   */
  async connect(options: { serviceTypes?: string[] } = {}): Promise<void> {
    const client = mqtt.connect(this.brokerUrl, this.buildClientOptions())
    this.client = client

    client.on('message', (topic, payload) => {
      if (topic === this.setTopic) {
        const command = this.parseCommand(payload.toString())
        const waiters = this.commandWaiters.splice(0)
        for (const resolve of waiters) {
          resolve(command)
        }
      } else if (this.subscribedActionTopics.has(topic)) {
        // Action / service invocation set topic
        // (thing-model-extension design §4.1 / §5.2). Same standard envelope
        // shape as property commands; parsed identically.
        const action = this.parseAction(payload.toString())
        const waiters = this.actionWaiters.splice(0)
        for (const resolve of waiters) {
          resolve(action)
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

    // 显式订阅 property/set 并 await SUBACK（connect() 返回前完成）。
    await this.subscribe(this.setTopic)

    // 离线投递场景：预先订阅声明的 service_type set topic，使 broker 排空投递
    // 时设备已有确认的订阅，且 message handler 的 action 分支可正确 dispatch。
    // 在线场景不传 serviceTypes，保持原有 subscribeAction() 按需订阅的顺序。
    if (options.serviceTypes && options.serviceTypes.length > 0) {
      for (const serviceType of options.serviceTypes) {
        const topic = this.actionSetTopicFor(serviceType)
        await this.subscribe(topic)
        this.subscribedActionTopics.add(topic)
      }
    }
  }

  async disconnect(): Promise<void> {
    if (!this.client) {
      return
    }
    const client = this.client
    this.client = undefined
    // Clear tracked action subscriptions; a fresh connect() re-establishes them
    // via subscribeAction(). Avoids stale topic->waiter dispatch on reconnect.
    this.subscribedActionTopics.clear()
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
    params: Array<{ key: string; version: string }>,
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
    })
  }

  // -------------------------------------------------------------------------
  // Action / service invocation support (thing-model-extension design §4.1/§5.2)
  //
  // Mirrors the property methods but parameterised by `serviceType`. The action
  // set topic is `{productId}/{deviceId}/thing/service/{serviceType}/set` and
  // the reply topic is `.../set_reply`. Devices subscribe per service_type,
  // receive the standard `{id: "action:{db_id}", params, ack: 1}` envelope and
  // reply with `{id, code, data?}`; any 2xx is success.
  // -------------------------------------------------------------------------

  /** Build the action set topic for a given service type. */
  actionSetTopicFor(serviceType: string): string {
    return `${this.productId}/${this.deviceId}/thing/service/${serviceType}/set`
  }

  /** Build the action set_reply topic for a given service type. */
  actionSetReplyTopicFor(serviceType: string): string {
    return `${this.productId}/${this.deviceId}/thing/service/${serviceType}/set_reply`
  }

  /**
   * Subscribe to the action set topic for `serviceType`. Must be called before
   * the platform invokes the action so the broker's subscribe hook triggers
   * delivery of any queued invocations (drain is triggered by the subscribe
   * webhook, not by connect — design §3.1 / §5.3).
   */
  async subscribeAction(serviceType: string): Promise<void> {
    const topic = this.actionSetTopicFor(serviceType)
    await this.subscribe(topic)
    this.subscribedActionTopics.add(topic)
  }

  /**
   * Resolve with the next action invocation received on a subscribed
   * `thing/service/{serviceType}/set` topic. Register the waiter BEFORE
   * triggering the invocation to avoid the eager publish-on-subscribe race.
   */
  waitForAction(timeoutMs = 15_000): Promise<ActionCommandMessage> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.actionWaiters = this.actionWaiters.filter(waiter => waiter !== resolve)
        reject(new Error('Timed out waiting for action invocation'))
      }, timeoutMs)

      this.actionWaiters.push((message) => {
        globalThis.clearTimeout(timeout)
        resolve(message)
      })
    })
  }

  /**
   * Reply to an action invocation on its `.../set_reply` topic. Publishes the
   * standard `{id, code, data?}` envelope; `data` is optional and omitted when
   * not provided. Mirrors `replyCommand` but publishes to the service-specific
   * reply topic derived from the message topic (or, when the raw envelope lacks
   * a topic, the known subscribed topic).
   *
   * Any 2xx `code` is treated as success by the backend (design §5.1).
   */
  async replyAction(
    message: ActionCommandMessage,
    code = 200,
    options: { serviceType?: string; data?: Record<string, unknown> } = {},
  ): Promise<void> {
    const serviceType = options.serviceType ?? this.resolveServiceType(message)
    const replyTopic = this.actionSetReplyTopicFor(serviceType)
    const payload: { id: string; code: number; data?: Record<string, unknown> } = {
      id: message.id,
      code,
    }
    if (options.data !== undefined) {
      payload.data = options.data
    }
    await this.publishJson(replyTopic, payload)
  }

  private parseAction(payload: string): ActionCommandMessage {
    const raw = JSON.parse(payload)
    return {
      id: String(raw.id),
      params: raw.params || {},
      raw,
    }
  }

  /**
   * Best-effort extraction of the service_type from a received action message.
   * The platform-set id is `action:{db_id}` (no service_type), so when the
   * caller does not pass `serviceType` explicitly we fall back to the single
   * subscribed action topic if exactly one is tracked.
   */
  private resolveServiceType(_message: ActionCommandMessage): string {
    if (this.subscribedActionTopics.size === 1) {
      const topic = Array.from(this.subscribedActionTopics)[0]
      // topic = {productId}/{deviceId}/thing/service/{serviceType}/set
      const segments = topic.split('/')
      const serviceType = segments[segments.length - 2]
      if (serviceType && segments[segments.length - 1] === 'set') {
        return serviceType
      }
    }
    throw new Error(
      'replyAction could not resolve service_type; pass options.serviceType explicitly',
    )
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
    return {
      id: String(raw.id),
      params: raw.params || {},
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
