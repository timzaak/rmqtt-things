# API Reference

Base URL: `http://localhost:8080/api`

All error responses follow a uniform format:

```json
{"error": "error description"}
```

Swagger UI is also available at `http://localhost:8080/swagger` for interactive API testing in the browser.

## WebHook Callback Endpoints

These endpoints are called by the RMQTT Broker, not by end users. RMQTT forwards device messages via its WebHook plugin; the backend processes them and persists the results to the database.

Request bodies use the standard RMQTT WebHook format. The `payload` field is a base64-encoded JSON string.

### POST /api/device/connect

RMQTT callback when a device connects. Records the device's online status.

Request body:

| Field | Type | Description |
|-------|------|-------------|
| clientid | string | Device ID |
| username | string | Product ID (may be empty) |
| ipaddress | string | Device IP address |
| connected_at | int64 | Connection timestamp (milliseconds) |
| keepalive | int16 | Heartbeat interval (seconds) |
| proto_ver | int8 | MQTT protocol version |
| product_id | string | Product ID (may be empty; falls back to `username`) |
| device_id | string | Device ID (may be empty; falls back to `clientid`) |

Response: `204 No Content`

### POST /api/device/disconnect

RMQTT callback when a device disconnects. Records the device's offline status and disconnect reason.

The request body is similar to `connect`, but adds `reason` (disconnect reason) and `disconnected_at`, and omits `keepalive`.

Response: `204 No Content`

### POST /api/thing/property/post

Device reports properties. Decoded payload format:

```json
{
  "id": "request_id",
  "params": {"temperature": 25.3, "humidity": 60},
  "ack": 1
}
```

`params` is a key-value map where keys are property names and values are property values. If `ack` is 1, the backend publishes an acknowledgement to `{topic}_reply` via RMQTT.

Response: `204 No Content`

### POST /api/thing/event/post

Device reports an event (non-property). The payload format is the same as above.

Response: `204 No Content`

### POST /api/thing/property/set_subscribe

Triggered when a device subscribes to a property-set topic. The backend checks for pending property commands and, if any exist, delivers them immediately.

Response: `204 No Content`

### POST /api/thing/property/set_reply

Device replies with the execution result of a property-set command. Decoded payload:

```json
{
  "id": "request_id",
  "data": [1, 2, 3],
  "code": 200
}
```

`data` is a list of command IDs. A `code` of 200 indicates success; other values indicate failure.

Response: `204 No Content`

### POST /api/thing/file/upload

Device requests file upload credentials. Decoded payload:

```json
{
  "fileName": "log.txt",
  "directory": "productA/device1/",
  "useOriginName": false,
  "fileType": "text"
}
```

The backend returns an S3 presigned POST upload credential and sends it to the device via MQTT response. The `directory` must be within the configured allowed directory list.

Response: `204 No Content` (credentials are sent to the device via MQTT)

### POST /api/ota/version

Device reports its current firmware version. Decoded payload:

```json
{
  "id": "request_id",
  "params": [{"key": "main", "version": 102034}, {"key": "camera", "version": 201000}],
  "ack": 0
}
```

`version` is an integer-encoded version number (e.g. `1.2.34` = `102034`). If a matching OTA upgrade task exists, the backend pushes upgrade information to `{productId}/{deviceId}/ota/upgrade` via MQTT.

Response: `204 No Content`

### POST /api/access/auth

Device authentication. Called by RMQTT when a device connects.

Request body:

| Field | Type | Description |
|-------|------|-------------|
| client_id | string | Device ID |
| username | string | Product ID (may be empty) |
| password | string | Authentication password in the format `nonce.timestamp.hmac_sha1` |
| ipaddress | string | Device IP address |

Password format: `{6-char random nonce}.{unix timestamp}.{hmac_sha1_hex}`. The signed content is `{clientId}.{nonce}.{timestamp}.{suffix}`, where `suffix` is configured in `config.toml` under `[mqtt.access.auth]`. Timestamps deviating more than 5 minutes are rejected.

