# Getting Started

## Prerequisites

- Latest stable Rust (install via [rustup](https://rustup.rs/))
- Node.js 20 or later
- Docker (for running PostgreSQL and the RMQTT Broker)
- Git


## Start PostgreSQL

```bash
docker run --rm --name postgres \
  -e POSTGRES_DB=rmqtt_things \
  -e POSTGRES_USER=rmqtt_user \
  -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 \
  postgres:18-alpine \
  postgres -c log_statement=all -c log_destination=stderr
```

This starts a PostgreSQL 18 container with database `rmqtt_things`, user `rmqtt_user`, password `rmqtt_pass`, mapped to local port 5432. The `--rm` flag removes the container on stop — data is not persisted, which is fine for development.


## Start the RMQTT Broker

```bash
docker run --rm --name rmqtt \
  -p 1883:1883 \
  -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf \
  rmqtt/rmqtt:0.21.0 \
  -f conf/rmqtt.toml
```

Run this from the project root directory. It mounts the project's `conf/` directory into the container, so RMQTT loads the configuration including the WebHook plugin.

The two ports serve different purposes:
- **1883**: MQTT protocol port — devices connect here.
- **6060**: HTTP API port — the backend uses this to send messages to the broker.

The WebHook configuration is in `conf/plugins/rmqtt-web-hook.toml`. It is pre-configured to forward device connect/disconnect, property report, and other events to `http://host.docker.internal:8080`, which is your backend service.

## Configure the Backend

```bash
cd backend
cp config.example.toml config.toml
```

Then take a look at `config.toml`. The defaults work out of the box; adjust the following as needed:

| Setting | Default | When to change |
|---------|---------|----------------|
| `database.url` | `postgres://rmqtt_user:rmqtt_pass@localhost:5432/rmqtt_things` | If you changed the PostgreSQL username or password |
| `mqtt.url` | `http://127.0.0.1:6060/api/v1` | If you changed the RMQTT HTTP port |
| `api.openapi_enabled` | `true` | Set to `false` to hide the Swagger UI |
| `s3.*` | MinIO defaults | File upload depends on object storage; can ignore for local development |
| `ca.*` | Self-signed CA | Used by the certificate issuance feature; can leave as-is for local development |

If you started PostgreSQL and RMQTT with the commands above, `database.url` and `mqtt.url` require no changes — they will work as-is.

## Start the Backend

```bash
cd backend
cargo run
```

The first build takes a few minutes; subsequent runs after code changes are much faster. The backend listens on port 8080.

You will know it started successfully when you see a log line like:

```
Listening on 0.0.0.0:8080
```

The backend runs database migrations automatically and creates the required tables. No manual setup is needed.

## Start the Frontend

Open another terminal:

```bash
cd frontend
npm install
npm run dev
```

The frontend listens on port 3000. Open http://localhost:3000 to see the management UI.

`npm run dev` starts the Vite dev server with hot module replacement — the browser reloads automatically when you change code.

## Verify Everything Is Running

With both backend and frontend running, check in order:

1. Open http://localhost:8080/swagger — if you see the Swagger documentation page, the backend API is working.

2. Open http://localhost:3000 — if you see the management UI, the frontend is working.

3. Call an endpoint on the Swagger page, e.g. get device status (GET `/api/admin/device/status`). A 200 response with an empty list (not a 500 error) means the database connection is working.

At this point, your entire development environment is up and running.
