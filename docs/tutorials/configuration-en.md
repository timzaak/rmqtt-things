# Configuration

RMQTT Things uses a single TOML file for all configuration. The file path is specified via the `APP_CONFIG` environment variable; if not set, it defaults to `config.toml` in the current directory.

```rust
// main.rs
let config_path = env::var("APP_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
```

The project includes two reference configuration files:

- `backend/config.example.toml` — for local development
- `docs/tutorials/config.production.toml` — for production deployment

## Starting the Service

```bash
# Default: reads ./config.toml
cargo run

# Specify a config file
APP_CONFIG=/path/to/config.toml cargo run
```

## database

PostgreSQL connection settings. Must be changed for your environment.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | string | `postgres://postgres:postgres@localhost:5432/postgres` | PostgreSQL connection URL, format: `postgres://user:password@host:port/database` |

The default value is fine for local development. For production, always use the actual database address and a strong password.

```toml
[database]
url = "postgres://rmqtt_user:your_password@db.example.com:5432/rmqtt_things"
```

## mqtt

RMQTT HTTP API connection settings. The system calls RMQTT's HTTP interface at this address to publish messages, send property commands, and perform device authentication.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | string | `http://127.0.0.1:6000/api/api/v1/mqtt/publish` | RMQTT HTTP API address |

The default URL appears to have a duplicated path segment (`api/api`), which matches RMQTT's default configuration. If you have customized RMQTT's API path prefix, update this accordingly. `config.example.toml` uses `http://127.0.0.1:6060/api/v1` — always match your actual deployment.

### mqtt.publish.response

After a device reports its properties, the system uses these settings to send a response message back to the device.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `qos` | u8 | `2` | MQTT QoS level (0, 1, or 2) |
| `retain` | bool | `false` | Whether to retain the message |
| `clientid` | string | `rmqtt_things` | MQTT client ID used when publishing |

### mqtt.property_command.publish

Configuration for sending property-set commands. When you send an attribute control command to a device through the admin backend, these settings apply.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `qos` | u8 | `2` | MQTT QoS level |
| `retain` | bool | `false` | Whether to retain the message |
| `clientid` | string | `rmqtt_things` | MQTT client ID used when publishing |
| `topic` | string | `thing/$clientid/thing/service/property/set` | Topic template for property-set commands; supports variables `${productId}` and `$clientid` |
| `retries` | u8 | `2` | Number of retries on send failure |

The `topic` field supports variable substitution. `${productId}` is replaced with the product ID, and `$clientid` is replaced with the device client ID. Production configurations typically use `${productId}/$clientid/thing/service/property/set`.

### mqtt.access.auth

Device authentication settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `suffix` | string | `default_suffix` | Client ID suffix, used to distinguish devices across different deployment environments |
| `timestamp_tolerance_secs` | integer | `300` | How many seconds a device's HMAC timestamp may differ from server time. A password is rejected outside this window. Raise it for testing (e.g. `604800` = 7 days, so one password lasts longer); keep the default for production, where a tight window limits replay attacks. |

If you have multiple RMQTT Things instances sharing a single RMQTT cluster, use a different `suffix` for each. Always change this in production — do not use the default value.

```toml
[mqtt.access.auth]
suffix = "my_deployment"
timestamp_tolerance_secs = 300
```

## otel

OpenTelemetry configuration for logging, distributed tracing, and metrics export. All three fields are optional; any unset field is simply disabled.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `log` | string | `None` | OTLP log export endpoint |
| `trace` | string | `None` | OTLP trace export endpoint |
| `metrics` | string | `None` | OTLP metrics export endpoint |

Endpoints support both gRPC and HTTP formats:

```toml
[otel]
trace = "http://localhost:4317"            # gRPC
trace = "http://localhost:4318/v1/traces"  # HTTP
```

Local development typically does not need these. For production, it is recommended to at least configure `trace` to aid troubleshooting.

## api

