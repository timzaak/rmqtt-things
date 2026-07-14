[English](README.md) | [中文](README_zh.md)

# RMQTT Things

基于 [RMQTT](https://github.com/rmqtt/rmqtt) 的物联网物模型管理平台，全程使用 AI 开发，内置完整 skill 配置，支持 AI 驱动二次开发。

**在线演示：** https://mqtt.fornetcode.com

演示登录账号（Herald）：

| 字段 | 值 |
|---|---|
| 邮箱 | `admin@rmqtt-things.local` |
| 密码 | `password` |
| Realm | `rmqtt` |

<!-- TODO: 添加管理界面截图 -->

## 模拟设备接入（MQTTX）

用 [MQTTX](https://mqttx.app/) 模拟一台设备接入演示服务器。鉴权为 HMAC-SHA1 签名密码——演示服务器的时间戳容差为 7 天（`timestamp_tolerance_secs`），生成一次即可反复测试：

> 真实设备：先用 SNTP 或通信模组校准时钟；每次发送 MQTT `CONNECT`（含自动重连）前都要重新生成密码，禁止用过期密码重连——生产默认容差为 5 分钟。

```bash
DEVICE_ID="mqttx_demo_001"; SUFFIX="4f39c2635d373677b95edc460bb99ba4"
NONCE="a1b2c3"; TS=$(date +%s)
HASH=$(printf '%s' "${DEVICE_ID}.${NONCE}.${TS}.${SUFFIX}" | openssl dgst -sha1 -hmac "$SUFFIX" | awk '{print $NF}')
echo "${NONCE}.${TS}.${HASH}"   # 粘贴到 MQTTX 的 Password
```

MQTTX 连接：Host `152.32.249.178`，Port `1883`（明文 TCP），Client ID = `mqttx_demo_001`，Username = `test-1`（产品 `model_no`）。`test-1` 已开启自动注册，新设备 ID 首次连接即自动建档。

发布属性到 `test-1/mqttx_demo_001/thing/event/property/post`：

```json
{"id":"1","params":{"temperature":25.5,"humidity":60},"ack":0}
```

验证：`curl -s "https://mqtt.fornetcode.com/api/admin/property?product_id=test-1&device_id=mqttx_demo_001"`

命令由 `test-1/mqttx_demo_001/thing/service/property/set` 下发（自动订阅，无需手动 SUBSCRIBE）。在 `.../set_reply` 回复，`id` 用收到的值：

```json
{"id":"<收到的id>","code":200,"data":{}}
```

完整 topic 规范与多语言密码生成代码见[物模型规范](docs/tutorials/thing-model-spec.md)与[设备接入指南](docs/tutorials/device-integration.md)。

## 为什么关注这个项目

这个项目不是又一个 IoT 平台。它的重点是：**展示如何用 AI 完整开发一个生产级项目，并且让后续迭代也能用 AI 完成。**

项目内置了一套 skill 系统（`.claude/` 目录），把需求、设计、编码、测试串成完整流水线。配合 Claude Code 或 AidCode + 国产大模型，clone 下来就能用 AI 做二开。你只需要描述需求，AI 帮你走完剩下的流程。

Skill 配置也是独立的：[web-dev-skills](https://github.com/timzaak/web-dev-skills)，可以套到其他 Rust + React 项目上。

选 Rust + React 是有考量的：编译器和类型系统是 AI 编码最好的质检员，OpenAPI-to-TypeScript 代码生成保持前后端 API 同步。

## 功能

设备走 MQTT 上报数据，Rust 后端接 WebHook 写 PostgreSQL，React 前端做管理界面。

- 设备生命周期管理：上下线跟踪、属性上报、事件历史
- 命令下发与 OTA 固件升级
- TLS 证书签发（内置 CA）

技术栈：Rust / Axum / SQLx / PostgreSQL / React 19 / TanStack

## 快速开始

前置条件：Docker、Rust 工具链、Node.js。完整步骤见[入门指南](docs/tutorials/getting-started.md)。

```shell
docker run postgres:18-alpine
docker run rmqtt/rmqtt:0.21.0
cd backend && cargo run
cd frontend && npm install && npm run dev
```

## 文档

| 文档 | 说明 |
|---|---|
| [入门指南](docs/tutorials/getting-started.md) | 完整安装配置 |
| [用 AI 二次开发](docs/tutorials/ai-development.md) | Skill 流水线与操作步骤 |
| [架构说明](docs/tutorials/architecture.md) | 系统设计与技术决策 |
| [物模型规范](docs/tutorials/thing-model-spec.md) | MQTT topic 格式与消息 schema |
| [API 参考](docs/tutorials/api-reference.md) | 管理端与 WebHook API 文档 |
| [部署指南](docs/tutorials/deployment.md) | 生产环境部署 |

## 许可证

[MIT](LICENSE-MIT) / [Apache 2.0](LICENSE-APACHE)