Response: plain text `"allow"` or `"deny"`

### POST /api/access/acl

Device publish/subscribe permission check. Called by RMQTT on every pub/sub operation.

Request body:

| Field | Type | Description |
|-------|------|-------------|
| client_id | string | Device ID |
| username | string | Product ID |
| topic | string | MQTT topic to operate on |
| access | string | `"1"` = subscribe, `"2"` = publish |

Rule: devices may only operate on topics prefixed with `{username}/{clientId}/`, and only `thing` and `ota` topic types are allowed.

Response: plain text `"allow"` or `"deny"`

## Admin Management Endpoints

APIs for the management backend. Supports paginated queries.

**Authentication**: When Herald is configured, all Admin endpoints require a valid `X-Auth` cookie. Unauthenticated requests return `401 Unauthorized`, unauthorized requests return `403 Forbidden`. Without Herald, no authentication is required. See [Authentication & Authorization](auth-en.md).

Common pagination parameters (query string) for most GET endpoints:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| page | int64 | 1 | Page number |
| page_size | int64 | 10 | Items per page |

Paginated response format:

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

Some endpoints omit `total` (for performance reasons) and return only `page` and `page_size`.

### Properties

#### GET /api/admin/property

Query the latest device properties.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| device_id | string | No | Device ID |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

```bash
curl "http://localhost:8080/api/admin/property?product_id=demo&page=1&page_size=10"
```

#### GET /api/admin/property/history

Query property change history. Parameters are the same as above.

#### GET /api/admin/property/command

Query property-set commands.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| device_id | string | No | Device ID |
| status | int16 | No | 0=pending, 1=sent, 2=success, 3=failed, 4=deleted |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

```bash
curl "http://localhost:8080/api/admin/property/command?product_id=demo&status=0"
```

#### POST /api/admin/property/command

Create a property-set command. If the device is online (has subscribed to the property topic), the command is delivered immediately; otherwise it is stored in the database and retried when the device comes online.

```bash
curl -X POST http://localhost:8080/api/admin/property/command \
  -H "Content-Type: application/json" \
  -d '{"product_id":"demo","device_id":"device1","command":{"brightness":80}}'
```

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| device_id | string | Yes | Device ID |
| command | object | Yes | Property key-value pairs to deliver |

Response: `201 Created`

#### DELETE /api/admin/property/command

Batch delete property commands.

```bash
curl -X DELETE "http://localhost:8080/api/admin/property/command?ids=1&ids=2&ids=3"
```

Response: `200 OK`

### Events

#### GET /api/admin/event

Query event history.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| device_id | string | No | Device ID |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

### Device Status

#### GET /api/admin/device/status

Query current device online/offline status.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | No | Product ID |
| device_id | string | No | Device ID |
| status | int16 | No | 0=offline, 1=online |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

```bash
curl "http://localhost:8080/api/admin/device/status?product_id=demo&status=1"
```

#### GET /api/admin/device/status/history

Query device connect/disconnect history. Parameters are the same as property queries (`product_id` is required).

### Validation Templates

Validation templates define the data structure (JSON Schema) for events or properties. Device-reported data is validated against matching templates.

#### GET /api/admin/valid/event

List validation templates.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | No | Product ID |
| event | string | No | Event type (`property` for property templates) |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

#### POST /api/admin/valid/event

Create a validation template.

```bash
curl -X POST http://localhost:8080/api/admin/valid/event \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "demo",
    "event": "property",
    "description": "Temperature and humidity property template",
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

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| event | string | Yes | Event type; `property` for property templates |
| description | string | No | Description |
| schema | object | Yes | JSON Schema |

Response: `201 Created`

#### GET /api/admin/valid/event/{id}

Get details of a single validation template.

#### PATCH /api/admin/valid/event/{id}

Update a validation template. The `schema` of an active (status=1) template cannot be modified.

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| schema | object | No | Updated JSON Schema |
| description | string | No | Updated description |

#### PATCH /api/admin/valid/event/{id}/status

Update template status. Status values: 0=Draft, 1=Active, 2=Inactive.

```bash
curl -X PATCH http://localhost:8080/api/admin/valid/event/1/status \
  -H "Content-Type: application/json" \
  -d '{"status": 1}'