HTTP API settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `openapi_enabled` | bool | `true` | Whether to enable Swagger UI (`/swagger`) and OpenAPI JSON (`/api-docs/openapi.json`) |
| `serve_web_path` | string | `None` | Directory path for frontend static files; when set, the API server also serves the frontend |
| `property_schema_validator` | bool | `false` | Whether to enable thing-model property validation; when enabled, reported property values are checked against the thing-model definition for type and range |

`openapi_enabled` should be turned off in production to avoid exposing API documentation. `serve_web_path` is used for integrated deployment (frontend and backend served together); if left unset, the API server only serves API endpoints. `property_schema_validator` defaults to off because validation requires a defined thing model — if you enable it before the thing model is configured, device data will be rejected.

## cache

Cache configuration. Supports both in-memory and Redis modes.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cache_type` | string (`"Memory"` / `"Redis"`) | `Memory` | Cache type |
| `redis_url` | string | `None` | Redis connection URL; required when `cache_type` is `Redis` |

In-memory caching is sufficient for local development — no Redis needed. Use Redis in production, since in-memory cache is not shared across multiple instances and is lost on restart.

```toml
# In-memory cache (default)
[cache]
cache_type = "Memory"

# Redis cache
[cache]
cache_type = "Redis"
redis_url = "redis://localhost:6379"
```

## s3

S3-compatible object storage configuration. Used to store OTA firmware packages, device certificates, and other files. The entire section is optional; if not configured, file storage features are unavailable.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `endpoint` | string | none | S3 service endpoint |
| `region` | string | none | Region |
| `access_key` | string | none | Access key |
| `secret_key` | string | none | Secret key |
| `bucket` | string | none | Bucket name |
| `directories` | string array | none | Allowed directory path templates |
| `expired_seconds` | u32 | none | Pre-signed URL expiration time (seconds) |

For local development, you can use MinIO:

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

For production, use AWS S3 or another compatible service with actual credentials:

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

`directories` supports `${productId}` and `${deviceId}` variables, which the system replaces with the actual product and device IDs. `public/*` is a public directory accessible without device-level permissions.

## herald

Herald SSO configuration. Optional. When configured, Admin API endpoints require Herald SSO authentication and permission checks. Without it, admin endpoints have no authentication. See [Authentication & Authorization](auth-en.md).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base_url` | string | none | Herald service URL |
| `api_key` | string | none | API key for Herald ext API |
| `realm_id` | string | none | Realm that rmqtt-things belongs to |
| `client_id` | string | none | Client identifier, e.g. `admin-web-console` |

```toml
[herald]
base_url = "http://127.0.0.1:3000"
api_key = "your-api-key"
realm_id = "rmqtt"
client_id = "admin-web-console"
```

In production, change `base_url` to the actual Herald address (use container name for Docker). Generate `api_key` from the Herald admin panel. All fields are required — if any is missing, the `[herald]` section won't take effect.

## ca

CA certificate configuration. The system uses these parameters to generate and manage device TLS certificates.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ca_dir` | string | `conf` | Directory where CA certificate files are stored |
| `name` | string | `RMQTT Thing CA` | CA Common Name |
| `valid_days` | i64 | `36503` (~100 years) | Validity period in days for issued certificates |
| `domain` | string | `*.fornetcode.com` | Certificate domain; supports wildcards |

On startup, the system checks `ca_dir` for existing CA certificate files and auto-generates them if none are found. The default `valid_days` of 100 years rarely needs changing. Update `domain` to your actual domain.

```toml
[ca]
ca_dir = "conf"
name = "RMQTT Things Production CA"
valid_days = 3650
domain = "*.your-domain.com"
```

For production, `valid_days` is recommended to be 3650 (10 years), which is more reasonable than the default 100 years. Expired certificates must be re-issued; excessively long validity periods are less secure.

## A Complete Local Development Configuration

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

This configuration assumes PostgreSQL and RMQTT are running locally. For Docker deployments, replace the addresses with container names or service names.
