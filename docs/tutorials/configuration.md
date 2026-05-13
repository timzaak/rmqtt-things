# 配置

RMQTT Things 用一个 TOML 文件管理所有配置。文件路径通过环境变量 `APP_CONFIG` 指定，不设这个变量就默认读当前目录下的 `config.toml`。

```rust
// main.rs
let config_path = env::var("APP_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
```

项目自带两个配置文件可以参考：

- `backend/config.example.toml`，本地开发用
- `docs/tutorials/config.production.toml`，生产部署用

## 启动方式

```bash
# 默认读 ./config.toml
cargo run

# 指定配置文件
APP_CONFIG=/path/to/config.toml cargo run
```

## database

PostgreSQL 连接配置。必须改。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `url` | string | `postgres://postgres:postgres@localhost:5432/postgres` | PostgreSQL 连接 URL，格式：`postgres://用户名:密码@主机:端口/数据库名` |

本地开发用默认值就行。生产环境必须改成实际的数据库地址，并且用强密码。

```toml
[database]
url = "postgres://rmqtt_user:your_password@db.example.com:5432/rmqtt_things"
```

## mqtt

RMQTT HTTP API 的连接配置。系统通过这个地址调用 RMQTT 的 HTTP 接口来发布消息、下发属性命令、做设备认证。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `url` | string | `http://127.0.0.1:6000/api/api/v1/mqtt/publish` | RMQTT HTTP API 地址 |

默认值看起来路径有重复（`api/api`），这是因为 RMQTT 的默认配置。如果你改过 RMQTT 的 API 路径前缀，这里也要对应改。`config.example.toml` 里用的是 `http://127.0.0.1:6060/api/v1`，以你实际部署为准。

### mqtt.publish.response

设备属性上报后，系统通过这里的配置把响应消息发回给设备。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `qos` | u8 | `2` | MQTT QoS 等级（0、1、2） |
| `retain` | bool | `false` | 是否保留消息 |
| `clientid` | string | `rmqtt_things` | 发布消息时用的 MQTT 客户端 ID |

### mqtt.property_command.publish

属性设置命令的下发配置。当你通过管理后台向设备发送属性控制命令时，走的是这个配置。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `qos` | u8 | `2` | MQTT QoS 等级 |
| `retain` | bool | `false` | 是否保留消息 |
| `clientid` | string | `rmqtt_things` | 发布消息时用的 MQTT 客户端 ID |
| `topic` | string | `thing/$clientid/thing/service/property/set` | 属性设置命令的 Topic 模板，支持变量 `${productId}`、`$clientid` |
| `retries` | u8 | `2` | 发送失败时的重试次数 |

`topic` 里可以用变量替换。`${productId}` 会替换成产品 ID，`$clientid` 会替换成设备客户端 ID。生产环境配置里通常写成 `${productId}/$clientid/thing/service/property/set`。

### mqtt.access.auth

设备认证配置。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `suffix` | string | `default_suffix` | 客户端 ID 后缀，用于区分不同部署环境的设备 |

如果你有多个 RMQTT Things 实例共用一个 RMQTT 集群，用不同的 `suffix` 来区分它们。生产环境一定要改，不要用默认值。

```toml
[mqtt.access.auth]
suffix = "my_deployment"
```

## otel

OpenTelemetry 配置，用于日志、链路追踪和指标上报。三个字段都是可选的，不配就不开启。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `log` | string | `None` | OTLP 日志导出地址 |
| `trace` | string | `None` | OTLP 链路追踪导出地址 |
| `metrics` | string | `None` | OTLP 指标导出地址 |

地址支持 gRPC 和 HTTP 两种格式：

```toml
[otel]
trace = "http://localhost:4317"            # gRPC
trace = "http://localhost:4318/v1/traces"  # HTTP
```

本地开发一般不用配。生产环境建议至少配上 `trace`，方便排查问题。

## api

HTTP API 相关配置。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `openapi_enabled` | bool | `true` | 是否启用 Swagger UI（`/swagger`）和 OpenAPI JSON（`/api-docs/openapi.json`） |
| `serve_web_path` | string | `None` | 前端静态文件目录路径，设了之后 API 服务同时托管前端页面 |
| `property_schema_validator` | bool | `false` | 是否开启物模型属性校验，开启后设备上报的属性值会按物模型定义做类型和范围检查 |

`openapi_enabled` 在生产环境建议关掉，避免暴露接口文档。`serve_web_path` 用于一体化部署（前后端打包在一起），留空的话 API 服务只提供接口。`property_schema_validator` 默认关闭是因为校验需要先定义好物模型，如果物模型还没配好就开校验，设备数据会被拒绝。

## cache

