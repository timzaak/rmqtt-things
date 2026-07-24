# API 参考

Base URL: `http://localhost:8080/api`

错误响应统一格式：

```json
{"error": "错误描述"}
```

服务同时提供 Swagger UI：`http://localhost:8080/swagger`，可以直接在浏览器里试接口。

## WebHook 回调接口

这些接口是 RMQTT Broker 调的，不是给用户用的。RMQTT 通过 WebHook 插件把设备消息转发过来，后端处理后存数据库。

请求体都是 RMQTT 的标准 WebHook 格式，payload 字段是 base64 编码的 JSON。

### POST /api/device/connect

设备连接时 RMQTT 回调。记录设备上线状态。

请求体：

| 字段 | 类型 | 说明 |
|------|------|------|
| clientid | string | 设备 ID |
| username | string | 产品 ID（可为空） |
| ipaddress | string | 设备 IP |
| connected_at | int64 | 连接时间戳（毫秒） |
| keepalive | int16 | 心跳间隔（秒） |
| proto_ver | int8 | MQTT 协议版本 |
| product_id | string | 产品 ID（可为空，为空时取 username） |
| device_id | string | 设备 ID（可为空，为空时取 clientid） |

响应：`204 No Content`

### POST /api/device/disconnect

设备断开时 RMQTT 回调。记录设备离线状态和断开原因。

请求体与 connect 类似，多了 `reason`（断开原因）和 `disconnected_at`，少了 `keepalive`。

响应：`204 No Content`

### POST /api/thing/property/post

设备上报属性。payload 解码后格式：

```json
{
  "id": "request_id",
  "params": {"temperature": 25.3, "humidity": 60},
  "ack": 1
}
```

`params` 是键值对，key 是属性名，value 是属性值。如果 `ack` 为 1，后端会通过 RMQTT 发布确认响应到 `{topic}_reply`。

响应：`204 No Content`

### POST /api/thing/event/post

设备上报事件（非属性类）。payload 格式同上。

响应：`204 No Content`

### POST /api/thing/property/set_subscribe

设备订阅属性下发主题时触发。后端检查是否有待发送的属性命令，如果有就立即下发。

响应：`204 No Content`

### POST /api/thing/property/set_reply

设备回复属性下发命令的执行结果。payload 解码后：

```json
{
  "id": "request_id",
  "data": [1, 2, 3],
  "code": 200
}
```

`data` 是命令 ID 列表。`code` 为 200 标记成功，其他标记失败。

响应：`204 No Content`

### POST /api/thing/file/upload

设备请求文件上传凭证。payload 解码后：

```json
{
  "fileName": "log.txt",
  "directory": "productA/device1/",
  "useOriginName": false,
  "fileType": "text"
}
```

后端返回 S3 预签名 POST 上传凭证，通过 MQTT 响应发给设备。`directory` 必须在配置的允许目录列表内。

响应：`204 No Content`（凭证通过 MQTT 发给设备）

### POST /api/ota/version

设备上报当前固件版本。payload 解码后：

```json
{
  "id": "request_id",
  "params": [{"key": "main", "version": 102034}, {"key": "camera", "version": 201000}],
  "ack": 0
}
```

`version` 是整数编码的版本号（如 `1.2.34` = `102034`）。如果有匹配的 OTA 升级任务，后端会通过 MQTT 推送升级信息到 `{productId}/{deviceId}/ota/upgrade`。

响应：`204 No Content`

### POST /api/access/auth

设备认证。RMQTT 在设备连接时调用。

请求体：

| 字段 | 类型 | 说明 |
|------|------|------|
| client_id | string | 设备 ID |
| username | string | 产品 ID（可为空） |
| password | string | 认证密码，格式 `nonce.timestamp.hmac_sha1` |
| ipaddress | string | 设备 IP |

