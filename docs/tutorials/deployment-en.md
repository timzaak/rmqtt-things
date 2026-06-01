# Deployment

RMQTT Things is deployed with Docker, running five containers on a single machine.

## Architecture

```
Internet ──→ Caddy (80/443) ──→ App (8080)
                                    |
                       PostgreSQL + Redis + RMQTT Broker

MQTT Devices ──→ RMQTT (1883)
```

External traffic enters through Caddy. Caddy handles TLS termination and forwards requests to the App container on port 8080. The App serves both API requests (`/api/*`) and frontend static files (`/app/web`).

MQTT devices do not go through Caddy; they connect directly to RMQTT on port 1883. Device-reported properties and events are forwarded to the App's HTTP endpoints via RMQTT WebHook callbacks and stored in the database.

All containers are attached to the same Docker network and communicate using container names (e.g., the App connects to PostgreSQL at `rmqtt-things-postgres:5432`).

## Prerequisites

- Linux server (Ubuntu 22.04+ or Debian 12+), at least 2 GB RAM
- Docker Engine 24+ and Docker CLI
- A domain name with a DNS A record pointing to the server IP
- Firewall open on three ports: 80 (HTTP), 443 (HTTPS), 1883 (MQTT)

## Preparation

### Create the Docker Network

```bash
docker network create rmqtt-things-net
```

All containers will join this network.

### Create Volumes

```bash
docker volume create pgdata
docker volume create redisdata
docker volume create caddy-data
docker volume create caddy-config
```

These four volumes store database data, Redis persistence, and Caddy certificates. Docker volume data survives container removal.

### Create the Configuration Directory

```bash
mkdir -p /opt/rmqtt-things/conf/plugins
```

Copy the project's RMQTT configuration there:

```bash
cp conf/rmqtt.toml /opt/rmqtt-things/conf/
cp conf/plugins/*.toml /opt/rmqtt-things/conf/plugins/
```

### Update the RMQTT Callback Address

The RMQTT configuration in the repository uses `host.docker.internal:8080`, which is for local development. For production, change it to the container name:

```bash
sed -i 's|http://host.docker.internal:8080|http://rmqtt-things-app:8080|g' \
    /opt/rmqtt-things/conf/plugins/rmqtt-web-hook.toml

sed -i 's|http://host.docker.internal:8080|http://rmqtt-things-app:8080|g' \
    /opt/rmqtt-things/conf/plugins/rmqtt-auth-http.toml
```

Verify the change:

```bash
grep -r "host.docker.internal" /opt/rmqtt-things/conf/plugins/
```

The output should be empty.

### Prepare the App Configuration

Copy the production config template to the server:

```bash
cp docs/tutorials/config.production.toml /opt/rmqtt-things/config.production.toml
```

Edit this file and replace all `CHANGE_ME` placeholders with actual values. The fields to update:

```toml
[database]
url = "postgres://rmqtt_user:your_password@rmqtt-things-postgres:5432/rmqtt_things"

[cache]
redis_url = "redis://rmqtt-things-redis:6379"

[mqtt]
url = "http://rmqtt-things-rmqtt:6060/api/v1"

[mqtt.access.auth]
suffix = "a random string used for device authentication"

[ca]
domain = "*.your-domain.com"

[s3]
endpoint = "your S3-compatible storage endpoint"
access_key = "your access key"
secret_key = "your secret key"
bucket = "rmqtt-things"

# For admin authentication (recommended for production)
[herald]
base_url = "http://herald:3000"              # Herald container name or address
api_key = "your Herald API Key"
realm_id = "rmqtt"
client_id = "admin-web-console"
```

Redis has no password because the Docker network does not expose its port externally. If you expose the Redis port to the outside, you must add a password.

### Prepare the Caddy Configuration

```bash
cp docs/tutorials/Caddyfile /opt/rmqtt-things/Caddyfile
```

Edit the Caddyfile and replace `your-domain.com` with your actual domain. The final content is just two lines:

```
your-domain.com {
    reverse_proxy rmqtt-things-app:8080
}
```

Caddy automatically requests TLS certificates from Let's Encrypt and handles renewal. No additional certificate configuration is needed.

## Start Services

Start services in this order: PostgreSQL -> Redis -> RMQTT -> App -> Caddy. The App needs to connect to the database and Redis on startup, while RMQTT does not need the App at startup, so bring up the infrastructure services first.

### PostgreSQL

```bash
docker run -d \
    --name rmqtt-things-postgres \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e POSTGRES_USER=rmqtt_user \
    -e POSTGRES_PASSWORD=your_password \
    -e POSTGRES_DB=rmqtt_things \
    -v pgdata:/var/lib/postgresql/data \
    postgres:18-alpine
```

Verify:

```bash
docker exec rmqtt-things-postgres pg_isready -U rmqtt_user
```

Output `/var/run/postgresql:5432 - accepting connections` indicates the database is ready.

### Redis

