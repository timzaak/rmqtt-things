# 设备端开发参考

面向固件和嵌入式开发者。如果你的设备要接入 RMQTT Things 平台，这篇文档包含所有你需要的信息。

完整的参考实现见 [`demo/e2e/helpers/mqtt-device.ts`](../../demo/e2e/helpers/mqtt-device.ts)。

## MQTT 连接参数

| 参数 | 值 | 说明 |
|------|-----|------|
| Broker 地址 | `mqtt://host:1883` | TCP 明文连接 |
| Broker 地址（TLS） | `mqtts://host:8883` | 需要客户端证书 |
| `client_id` | 设备 ID | 全局唯一 |
| `username` | 产品的 `model_no` | 创建产品时指定 |
| `password` | HMAC-SHA1 生成值 | 见下方 |

## HMAC-SHA1 密码生成

密码格式：`nonce.timestamp.hex(hmac_sha1(suffix, deviceId.nonce.timestamp.suffix))`

- `nonce`：6 位随机十六进制字符串
- `timestamp`：当前 Unix 时间戳（秒），与服务器时间差不得超过 5 分钟
- `suffix`：共享密钥，后端配置 `[mqtt.access.auth]` 中的 `suffix` 字段，默认 `suffix_go`

### JavaScript / TypeScript

```javascript
import { createHmac, randomBytes } from 'node:crypto'

function generatePassword(deviceId, suffix = 'suffix_go') {
  const nonce = randomBytes(3).toString('hex') // 6位十六进制
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
    nonce = secrets.token_hex(3)  # 6位十六进制
    timestamp = str(int(time.time()))
    to_sign = f"{device_id}.{nonce}.{timestamp}.{suffix}"
    h = hmac.new(suffix.encode(), to_sign.encode(), hashlib.sha1).hexdigest()
    return f"{nonce}.{timestamp}.{h}"
```

### C（嵌入式参考）

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// 需要一个 HMAC-SHA1 实现，如 mbedTLS 或 wolfSSL
// 以下为伪代码框架

void generate_password(const char *device_id, const char *suffix,
                       char *output, size_t output_len) {
    char nonce[7];
    // 生成 6 位随机十六进制
    snprintf(nonce, sizeof(nonce), "%06x", rand() & 0xFFFFFF);

    time_t ts = time(NULL);
    char to_sign[128];
    snprintf(to_sign, sizeof(to_sign), "%s.%s.%ld.%s",
             device_id, nonce, (long)ts, suffix);

    unsigned char hmac_result[20];
    // hmac_sha1(suffix, to_sign, hmac_result) — 使用你的 HMAC 库

    char hex_hash[41];
    // bytes_to_hex(hmac_result, 20, hex_hash)

    snprintf(output, output_len, "%s.%ld.%s", nonce, (long)ts, hex_hash);
}
```

## 主题速查表

`{pid}` = productId（即 model_no），`{did}` = deviceId。

| 方向 | 主题模式 | 用途 |
|------|----------|------|
| 设备→云端 | `{pid}/{did}/thing/event/property/post` | 上报属性 |
| 设备→云端 | `{pid}/{did}/thing/event/{type}/post` | 上报事件 |
| 云端→设备 | `{pid}/{did}/thing/service/property/set` | 设置属性（命令） |
| 设备→云端 | `{pid}/{did}/thing/service/property/set_reply` | 回复命令 |
| 设备→云端 | `{pid}/{did}/thing/file/upload` | 请求文件上传 |
| 云端→设备 | `{pid}/{did}/thing/file/upload_reply` | 返回上传凭证 |
| 设备→云端 | `{pid}/{did}/ota/version` | 上报固件版本 |
| 云端→设备 | `{pid}/{did}/ota/upgrade` | 推送 OTA 升级 |

## 认证与权限

### 认证

RMQTT 的 auth-http 插件在设备连接时调用后端 `/api/access/auth` 验证 HMAC 密码。验证流程：

1. 拆分密码，检查 nonce 长度 6 位、时间戳格式正确
2. 时间戳与当前时间差超过 300 秒（5 分钟），拒绝
3. 用配置里的 `suffix` 作为密钥，对 `{clientId}.{nonce}.{timestamp}.{suffix}` 算 HMAC-SHA1
4. 比对哈希值，一致返回 `"allow"`，否则 `"deny"`

后端挂了直接拒绝连接（`deny_if_error = true`），宁可设备连不上也不放未认证设备进来。

### ACL

RMQTT 的 ACL 插件在设备每次 PUBLISH 或 SUBSCRIBE 时调用后端 `/api/access/acl` 校验权限。规则：

1. topic 的第二段（deviceId）必须等于 clientId，设备只能操作自己的主题空间
2. topic 的第一段（productId）必须等于 username
3. 只允许 `thing/event/*`、`thing/service/*`、`ota/*` 这几类 topic
4. 其他全部 deny

### 自动订阅

设备连接后，RMQTT auto-subscription 插件自动订阅以下主题，设备不需要手动发 SUBSCRIBE：

| 主题 | 用途 |
|------|------|
| `+/{deviceId}/thing/service/property/set` | 接收属性设置命令 |
| `+/{deviceId}/thing/event/property/post_reply` | 收到属性上报的回复 |
| `+/{deviceId}/thing/event/file/upload_reply` | 收到文件上传凭证 |
| `+/{deviceId}/ota/upgrade` | 接收 OTA 升级通知 |
| `+/{deviceId}/ota/version_reply` | OTA 版本查询回复 |

通配符 `+` 匹配 productId，产品 ID 改了不影响订阅。

## 设备生命周期

一个设备从连接到断开的典型交互序列：

```
1. 连接 MQTT Broker（HMAC 密码认证）
2. 上报初始属性 → thing/event/property/post
3. 接收云端命令 ← thing/service/property/set（自动订阅）
4. 回复命令确认 → thing/service/property/set_reply
5. 上报事件数据 → thing/event/{type}/post
6. 断开连接
```

完整的协议细节（消息格式、reply 机制、OTA 流程）见 [物模型协议规范](thing-model-spec.md)。