```

When a property template (event=property) is activated, the cache is automatically refreshed. The next property report from a device will be validated against the new schema.

### Certificates

#### GET /api/admin/ca/cert

List issued certificates.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | No | Product ID |
| device_id | string | No | Device ID |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

#### POST /api/admin/ca/cert

Issue a device certificate. Generates a client certificate using the self-signed CA, with the CN set to `{productId}/{deviceId}`.

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

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| device_id | string | Yes | Device ID |
| force | bool | Yes | Whether to force re-issuance (overwrites unexpired certificates) |
| start_at | string | Yes | Certificate effective time (RFC 3339) |
| end_at | string | Yes | Certificate expiration time (RFC 3339) |

Response:

```json
{
  "cert_pem": "-----BEGIN CERTIFICATE-----\n...",
  "key_pem": "-----BEGIN RSA PRIVATE KEY-----\n..."
}
```

#### PATCH /api/admin/ca/cert/status

Update certificate status (used for revocation).

```bash
curl -X PATCH http://localhost:8080/api/admin/ca/cert/status \
  -H "Content-Type: application/json" \
  -d '{"product_id":"demo","device_id":"device1","status":2}'
```

Status values: 0=Normal, 2=Revoked

### Products

#### GET /api/admin/product

List products.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| search | string | No | Search keyword (fuzzy match on name or model number) |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

#### POST /api/admin/product

Create a product.

```bash
curl -X POST http://localhost:8080/api/admin/product \
  -H "Content-Type: application/json" \
  -d '{"name":"Smart Thermometer","model_no":"TH-200","description":"Temperature and humidity sensor with display"}'
```

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | Yes | Product name |
| model_no | string | Yes | Product model number (unique) |
| description | string | No | Description |

#### GET /api/admin/product/{id}

Get product details.

#### PATCH /api/admin/product/{id}

Update product information.

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | Yes | Product name |
| description | string | Yes | Description |

### OTA Versions

#### GET /api/admin/ota/version

List OTA versions.

Parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| product_id | string | No | Product ID |
| page | int64 | No | Page number |
| page_size | int64 | No | Items per page |

#### POST /api/admin/ota/version

Create an OTA version.

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

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| product_id | string | Yes | Product ID |
| key | string | Yes | Firmware partition key (distinguishes MCUs in multi-MCU setups) |
| version | string | Yes | Target version number (e.g. `"102034"`) |
| max_version | string | No | Maximum applicable version |
| min_version | string | Yes | Minimum applicable version |
| file_key | string | Yes | Firmware file path on S3 |
| log | object | No | Changelog |
| device_ids | string[] | No | List of device IDs for targeted upgrades |
| bin_length | int64 | Yes | Firmware file size |
| bin_md5 | string | Yes | Firmware MD5 checksum |

Response: `201 Created`

#### GET /api/admin/ota/version/{id}

Get details of a single OTA version.

#### PUT /api/admin/ota/version/{id}

Update OTA version information.

#### DELETE /api/admin/ota/version/{id}

Delete an OTA version.

### File Upload

#### POST /api/admin/file/upload

Get an S3 presigned upload credential from the management backend.

```bash
curl -X POST http://localhost:8080/api/admin/file/upload \
  -H "Content-Type: application/json" \
  -d '{"fileName":"firmware.bin","directory":"public/","useOriginName":true,"fileType":"binary"}'
```

Response:

```json
{
  "url": "http://localhost:9000/rmqtt-things",
  "fields": {"key": "public/firmware.bin", "policy": "...", "signature": "..."}
}
```

### Health Check

#### GET /api/health

```bash
curl http://localhost:8080/api/health
```

```json
{"status":"health","timestamp":"2025-01-01T00:00:00Z"}
```