```bash
docker run -d \
    --name rmqtt-things-redis \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -v redisdata:/data \
    redis:8-alpine \
    redis-server --appendonly yes
```

`--appendonly yes` enables AOF persistence so data survives Redis restarts.

Verify:

```bash
docker exec rmqtt-things-redis redis-cli ping
```

Output `PONG` means Redis is running.

### RMQTT Broker

```bash
docker run -d \
    --name rmqtt-things-rmqtt \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -p 1883:1883 \
    -v /opt/rmqtt-things/conf:/app/rmqtt/conf \
    rmqtt/rmqtt:0.21.0
```

Port 1883 is mapped to the host so MQTT devices can connect through it. The RMQTT management API port (6060) is not exposed externally; only the App container accesses it through the Docker network.

Verify:

```bash
docker logs rmqtt-things-rmqtt --tail 20
```

Look for log entries confirming that the `rmqtt-web-hook` and `rmqtt-auth-http` plugins loaded successfully, which means the configuration is working.

### App

```bash
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /opt/rmqtt-things/config.production.toml:/app/config.toml:ro \
    ghcr.io/<owner>/rmqtt-things:<tag>
```

Replace `<owner>` with your GitHub username or organization, and `<tag>` with the version (e.g., `v0.1.0`).

The App runs database migrations automatically on startup (`sqlx::migrate!`). You do not need to create tables manually. However, back up the database before each deployment since migrations are irreversible.

Verify:

```bash
docker exec rmqtt-things-app wget -qO- http://localhost:8080/api/health
```

A response of `{"status":"health"}` means the service is running.

### Caddy

```bash
docker run -d \
    --name caddy \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -p 80:80 \
    -p 443:443 \
    -v /opt/rmqtt-things/Caddyfile:/etc/caddy/Caddyfile:ro \
    -v caddy-data:/data \
    -v caddy-config:/config \
    caddy:2-alpine
```

On first startup, Caddy requests a certificate from Let's Encrypt. If the domain DNS has not propagated yet, or port 80 is blocked by a firewall, the certificate request will fail and Caddy will keep retrying.

Verify:

```bash
curl -I https://your-domain.com
```

An `HTTP/2 200` response means deployment is complete. Opening `https://your-domain.com` in a browser should show the frontend.

## Verify the Full Deployment

After deployment, go through this checklist:

1. Open `https://your-domain.com` in a browser and confirm the frontend loads
2. `curl https://your-domain.com/api/health` returns 200
3. `docker exec rmqtt-things-redis redis-cli ping` returns PONG
4. `docker exec rmqtt-things-postgres pg_isready -U rmqtt_user` returns accepting connections
5. Connect to port 1883 with an MQTT client and confirm the connection succeeds

## CI/CD

The project uses GitHub Actions for automated builds and pushes. The workflow is defined in `.github/workflows/cd.yml`.

Trigger: push a tag starting with `v` (e.g., `git tag v0.1.0 && git push origin v0.1.0`).

The workflow:
1. Checks out the code
2. Logs in to GHCR (GitHub Container Registry)
3. Builds the Docker image (using GitHub Actions cache for speed)
4. Pushes to `ghcr.io/<owner>/rmqtt-things:<tag>`

The Dockerfile uses a multi-stage build with five stages:

| Stage | Base Image | Purpose |
|-------|-----------|---------|
| chef | rust:1.91-slim | Installs cargo-chef |
| planner | chef | Analyzes the dependency graph, generates recipe.json |
| builder | chef | Compiles dependencies first (cache layer), then compiles the project |
| frontend-builder | node:20-slim | Exports OpenAPI spec from builder, generates frontend API client, builds the frontend |
| runtime | debian:bookworm-slim | Copies only the binary and frontend assets, runs as non-root user |

The dependency caching design is key: as long as `Cargo.toml` and `Cargo.lock` are unchanged, the dependency layer is cached and only the application code is recompiled. The same applies to the frontend -- unchanged `package.json` and `package-lock.json` reuse `node_modules`.

The final runtime image contains only the binary, frontend static assets, and ca-certificates, keeping it small. The process runs as the `rmqtt` user, not root.

### Release a New Version

1. Tag and push to trigger the GitHub Actions image build:

```bash
git tag v0.2.1
git push origin v0.2.1
```

2. After GitHub Actions finishes, SSH into the production server, set the version and run the upgrade:

```bash
VERSION=0.3.0  # Replace with the target version

# Pull the new image
docker pull ghcr.io/timzaak/rmqtt-things:${VERSION}

# Stop and remove the old container
docker stop rmqtt-things-app
docker rm rmqtt-things-app

# Start with the new image
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /server/conf/rmqtt-thing/config.toml:/app/config.toml:ro \
    ghcr.io/timzaak/rmqtt-things:${VERSION}
```

3. Verify:

```bash
# Check logs for successful startup (should see "Listening on port 8080")
docker logs rmqtt-things-app --tail 10

# Health check (no curl/wget in the container, use caddy container instead)
docker exec caddy wget -qO- http://rmqtt-things-app:8080/api/health
# Should return {"status":"health",...}
```