缓存配置。支持内存缓存和 Redis 两种模式。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `cache_type` | string (`"Memory"` / `"Redis"`) | `Memory` | 缓存类型 |
| `redis_url` | string | `None` | Redis 连接地址，`cache_type` 为 `Redis` 时必须配 |

本地开发用内存缓存就够了，不用装 Redis。生产环境用 Redis，因为内存缓存在多实例部署时不共享，重启也会丢失。

```toml
# 内存缓存（默认）
[cache]
cache_type = "Memory"

# Redis 缓存
[cache]
cache_type = "Redis"
redis_url = "redis://localhost:6379"
```

## s3

S3 兼容的对象存储配置。用于存储 OTA 固件包、设备证书等文件。整个段是可选的，不配就没有文件存储功能。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `endpoint` | string | 无 | S3 服务地址 |
| `region` | string | 无 | 区域 |
| `access_key` | string | 无 | 访问密钥 |
| `secret_key` | string | 无 | 密钥 |
| `bucket` | string | 无 | 存储桶名称 |
| `directories` | string 数组 | 无 | 允许的目录路径模板 |
| `expired_seconds` | u32 | 无 | 预签名 URL 过期时间（秒） |

本地开发可以用 MinIO：

```toml
[s3]
endpoint = "http://localhost:9000"
region = "us-east-1"
access_key = "minioadmin"
secret_key = "minioadmin"
bucket = "rmqtt-things"
directories = ["${productId}/${deviceId}/*", "public/*"]
expired_seconds = 600
```

生产环境用 AWS S3 或其他兼容服务，把密钥改成实际的：

```toml
[s3]
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
access_key = "AKIAIOSFODNN7EXAMPLE"
secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
bucket = "rmqtt-things"
directories = ["${productId}/${deviceId}/*", "public/*"]
expired_seconds = 600
```

`directories` 里可以用 `${productId}` 和 `${deviceId}` 变量，系统会根据实际的产品和设备做替换。`public/*` 是公共目录，不需要设备级权限就能访问。

## herald

Herald 统一认证服务配置。可选。配了之后管理端 API 会经过 Herald SSO 认证和权限校验；不配就无认证保护。详见[认证与权限](auth.md)。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `base_url` | string | 无 | Herald 服务地址 |
| `api_key` | string | 无 | 调用 Herald ext API 的密钥 |
| `realm_id` | string | 无 | rmqtt-things 所属的 realm |
| `client_id` | string | 无 | 客户端标识，如 `rmqtt-things-admin` |

```toml
[herald]
base_url = "http://127.0.0.1:3000"
api_key = "your-api-key"
realm_id = "default"
client_id = "rmqtt-things-admin"
```

生产环境把 `base_url` 改成 Herald 的实际地址（Docker 部署用容器名）。`api_key` 在 Herald 管理端生成。所有字段都是必填的，缺任何一个 `[herald]` 段就不会生效。

## ca

CA 证书配置。系统用这些参数生成和管理设备 TLS 证书。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ca_dir` | string | `conf` | CA 证书文件的存储目录 |
| `name` | string | `RMQTT Thing CA` | CA 的 Common Name |
| `valid_days` | i64 | `36503`（约 100 年） | 签发证书的有效天数 |
| `domain` | string | `*.fornetcode.com` | 证书域名，支持通配符 |

系统启动时检查 `ca_dir` 目录下有没有 CA 证书文件，没有就自动生成。`valid_days` 默认 100 年基本不用改。`domain` 改成你实际使用的域名。

```toml
[ca]
ca_dir = "conf"
name = "RMQTT Things Production CA"
valid_days = 3650
domain = "*.your-domain.com"
```

生产环境的 `valid_days` 建议设成 3650（10 年），比默认的 100 年更合理。证书过期了需要重新签发，有效期太长反而不安全。

## 一个完整的本地开发配置

```toml
[database]
url = "postgres://postgres:postgres@localhost:5432/rmqtt_things"

[mqtt]
url = "http://127.0.0.1:6060/api/v1"

[mqtt.publish.response]
qos = 2
retain = false
clientid = "rmqtt_things"

[mqtt.property_command.publish]
qos = 2
retain = false
clientid = "rmqtt_things"
topic = "thing/$clientid/thing/service/property/set"

[mqtt.access.auth]
suffix = "dev"

[api]
openapi_enabled = true

[cache]
cache_type = "Memory"

[ca]
ca_dir = "conf"
name = "RMQTT Thing CA"
valid_days = 36503
domain = "*.localhost"
```

这个配置假设 PostgreSQL 和 RMQTT 都跑在本机。如果是 Docker 部署，把地址换成容器名或服务名。
