# Device-Side Development Reference

Intended for firmware and embedded developers. If your device needs to connect to the RMQTT Things platform, this document contains everything you need to know.

For a complete reference implementation, see [`demo/e2e/helpers/mqtt-device.ts`](../../demo/e2e/helpers/mqtt-device.ts).

## MQTT Connection Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Broker address | `mqtt://host:1883` | Plain TCP connection |
| Broker address (TLS) | `mqtts://host:8883` | Requires client certificate |
| `client_id` | Device ID | Globally unique |
| `username` | Product `model_no` | Specified when creating the product |
| `password` | HMAC-SHA1 generated value | See below |

## HMAC-SHA1 Password Generation

Password format: `nonce.timestamp.hex(hmac_sha1(suffix, deviceId.nonce.timestamp.suffix))`

- `nonce`: 6-character random hex string
- `timestamp`: Current Unix timestamp (seconds); must not differ from server time by more than 5 minutes
- `suffix`: Shared secret, configured in `[mqtt.access.auth]` `suffix` field on the backend; defaults to `suffix_go`

### JavaScript / TypeScript

```javascript
import { createHmac, randomBytes } from 'node:crypto'

function generatePassword(deviceId, suffix = 'suffix_go') {
  const nonce = randomBytes(3).toString('hex') // 6-char hex
  const timestamp = Math.floor(Date.now() / 1000)
  const toSign = `${deviceId}.${nonce}.${timestamp}.${suffix}`
  const hash = createHmac('sha1', suffix).update(toSign).digest('hex')
  return `${nonce}.${timestamp}.${hash}`
}
```

### Python

```python
import hmac, hashlib, time, secrets

def generate_password(device_id: str, suffix: str = "suffix_go") -> str:
    nonce = secrets.token_hex(3)  # 6-char hex
    timestamp = str(int(time.time()))
    to_sign = f"{device_id}.{nonce}.{timestamp}.{suffix}"
    h = hmac.new(suffix.encode(), to_sign.encode(), hashlib.sha1).hexdigest()
    return f"{nonce}.{timestamp}.{h}"
```

### C (Embedded Reference)

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// Requires an HMAC-SHA1 implementation, e.g. mbedTLS or wolfSSL
// Below is a pseudocode skeleton

void generate_password(const char *device_id, const char *suffix,
                       char *output, size_t output_len) {
    char nonce[7];
    // Generate 6-char random hex
    snprintf(nonce, sizeof(nonce), "%06x", rand() & 0xFFFFFF);

    time_t ts = time(NULL);
    char to_sign[128];
    snprintf(to_sign, sizeof(to_sign), "%s.%s.%ld.%s",
             device_id, nonce, (long)ts, suffix);

    unsigned char hmac_result[20];
    // hmac_sha1(suffix, to_sign, hmac_result) — use your HMAC library

    char hex_hash[41];
    // bytes_to_hex(hmac_result, 20, hex_hash)

    snprintf(output, output_len, "%s.%ld.%s", nonce, (long)ts, hex_hash);
}
```

## Topic Quick Reference

`{pid}` = productId (i.e. model_no), `{did}` = deviceId.

| Direction | Topic Pattern | Purpose |
|-----------|--------------|---------|
| Device → Cloud | `{pid}/{did}/thing/event/property/post` | Report properties |
| Device → Cloud | `{pid}/{did}/thing/event/{type}/post` | Report events |
| Cloud → Device | `{pid}/{did}/thing/service/property/set` | Set properties (command) |
| Device → Cloud | `{pid}/{did}/thing/service/property/set_reply` | Reply to command |
| Device → Cloud | `{pid}/{did}/thing/file/upload` | Request file upload |
| Cloud → Device | `{pid}/{did}/thing/file/upload_reply` | Return upload credentials |
| Device → Cloud | `{pid}/{did}/ota/version` | Report firmware version |
| Cloud → Device | `{pid}/{did}/ota/upgrade` | Push OTA upgrade |

## Authentication and Authorization

### Authentication

When a device connects, RMQTT's auth-http plugin calls the backend `/api/access/auth` endpoint to verify the HMAC password. The verification flow:

1. Split the password; check that the nonce is 6 characters and the timestamp format is valid
2. If the timestamp differs from the current time by more than 300 seconds (5 minutes), reject
3. Use the configured `suffix` as the key to compute HMAC-SHA1 over `{clientId}.{nonce}.{timestamp}.{suffix}`
4. Compare the hash; return `"allow"` on match, otherwise `"deny"`

If the backend is unreachable, the connection is rejected outright (`deny_if_error = true`). It is preferable to deny legitimate devices than to allow unauthenticated ones.

### ACL

On every PUBLISH or SUBSCRIBE, RMQTT's ACL plugin calls the backend `/api/access/acl` endpoint to check permissions. Rules:

1. The second segment of the topic (deviceId) must equal the clientId — devices can only operate within their own topic space
2. The first segment of the topic (productId) must equal the username
3. Only `thing/event/*`, `thing/service/*`, and `ota/*` topic categories are allowed
4. Everything else is denied

### Auto-Subscription

After a device connects, the RMQTT auto-subscription plugin automatically subscribes to the following topics. The device does not need to send SUBSCRIBE manually:

| Topic | Purpose |
|-------|---------|
| `+/{deviceId}/thing/service/property/set` | Receive property-set commands |
| `+/{deviceId}/thing/event/property/post_reply` | Receive replies to property reports |
| `+/{deviceId}/thing/event/file/upload_reply` | Receive file upload credentials |
| `+/{deviceId}/ota/upgrade` | Receive OTA upgrade notifications |
| `+/{deviceId}/ota/version_reply` | OTA version query replies |

The `+` wildcard matches the productId, so subscriptions remain valid even if the product ID changes.

## Device Lifecycle

A typical interaction sequence from connection to disconnection:

```
1. Connect to MQTT Broker (HMAC password authentication)
2. Report initial properties → thing/event/property/post
3. Receive cloud command ← thing/service/property/set (auto-subscribed)
4. Reply with command acknowledgment → thing/service/property/set_reply
5. Report event data → thing/event/{type}/post
6. Disconnect
```

For full protocol details (message formats, reply mechanisms, OTA flow), see the [Thing Model Protocol Specification](thing-model-spec-en.md).
