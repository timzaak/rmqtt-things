# Connecting Your First Device

Use an MQTT client to simulate a device and walk through the full workflow of property reporting and command reception. Before you begin, complete the [Quick Start](getting-started-en.md) to ensure the backend service is running.

## Prerequisites

Install the [mosquitto](https://mosquitto.org/) client tools, or use the [MQTTX](https://mqttx.app/) graphical client. The examples below use the mosquitto CLI; MQTTX users can map the parameters accordingly.

## Step 1: Create a Product

```bash
curl -X POST http://localhost:8080/api/admin/product \
  -H "Content-Type: application/json" \
  -d '{"name": "我的第一个产品", "model_no": "my_product", "description": "教程测试产品"}'
```

`model_no` is the unique identifier for the product and also serves as the MQTT connection `username`.

## Step 2: Generate a Device Password

Devices need a password to connect to the MQTT broker, generated via HMAC-SHA1. Format: `nonce.timestamp.hex(hmac_sha1(suffix, deviceId.nonce.timestamp.suffix))`

Generate it with the following command (requires openssl):

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

The timestamp must not differ from server time by more than the configured tolerance (default 5 minutes; raise it via `timestamp_tolerance_secs` if you want one password to last longer during testing). If your development machine and server clocks are out of sync, synchronize them first.

Password generation code in other languages (JavaScript, Python, C) is available in the [Device Development Reference](device-guide-en.md).

## Step 3: Report Device Properties

Use `mosquitto_pub` to simulate a device reporting properties:

```bash
mosquitto_pub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/event/property/post" \
  -m '{"id":"1","params":{"temperature":25.5,"humidity":60},"ack":0}'
```

Parameter descriptions:
- `-i`: MQTT client_id, i.e. the device ID
- `-u`: MQTT username, i.e. the product's `model_no`
- `-P`: HMAC-generated password
- `-t`: Property report topic

## Step 4: Verify Data Arrival

Query the device's latest properties:

```bash
curl "http://localhost:8080/api/admin/property?product_id=my_product&device_id=my_device_001"
```

If the response contains `temperature: 25.5` and `humidity: 60`, the data has arrived successfully.

## Step 5: Send a Property Command

Send a command to the device via the API:

```bash
curl -X POST http://localhost:8080/api/admin/property/command \
  -H "Content-Type: application/json" \
  -d '{"product_id":"my_product","device_id":"my_device_001","command":{"power":"on"}}'
```

## Step 6: Receive Commands on the Device

In a separate terminal, use `mosquitto_sub` to listen for commands:

```bash
mosquitto_sub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/service/property/set"
```

In production, the RMQTT auto-subscription plugin automatically subscribes to this topic when the device connects, so the device does not need to subscribe manually.

After receiving a command message, the device should reply with an acknowledgment:

```bash
mosquitto_pub -h 127.0.0.1 -p 1883 \
  -i "my_device_001" \
  -u "my_product" \
  -P "$PASSWORD" \
  -t "my_product/my_device_001/thing/service/property/set_reply" \
  -m '{"id":"收到的消息id","code":200,"data":{}}'
```

## Completion Checklist

You have now walked through the core device integration workflow:

1. Create a product
2. Generate an HMAC password
3. Report device properties
4. Send a command from the platform
5. Device replies with acknowledgment

Next steps: For the full protocol specification (topic formats, message formats, OTA, file upload), see the [Thing Model Protocol Specification](thing-model-spec-en.md). For multi-language password generation code and a topic quick-reference table, see the [Device Development Reference](device-guide-en.md).
