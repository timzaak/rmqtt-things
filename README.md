[English](README.md) | [中文](README_zh.md)

# RMQTT Things

IoT thing-model management platform built on [RMQTT](https://github.com/rmqtt/rmqtt). Devices report telemetry and events over MQTT, a Rust backend persists to PostgreSQL and exposes admin APIs, a React frontend provides the management UI. Production-ready.

Ships with Claude Code skills covering the full development cycle: requirements, technical design, task planning, implementation, and testing. The Rust/React stack was chosen for AI coding compatibility — the compiler and type system catch errors that dynamic languages miss, and OpenAPI-to-TypeScript codegen keeps the frontend in sync with backend APIs.

## Features

Backend (Rust / Axum / SQLx / PostgreSQL):
- Device lifecycle: connect/disconnect tracking, property reporting, event history
- Command delivery: send instructions to devices, track status
- OTA firmware updates with version management
- TLS certificate issuance (built-in CA via rcgen)
- S3 file upload with presigned URLs
- Swagger UI at `/api/swagger-ui`

Frontend (React 19 / TanStack Router + Query / Tailwind / Radix UI):
- Device management and monitoring
- Certificate issuance and lifecycle
- OTA scheduling and deployment
- Product configuration
- Property and event viewer

MQTT integration:
- RMQTT webhook-based event routing
- HMAC-SHA1 device authentication
- Per-device topic ACL

Testing:
- Playwright E2E tests covering device registration, certificate issuance, OTA, and property commands

## AI development pipeline

Built-in Claude Code skills chain PRD, design, implementation, and testing into a single pipeline with quality gates between phases. See [Developing with Claude Code](docs/tutorials/ai-development-en.md) for details.

## Quick start

Prerequisites: Docker, Rust toolchain, Node.js.

```shell
# PostgreSQL
docker run --rm --name=postgres \
  -e POSTGRES_DB=rmqtt_things -e POSTGRES_USER=rmqtt_user -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 postgres:18-alpine \
  postgres -c log_statement=all -c log_destination=stderr

# RMQTT broker
docker run --rm --name rmqtt -p 1883:1883 -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf rmqtt/rmqtt:0.20.0 -f conf/rmqtt.toml

# Backend
cd backend && cp config.example.toml config.toml && cargo run

# Frontend
cd frontend && npm install && npm run dev
```

Swagger UI: http://localhost:8080/api/swagger-ui

## Demo

A live instance is deployed at:

| Service | URL |
|---|---|
| Frontend | https://mqtt.fornetcode.com |
| Swagger UI | https://mqtt.fornetcode.com/api/swagger-ui |
| Health check | https://mqtt.fornetcode.com/api/health |
| MQTT broker | `mqtt.fornetcode.com:1883` |

Quick test:

```shell
# API health
curl https://mqtt.fornetcode.com/api/health

# Connect a device with mosquitto
mosquitto_pub -h mqtt.fornetcode.com -p 1883 \
  -t "test_device/thing/event/property/post" \
  -m '{"temperature": 25.5}'
```

Open the frontend URL in a browser to explore the management UI. Use Swagger UI to try admin APIs directly.

## Project structure

```
backend/         Rust backend (Axum + SQLx + PostgreSQL)
frontend/        React SPA (TanStack Router/Query + Tailwind + Radix UI)
demo/            Playwright E2E tests
conf/            RMQTT broker config and plugin rules
docs/            Tutorials, API reference, architecture
.claude/         Skills, agents, guides, protocols
```

## Documentation

| Document | Description |
|---|---|
| [Getting started](docs/tutorials/getting-started-en.md) | Full setup guide |
| [Thing model spec](docs/tutorials/thing-model-spec-en.md) | MQTT topic format and message schema |
| [API reference](docs/tutorials/api-reference-en.md) | Admin and webhook API docs |
| [Architecture](docs/tutorials/architecture-en.md) | System design and tech decisions |
| [Deployment](docs/tutorials/deployment-en.md) | Production deployment guide |
| [Developing with Claude Code](docs/tutorials/ai-development-en.md) | AI development pipeline and workflow |

## Extending

Complete runnable product. Fork and customize payload format, RPC ack, topic ACL, and device auth as needed. For AI-driven feature development, use the skills above with Claude Code.

RMQTT plugin configs: [conf/plugins](conf/plugins). Production: set `allow_anonymous = false` in `rmqtt.toml`.

## License

[MIT](LICENSE-MIT) / [Apache 2.0](LICENSE-APACHE)