密码格式：`{6位随机字符}.{unix时间戳}.{hmac_sha1_hex}`。签名内容为 `{clientId}.{nonce}.{timestamp}.{suffix}`，suffix 在 config.toml 的 `[mqtt.access.auth]` 里配置。时间戳偏差超过 5 分钟会被拒绝。

响应：纯文本 `"allow"` 或 `"deny"`

### POST /api/access/acl

设备发布/订阅权限检查。RMQTT 在每次 pub/sub 时调用。

请求体：

| 字段 | 类型 | 说明 |
|------|------|------|
| client_id | string | 设备 ID |
| username | string | 产品 ID |
| topic | string | 要操作的 MQTT 主题 |
| access | string | `"1"` = 订阅，`"2"` = 发布 |

规则：设备只能操作以 `{username}/{clientId}/` 开头的主题，且只允许 `thing` 和 `ota` 类型的主题。

响应：纯文本 `"allow"` 或 `"deny"`

### POST /api/thing/factory-metadata/get

设备运行时拉取自身出厂元数据（设备级 + 子组件级合并视图）。RMQTT 把设备发布的 `{productId}/{deviceId}/thing/factory-metadata/get` 主题消息转发到此回调。详见[设备出厂元数据](device-guide.md#出厂元数据拉取)。

**认证**：仅内网 IP 白名单校验（设备 HMAC 认证由 RMQTT broker 完成）。

请求体为标准 RMQTT webhook 消息信封。后端把合并后的 `FactoryDeviceView`（结构与下方管理端 `GET /api/admin/factory/devices/{deviceSn}` 一致）通过 MQTT 响应发布到 `{topic}_reply`；设备无任何出厂元数据时 `data` 为 `null`。

响应：`204 No Content`（数据通过 `_reply` 主题异步返回设备）

## Admin 管理接口

管理后台用的 API。支持分页查询。

**认证要求**：配置了 Herald 后，所有 Admin 接口需要携带有效的 `X-Auth` Cookie。未认证返回 `401 Unauthorized`，无权限返回 `403 Forbidden`。没配 Herald 时无认证要求。详见[认证与权限](auth.md)。

分页参数（query string）在大多数 GET 接口中通用：

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int64 | 1 | 页码 |
| page_size | int64 | 10 | 每页条数 |

分页响应格式：

```json
{
  "data": [...],
  "pagination": {
    "page": 1,
    "page_size": 10,
    "total": 42
  }
}
```

部分接口不返回 `total`（性能考虑），只有 `page` 和 `page_size`。

### 属性

#### GET /api/admin/property

查询设备最新属性。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| device_id | string | 否 | 设备 ID |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

```bash
curl "http://localhost:8080/api/admin/property?product_id=demo&page=1&page_size=10"
```

#### GET /api/admin/property/history

查询属性变更历史。参数同上。

#### GET /api/admin/property/command

查询属性下发命令。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| device_id | string | 否 | 设备 ID |
| status | int16 | 否 | 0=pending, 1=sent, 2=success, 3=failed, 4=deleted |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

```bash
curl "http://localhost:8080/api/admin/property/command?product_id=demo&status=0"
```

#### POST /api/admin/property/command

创建属性下发命令。如果设备在线（已订阅属性主题），命令会立即下发；否则存数据库等设备上线后重试。

```bash
curl -X POST http://localhost:8080/api/admin/property/command \
  -H "Content-Type: application/json" \
  -d '{"product_id":"demo","device_id":"device1","command":{"brightness":80}}'
```

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| device_id | string | 是 | 设备 ID |
| command | object | 是 | 要下发的属性键值对 |

响应：`201 Created`

#### DELETE /api/admin/property/command

批量删除属性命令。

```bash
curl -X DELETE "http://localhost:8080/api/admin/property/command?ids=1&ids=2&ids=3"
```

响应：`200 OK`

### 事件

#### GET /api/admin/event

查询事件历史。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| device_id | string | 否 | 设备 ID |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

### 设备状态

#### GET /api/admin/device/status

查询设备当前在线/离线状态。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 否 | 产品 ID |
| device_id | string | 否 | 设备 ID |
| status | int16 | 否 | 0=offline, 1=online |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

```bash
curl "http://localhost:8080/api/admin/device/status?product_id=demo&status=1"
```

#### GET /api/admin/device/status/history

查询设备连接/断开历史。参数同属性查询（product_id 必填）。

### 校验模板

校验模板用来定义事件或属性的数据结构（JSON Schema），设备上报数据时会按模板校验。

#### GET /api/admin/valid/event

查询校验模板列表。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 否 | 产品 ID |
| event | string | 否 | 事件类型（`property` 表示属性模板） |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

#### POST /api/admin/valid/event

创建校验模板。

```bash
curl -X POST http://localhost:8080/api/admin/valid/event \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "demo",
    "event": "property",
    "description": "温度湿度属性模板",
    "schema": {
      "type": "object",
      "properties": {
        "temperature": {"type": "number"},
        "humidity": {"type": "number"}
      },
      "required": ["temperature"]
    }
  }'
```

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| event | string | 是 | 事件类型，`property` 表示属性模板 |
| description | string | 否 | 描述 |
| schema | object | 是 | JSON Schema |

响应：`201 Created`

#### GET /api/admin/valid/event/{id}

获取单个校验模板详情。

#### PATCH /api/admin/valid/event/{id}

更新校验模板。不能修改已激活（status=1）模板的 schema。

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| schema | object | 否 | 新的 JSON Schema |
| description | string | 否 | 新的描述 |

#### PATCH /api/admin/valid/event/{id}/status

更新模板状态。状态值：0=Draft, 1=Active, 2=Inactive。

```bash
curl -X PATCH http://localhost:8080/api/admin/valid/event/1/status \
  -H "Content-Type: application/json" \
  -d '{"status": 1}'
```

激活属性模板（event=property）后，缓存会自动刷新。设备下次上报属性时会按新 schema 校验。

### 证书

#### GET /api/admin/ca/cert

查询已签发的证书列表。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 否 | 产品 ID |
| device_id | string | 否 | 设备 ID |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

#### POST /api/admin/ca/cert

签发设备证书。用自签 CA 生成客户端证书，CN 设为 `{productId}/{deviceId}`。

```bash
curl -X POST http://localhost:8080/api/admin/ca/cert \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "demo",
    "device_id": "device1",
    "force": false,
    "start_at": "2025-01-01T00:00:00Z",
    "end_at": "2035-01-01T00:00:00Z"
  }'
```

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| device_id | string | 是 | 设备 ID |
| force | bool | 是 | 是否强制重新签发（覆盖未过期证书） |
| start_at | string | 是 | 证书生效时间（RFC 3339） |
| end_at | string | 是 | 证书过期时间（RFC 3339） |

响应：

```json
{
  "cert_pem": "-----BEGIN CERTIFICATE-----\n...",
  "key_pem": "-----BEGIN RSA PRIVATE KEY-----\n..."
}
```

#### PATCH /api/admin/ca/cert/status

更新证书状态（用于吊销）。

```bash
curl -X PATCH http://localhost:8080/api/admin/ca/cert/status \
  -H "Content-Type: application/json" \
  -d '{"product_id":"demo","device_id":"device1","status":2}'
```

状态值：0=Normal, 2=Revoked

### 产品

#### GET /api/admin/product

查询产品列表。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| search | string | 否 | 搜索关键字（模糊匹配名称或型号） |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

#### POST /api/admin/product

创建产品。

```bash
curl -X POST http://localhost:8080/api/admin/product \
  -H "Content-Type: application/json" \
  -d '{"name":"智能温湿度计","model_no":"TH-200","description":"带屏幕的温湿度传感器"}'
```

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | 是 | 产品名称 |
| model_no | string | 是 | 产品型号（唯一） |
| description | string | 否 | 描述 |

#### GET /api/admin/product/{id}

获取产品详情。

#### PATCH /api/admin/product/{id}

更新产品信息。

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | 是 | 产品名称 |
| description | string | 是 | 描述 |

### OTA 版本

#### GET /api/admin/ota/version

查询 OTA 版本列表。

参数：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 否 | 产品 ID |
| page | int64 | 否 | 页码 |
| page_size | int64 | 否 | 每页条数 |

#### POST /api/admin/ota/version

创建 OTA 版本。

```bash
curl -X POST http://localhost:8080/api/admin/ota/version \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "demo",
    "key": "main",
    "version": "102034",
    "min_version": "100000",
    "file_key": "demo/ota/v1.2.34.bin",
    "bin_length": 524288,
    "bin_md5": "abc123def456"
  }'
```

请求体：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| product_id | string | 是 | 产品 ID |
| key | string | 是 | 固件分区 key（多 MCU 场景区分） |
| version | string | 是 | 目标版本号（如 `"102034"`） |
| max_version | string | 否 | 最高适用版本 |
| min_version | string | 是 | 最低适用版本 |
| file_key | string | 是 | S3 上的固件文件路径 |
| log | object | 否 | 更新日志 |
| device_ids | string[] | 否 | 定向升级的设备列表 |
| bin_length | int64 | 是 | 固件文件大小 |
| bin_md5 | string | 是 | 固件 MD5 |

响应：`201 Created`

#### GET /api/admin/ota/version/{id}

获取单个 OTA 版本详情。

#### PUT /api/admin/ota/version/{id}

更新 OTA 版本信息。

#### DELETE /api/admin/ota/version/{id}

删除 OTA 版本。

### 文件上传

#### POST /api/admin/file/upload

管理后台获取 S3 预签名上传凭证。

```bash
curl -X POST http://localhost:8080/api/admin/file/upload \
  -H "Content-Type: application/json" \
  -d '{"fileName":"firmware.bin","directory":"public/","useOriginName":true,"fileType":"binary"}'
```

响应：

```json
{
  "url": "http://localhost:9000/rmqtt-things",
  "fields": {"key": "public/firmware.bin", "policy": "...", "signature": "..."}
}
```

### 出厂元数据查询

管理员查询产线上报的设备出厂元数据。写入入口见下方[产线写入接口](#产线写入接口)。详见 PRD [`docs/prd/core/support-multiple-device.md`](../prd/core/support-multiple-device.md)。

#### GET /api/admin/factory/devices/{deviceSn}

查询某设备的出厂元数据合并视图（设备级 + 子组件级 left join 当前存在部分）。设备无任何出厂元数据时返回 `404`。

```bash
curl -b "X-Auth=<token>" http://localhost:8080/api/admin/factory/devices/my-device-sn
```

```json
{
  "deviceSn": "my-device-sn",
  "deviceMetadata": {
    "metadata": {"serial": "SN-A", "batch": "2026Q3"},
    "fileAttachments": [{"fileKey": "...", "fileName": "report.pdf"}],
    "updatedAt": "2026-07-24T00:00:00Z"
  },
  "components": [
    {
      "componentSn": "cam-001",
      "componentType": "camera",
      "metadata": {"calibration": 2},
      "fileAttachments": [],
      "updatedAt": "2026-07-24T00:00:00Z"
    }
  ]
}
```

`deviceMetadata` 为整机级元数据（`null` 表示产线尚未上报设备级元数据），`components` 为子组件清单。二者独立落地、异步组装，部分未到达时返回当前存在部分，不报错。

#### GET /api/admin/factory/sn/{sn}/changes

查询某 SN 的出厂元数据变更日志（时间倒序，分页）。`sn` 既可以是设备 SN（设备级覆盖日志）也可以是子组件 SN（子组件级覆盖日志）。支持分页参数 `page` / `page_size`。

```bash
curl -b "X-Auth=<token>" "http://localhost:8080/api/admin/factory/sn/cam-001/changes?page=1&page_size=20"
```

```json
{
  "data": [
    {
      "id": 12,
      "sn": "cam-001",
      "before": {"metadata": {"calibration": 1}, "file_attachments": [], "updated_at": "..."},
      "after": {"metadata": {"calibration": 2}, "file_attachments": [], "updated_at": "..."},
      "actor": "factory",
      "created_at": "2026-07-24T00:00:00Z"
    }
  ],
  "pagination": {"page": 1, "page_size": 20, "total": 1}
}
```

`before` 为覆盖前快照（首次上报那行 `before` 为 `null`，因为 Created 不写日志），`after` 为覆盖后快照。子组件级快照含 `component_type` 字段；**设备级快照无 `component_type`**（整机没有组件类型概念），断言走 `after.metadata.xxx` 路径。

### 健康检查

#### GET /api/health

```bash
curl http://localhost:8080/api/health
```

```json
{"status":"health","timestamp":"2025-01-01T00:00:00Z"}
```

## 产线写入接口

产线（工厂）系统上报设备出厂元数据的独立 API。与 Admin 认证（Herald cookie）、设备认证（HMAC 证书）完全隔离：**必须携带 `Authorization: Bearer <key>`**，key 须出现在后端 `[factory] api_keys` 配置项中（空配置拒绝所有请求，返回 `401`）。读取见上方[出厂元数据查询](#出厂元数据查询)。

```bash
curl -X PUT http://localhost:8080/api/factory/components/cam-001 \
  -H "Authorization: Bearer ${FACTORY_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"metadata": {"calibration": 2}}'
```

### PUT /api/factory/components/{componentSn}

upsert 子组件出厂元数据（结构化字段 + 文件附件）。同 SN 重复上报幂等覆盖，覆盖时写一条变更日志。

请求体（全部可选，缺省值：`componentType`=`"camera"`、`metadata`=`{}`、`fileAttachments`=`[]`）：

| 字段 | 类型 | 说明 |
|------|------|------|
| componentType | string? | 组件类型提示，缺省 `"camera"` |
| metadata | object? | 结构化元数据（标定值等） |
| fileAttachments | array? | 文件附件，`fileKey` 须先经 `POST /api/factory/file/upload` 取得 |

响应：`204 No Content`

### PUT /api/factory/devices/{deviceSn}

upsert 设备级（整机）出厂元数据。与子组件级对称，但**无 `componentType` 字段**（整机没有组件类型概念）。覆盖时写一条变更日志（`sn = deviceSn`，`after` 快照无 `component_type`）。

请求体（全部可选，缺省 `metadata`=`{}`、`fileAttachments`=`[]`）：

| 字段 | 类型 | 说明 |
|------|------|------|
| metadata | object? | 整机级结构化元数据（序列号标签、批次等） |
| fileAttachments | array? | 文件附件（出厂检验报告等） |

响应：`204 No Content`

### PUT /api/factory/devices/{deviceSn}/components

全量替换设备的子组件关联（full-replace 语义：未出现在列表里的关联会被删除）。与子组件元数据异步到达、乱序不阻塞。该端点**不写变更日志**（变更日志范围限定在元数据覆盖）。

请求体：

```json
{
  "components": [
    {"componentSn": "cam-001", "componentType": "camera"},
    {"componentSn": "sensor-002"}
  ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| components | array | 子组件列表（full-replace），`componentSn` 必填，`componentType` 可选提示 |

响应：`204 No Content`

### POST /api/factory/file/upload

产线侧文件上传（S3 预签名 POST），取得的 `fileKey` 用于上述 `fileAttachments`。与管理端文件上传能力一致，但走产线 Bearer 认证；`directory` 必须在配置的允许目录列表内，且 factory 目录规则只能用字面前缀（`${productId}`/`${deviceId}` 模板占位符在产线路径下会被置空）。