### Rollback

If the new version has issues, re-run the stop and start steps with the previous tag:

```bash
docker stop rmqtt-things-app
docker rm rmqtt-things-app
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /server/conf/rmqtt-thing/config.toml:/app/config.toml:ro \
    ghcr.io/timzaak/rmqtt-things:0.3.0  # Rollback to the previous version
```

### Backup Database Before Upgrade

The App runs database migrations automatically on startup (`sqlx::migrate!`). Migrations are irreversible. Back up before upgrading:

```bash
docker exec rmqtt-things-postgres pg_dump -U rmqtt_user rmqtt_things > backup_$(date +%Y%m%d).sql
```

> Only the `rmqtt-things-app` container needs to be replaced during an upgrade. Other containers are unaffected. There will be a few seconds of downtime between stop and start.

## Data Persistence

The App itself is stateless. All persistent data is stored in these locations:

| Data | Storage Location | Volume or Mount |
|------|-----------------|-----------------|
| Business data (devices, products, events, etc.) | PostgreSQL | `pgdata` volume |
| Schema cache | Redis | `redisdata` volume (AOF persistence) |
| TLS certificates | Caddy | `caddy-data` volume |
| RMQTT configuration | Host | `/opt/rmqtt-things/conf/` directory mount |
| App configuration | Host | `/opt/rmqtt-things/config.production.toml` file mount |
| Caddyfile | Host | `/opt/rmqtt-things/Caddyfile` file mount |

Back up the database with `pg_dump`:

```bash
docker exec rmqtt-things-postgres pg_dump -U rmqtt_user rmqtt_things > backup.sql
```

Restore:

```bash
cat backup.sql | docker exec -i rmqtt-things-postgres psql -U rmqtt_user rmqtt_things
```

Redis data loss is not critical -- the App will rebuild the cache automatically. If you want to back it up anyway:

```bash
docker exec rmqtt-things-redis redis-cli BGSAVE
docker cp rmqtt-things-redis:/data/dump.rdb ./redis-backup.rdb
```

## Troubleshooting

### Caddy Certificate Request Fails

If logs show `acme: error` or similar messages, check:
- Whether the domain DNS points to the server IP (confirm with `dig your-domain.com`)
- Whether the firewall allows ports 80 and 443
- Whether another process is using port 80 on the server (`ss -tlnp | grep :80`)

### App Cannot Connect to the Database

App logs show `connection refused` or `no route to host`.

Check that the containers are on the same network:
```bash
docker network inspect rmqtt-things-net
```

You should see the postgres, app, and other containers attached to this network. Confirm that the database address in the config file is `rmqtt-things-postgres:5432`, not `localhost`.

### RMQTT WebHook Not Working

The App is not receiving device-reported data. First confirm the callback address in the RMQTT configuration has been changed to the container name:

```bash
grep "rmqtt-things-app" /opt/rmqtt-things/conf/plugins/rmqtt-web-hook.toml
```

You should see URLs starting with `http://rmqtt-things-app:8080`. If they still show `host.docker.internal`, the earlier `sed` replacement did not take effect.

### Device Connects to RMQTT but App Rejects It

The RMQTT authentication plugin (`rmqtt-auth-http`) calls the App's `/api/access/auth` endpoint each time a device connects. Check the App logs for incoming authentication requests and verify that the `suffix` in the `[mqtt.access.auth]` configuration matches what the device is configured with.

### App Exits Immediately After Starting

This is usually a database migration failure. Check the logs:

```bash
docker logs rmqtt-things-app --tail 100
```

Common causes: incorrect database password in the config, or PostgreSQL has not fully started yet. With `postgres:18-alpine`, the initial database setup on first startup takes a few seconds. Wait until `pg_isready` reports success before starting the App.

### Applying Caddyfile Changes

```bash
docker exec caddy caddy reload --config /etc/caddy/Caddyfile
```

No need to restart the Caddy container.

### Herald Authentication Not Working

Admin endpoints accept requests without authentication.

Check that the `[herald]` section in the config file is not commented out and all four fields (`base_url`, `api_key`, `realm_id`, `client_id`) are set. Verify that the `base_url` is reachable from the App container:

```bash
docker exec rmqtt-things-app wget -qO- http://herald:3000
```

### Herald Connection Timeout

App logs show connection timeouts when calling Herald.

Ensure both the App and Herald containers are on the same Docker network:

```bash
docker network inspect rmqtt-things-net
```

If Herald is not listed, connect it:

```bash
docker network connect rmqtt-things-net herald
```

### Login Loop After Herald Login

After logging in via Herald SSO, the browser keeps redirecting back to the login page.

Check Cookie domain settings. The Cookie domain must match the domain the browser is accessing. If using Caddy with a custom domain, ensure Herald's redirect URL and Cookie domain are configured for the same domain.
