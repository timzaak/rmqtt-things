[English](README.md) | [中文](README_zh.md)

# RMQTT Things

IoT thing-model management platform built on [RMQTT](https://github.com/rmqtt/rmqtt). Entirely developed with AI, ships with a built-in skill system for AI-driven secondary development.

**Live demo:** https://mqtt.fornetcode.com

Demo login (Herald):

| Field | Value |
|---|---|
| Email | `admin@rmqtt-things.local` |
| Password | `password` |
| Realm | `rmqtt` |

<!-- TODO: Add screenshot of management UI -->

## Simulating a Device with MQTTX

Use [MQTTX](https://mqttx.app/) to simulate a device against the demo server. Auth is an HMAC-SHA1 password — the demo server accepts a 7-day skew (`timestamp_tolerance_secs`), so one password lasts for testing:

> For real devices: sync the clock (SNTP) first, and regenerate the password before every MQTT `CONNECT` including reconnection — the production default is a 5-minute skew.

```bash
DEVICE_ID="mqttx_demo_001"; SUFFIX="4f39c2635d373677b95edc460bb99ba4"
NONCE="a1b2c3"; TS=$(date +%s)
HASH=$(printf '%s' "${DEVICE_ID}.${NONCE}.${TS}.${SUFFIX}" | openssl dgst -sha1 -hmac "$SUFFIX" | awk '{print $NF}')
echo "${NONCE}.${TS}.${HASH}"   # paste this as the MQTTX password
```

MQTTX connection: Host `152.32.249.178`, Port `1883` (plain TCP), Client ID = `mqttx_demo_001`, Username = `test-1` (the product `model_no`). `test-1` has auto-provisioning on, so new device ids register on first connect.

Publish properties to `test-1/mqttx_demo_001/thing/event/property/post`:

```json
{"id":"1","params":{"temperature":25.5,"humidity":60},"ack":0}
```

Verify: `curl -s "https://mqtt.fornetcode.com/api/admin/property?product_id=test-1&device_id=mqttx_demo_001"`

Commands arrive on `test-1/mqttx_demo_001/thing/service/property/set` (auto-subscribed — no manual SUBSCRIBE needed). Reply on `.../set_reply` with the received `id`:

```json
{"id":"<received-id>","code":200,"data":{}}
```

Full topic spec and other-language password code: [Thing model spec](docs/tutorials/thing-model-spec-en.md), [Device integration guide](docs/tutorials/device-integration-en.md).

## Why this project matters

This isn't just another IoT platform. The point is: **a production-grade project fully built by AI, with a workflow that lets AI handle ongoing development.**

Development runs on [web-dev-skills](https://github.com/timzaak/web-dev-skills), a standalone AI-development plugin that chains requirements, design, implementation, and testing into a single pipeline. Load it into your AI coding agent — Claude Code, ZCode, Codex, or any agent that supports skills — clone this repo, and start iterating: describe what you want, AI does the rest.

web-dev-skills is project-agnostic and also covers miniapp, Flutter, and Chrome-extension stacks — see its README for installation and the full command list.

The Rust + React stack was chosen for AI coding: the compiler and type system are the best QA for AI-generated code, and OpenAPI-to-TypeScript codegen keeps the frontend in sync with backend APIs.

## Features

Devices report over MQTT, a Rust backend receives data via WebHook and persists to PostgreSQL, a React frontend provides the management UI.

- Device lifecycle: connect/disconnect tracking, auto-provisioning, property reporting, event history
- Property shadow (desired state + delta) and property history charts
- Command delivery (property set, service/action invocation) and OTA firmware updates
- Alarm rules with a rule engine (webhook notifications) and event validation templates
- Device file upload to S3-compatible object storage
- TLS certificate issuance (external CA: generate with `--generate-ca` or bring your own)
- Herald SSO login with role-based permissions (optional; single-tenant mode without it)

Tech stack: Rust / Axum / SQLx / PostgreSQL / React 19 / TanStack / Redis (optional) / S3

## Quick start

Prerequisites: Docker, Rust toolchain, Node.js. See [getting started](docs/tutorials/getting-started-en.md) for full instructions.

```shell
# PostgreSQL
docker run --rm --name postgres \
  -e POSTGRES_DB=rmqtt_things -e POSTGRES_USER=rmqtt_user -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 postgres:18-alpine

# RMQTT broker (from the project root)
docker run --rm --name rmqtt -p 1883:1883 -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf rmqtt/rmqtt:0.23.1 -f conf/rmqtt.toml

# Backend
cd backend
cp config.example.toml config.toml
cargo run -- --generate-ca   # one-time: create the CA required at runtime
cargo run

# Frontend
cd ../frontend && npm install && npm run dev
```

## Documentation

| Document | Description |
|---|---|
| [Getting started](docs/tutorials/getting-started-en.md) | Full setup guide |
| [Developing with AI](docs/tutorials/ai-development-en.md) | Skill pipeline and workflow |
| [Architecture](docs/tutorials/architecture-en.md) | System design and tech decisions |
| [Thing model spec](docs/tutorials/thing-model-spec-en.md) | MQTT topic format and message schema |
| [API reference](docs/tutorials/api-reference-en.md) | Admin and webhook API docs |
| [Deployment](docs/tutorials/deployment-en.md) | Production deployment guide |

## License

[MIT](LICENSE-MIT) / [Apache 2.0](LICENSE-APACHE)
