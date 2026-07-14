# 连接你的第一个设备

用 MQTT 客户端模拟一个设备，走完属性上报和命令接收的完整流程。动手之前先完成 [快速上手](getting-started.md)，确保后端服务正常运行。

## 前置条件

安装 [mosquitto](https://mosquitto.org/) 客户端工具，或者用 [MQTTX](https://mqttx.app/) 图形客户端。下面的示例用 mosquitto 命令行，MQTTX 用户对照参数填就行。

## Step 1: 创建产品

```bash
curl -X POST http://localhost:8080/api/admin/product \
  -H "Content-Type: application/json" \
  -d '{"name": "我的第一个产品", "model_no": "my_product", "description": "教程测试产品"}'
```

`model_no` 是产品的唯一标识，同时也是 MQTT 连接时的 `username`。

## Step 2: 生成设备密码

设备连接 MQTT Broker 需要密码，通过 HMAC-SHA1 生成。格式：`nonce.timestamp.hex(hmac_sha1(suffix, deviceId.nonce.timestamp.suffix))`

用以下命令生成（需要 openssl）：

```bash
DEVICE_ID="my_device_001"
SUFFIX="suffix_go"
NONCE="a1b2c3"
TIMESTAMP=$(date +%s)
TO_SIGN="${DEVICE_ID}.${NONCE}.${TIMESTAMP}.${SUFFIX}"
HASH=$(echo -n "$TO_SIGN" | openssl dgst -sha1 -hmac "$SUFFIX" | awk '{print $NF}')
PASSWORD="${NONCE}.${TIMESTAMP}.${HASH}"
echo "Password: $PASSWORD"
```

timestamp 与服务器时间差不能超过配置的容差（默认 5 分钟；测试时想一次密码用更久，调大 `timestamp_tolerance_secs`）。如果你的开发机和服务器时间不同步，先同步一下。

其他语言（JavaScript、Python、C）的密码生成代码见 [设备端开发参考](device-guide.md)。

## Step 3: 上报设备属性

用 mosquitto_pub 模拟设备上报属性：

```bash
mosquitto_pub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/event/property/post" \
  -m '{"id":"1","params":{"temperature":25.5,"humidity":60},"ack":0}'
```

参数说明：
- `-i`：MQTT client_id，即设备 ID
- `-u`：MQTT username，即产品的 `model_no`
- `-P`：HMAC 生成的密码
- `-t`：属性上报主题

## Step 4: 验证数据到达

查询设备的最新属性：

```bash
curl "http://localhost:8080/api/admin/property?product_id=my_product&device_id=my_device_001"
```

响应中包含 `temperature: 25.5` 和 `humidity: 60` 就说明数据到了。

## Step 5: 下发属性命令

通过 API 向设备发送命令：

```bash
curl -X POST http://localhost:8080/api/admin/property/command \
  -H "Content-Type: application/json" \
  -d '{"product_id":"my_product","device_id":"my_device_001","command":{"power":"on"}}'
```

## Step 6: 设备接收命令

在另一个终端，用 mosquitto_sub 监听命令：

```bash
mosquitto_sub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/service/property/set"
```

生产环境中 RMQTT 的 auto-subscription 插件会在设备连接时自动订阅此主题，设备端不需要手动订阅。

收到命令消息后，设备应回复确认：

```bash
mosquitto_pub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/service/property/set_reply" \
  -m '{"id":"收到的消息id","code":200,"data":{}}'
```

## 完成清单

到这里你走完了设备接入的核心流程：

1. 创建产品
2. 生成 HMAC 密码
3. 设备上报属性
4. 平台下发命令
5. 设备回复确认

下一步：完整的协议规范（topic 格式、消息格式、OTA、文件上传）见 [物模型协议规范](thing-model-spec.md)。多语言密码生成代码和主题速查表见 [设备端开发参考](device-guide.md)。
