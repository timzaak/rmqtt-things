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

## 功能

设备走 MQTT 上报数据，Rust 后端接 WebHook 写 PostgreSQL，React 前端做管理界面。

- 设备生命周期管理：上下线跟踪、自动注册、属性上报、事件历史
- 设备影子（期望值 + 增量下发）与属性历史图表
- 命令下发（属性设置、服务/动作调用）与 OTA 固件升级
- 告警规则与规则引擎（Webhook 通知）、事件校验模板
- 设备文件上传（S3 兼容对象存储）
- TLS 证书签发（外部 CA：`--generate-ca` 生成或自带 CA）
- Herald SSO 登录与基于角色的权限（可选；未配置时管理 API 无认证，需置于可信网络）

技术栈：Rust / Axum / SQLx / PostgreSQL / React 19 / TanStack / Redis（可选）/ S3

## 快速开始

前置条件：Docker、Rust 工具链、Node.js。完整步骤见[入门指南](docs/tutorials/getting-started.md)。

```shell
# PostgreSQL
docker run --rm --name postgres \
  -e POSTGRES_DB=rmqtt_things -e POSTGRES_USER=rmqtt_user -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 postgres:18-alpine

# RMQTT broker（项目根目录执行）
docker run --rm --name rmqtt -p 1883:1883 -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf rmqtt/rmqtt:0.23.1 -f conf/rmqtt.toml

# 后端
cd backend
cp config.example.toml config.toml
cargo run -- --generate-ca   # 首次运行前一次性生成 CA
cargo run

# 前端
cd ../frontend && npm install && npm run dev
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
