# RMQTT Things

RMQTT Things connects MQTT devices to a management backend. Devices report properties and events via the RMQTT Broker; the backend receives WebHook callbacks and writes them to PostgreSQL. The frontend provides operational interfaces for device management, OTA upgrades, certificate issuance, and property commands.

## Audience

Backend engineers who need to deploy or extend this system. Assumes experience with Rust and React projects. If you just want to get it running, the Quick Start guide is all you need.

## Prerequisites

- Rust fundamentals — able to read Axum handlers and SQLx queries
- Basic operations with PostgreSQL and Redis
- MQTT protocol concepts (topic, QoS, retain)
- Day-to-day use of Docker and docker compose

## Chapters

- [Quick Start](getting-started-en.md) — From zero to running
- [Thing Model Protocol Specification](thing-model-spec-en.md) — MQTT topics, message formats, authentication, OTA, and other device communication protocols
- [Architecture](architecture-en.md) — Directory structure, technology choices, core data flows
- [API Reference](api-reference-en.md) — All HTTP endpoints
- [Connecting Your First Device](device-integration-en.md) — Walk through property reporting and command reception using mosquitto
- [Device-side Development Reference](device-guide-en.md) — Connection parameters, password generation, topic quick-reference table
- [Configuration](configuration-en.md) — Configuration options explained
- [Authentication & Authorization](auth-en.md) — Herald SSO for admin, HMAC for devices
- [Deployment](deployment-en.md) — Production deployment steps
- [Developing with Claude Code](ai-development-en.md) — AI workflow commands and practical steps
