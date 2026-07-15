# 部署

RMQTT Things 用 Docker 部署，五个容器跑在同一台机器上。

## 架构

```
Internet ──→ Caddy (80/443) ──→ App (8080)
                                    |
                       PostgreSQL + Redis + RMQTT Broker

MQTT Devices ──→ RMQTT (1883)
```

外部流量走 Caddy 进来。Caddy 负责 TLS 终止，把请求转发到 App 容器的 8080 端口。App 同时处理 API 请求（`/api/*`）和前端静态文件（`/app/web`）。

MQTT 设备不走 Caddy，直连 RMQTT 的 1883 端口。设备上报的属性和事件通过 RMQTT WebHook 回调 App 的 HTTP 接口存进数据库。

所有容器挂在同一个 Docker 网络上，用容器名互相访问（比如 App 连 PostgreSQL 用 `rmqtt-things-postgres:5432`）。

## 前置条件

- Linux 服务器（Ubuntu 22.04+ 或 Debian 12+），至少 2GB 内存
- Docker Engine 24+ 和 Docker CLI
- 一个域名，DNS A 记录指向服务器 IP
- 防火墙开放三个端口：80（HTTP）、443（HTTPS）、1883（MQTT）

## 准备工作

### 创建 Docker 网络

```bash
docker network create rmqtt-things-net
```

后面所有容器都会加入这个网络。

### 创建 Volumes

```bash
docker volume create pgdata
docker volume create redisdata
docker volume create caddy-data
docker volume create caddy-config
```

四个 volume 分别存数据库、Redis 持久化、Caddy 证书。Docker volume 的数据在容器删除后不会丢。

### 创建配置目录

```bash
mkdir -p /opt/rmqtt-things/conf/plugins
```

把项目的 RMQTT 配置复制过去：

```bash
cp conf/rmqtt.toml /opt/rmqtt-things/conf/
cp conf/plugins/*.toml /opt/rmqtt-things/conf/plugins/
```

### 修改 RMQTT 回调地址

项目仓库里的 RMQTT 配置用的是 `host.docker.internal:8080`，这是本地开发的地址。生产环境需要改成容器名：

```bash
sed -i 's|http://host.docker.internal:8080|http://rmqtt-things-app:8080|g' \
    /opt/rmqtt-things/conf/plugins/rmqtt-web-hook.toml

sed -i 's|http://host.docker.internal:8080|http://rmqtt-things-app:8080|g' \
    /opt/rmqtt-things/conf/plugins/rmqtt-auth-http.toml
```

改完验证一下：

```bash
grep -r "host.docker.internal" /opt/rmqtt-things/conf/plugins/
```

应该没有任何输出。

### 准备 App 配置

把生产配置模板复制到服务器：

```bash
cp docs/tutorials/config.production.toml /opt/rmqtt-things/config.production.toml
```

编辑这个文件，把所有 `CHANGE_ME` 替换成实际值。需要改的项：

```toml
[database]
url = "postgres://rmqtt_user:你的密码@rmqtt-things-postgres:5432/rmqtt_things"

[cache]
redis_url = "redis://rmqtt-things-redis:6379"

[mqtt]
url = "http://rmqtt-things-rmqtt:6060/api/v1"

[mqtt.access.auth]
suffix = "一个随机字符串，用于设备认证"

[ca]
domain = "*.your-domain.com"

[s3]
endpoint = "你的 S3 兼容存储地址"
access_key = "你的 access key"
secret_key = "你的 secret key"
bucket = "rmqtt-things"

# 如果需要管理端认证（推荐生产环境开启）
[herald]
base_url = "http://herald:3000"              # Herald 容器名或地址
api_key = "你的 Herald API Key"
realm_id = "rmqtt"
client_id = "admin-web-console"
```

Redis 没有密码，因为 Docker 网络不对外暴露端口。如果你对外暴露了 Redis 端口，需要加密码。

### 准备 Caddy 配置

```bash
cp docs/tutorials/Caddyfile /opt/rmqtt-things/Caddyfile
```

编辑 Caddyfile，把 `your-domain.com` 改成你的域名。最终内容只有两行：

```
your-domain.com {
    reverse_proxy rmqtt-things-app:8080
}
```

Caddy 会自动向 Let's Encrypt 申请 TLS 证书，也会自动续期。不需要额外配置证书。

## 启动服务

按 PostgreSQL -> Redis -> RMQTT -> App -> Caddy 的顺序启动。因为 App 启动时要连数据库和 Redis，RMQTT 启动时不需要连 App，所以先把基础服务拉起来。

### PostgreSQL

```bash
docker run -d \
    --name rmqtt-things-postgres \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e POSTGRES_USER=rmqtt_user \
    -e POSTGRES_PASSWORD=你的密码 \
    -e POSTGRES_DB=rmqtt_things \
    -v pgdata:/var/lib/postgresql/data \
    postgres:18-alpine
```

验证：

```bash
docker exec rmqtt-things-postgres pg_isready -U rmqtt_user
```

输出 `/var/run/postgresql:5432 - accepting connections` 就表示数据库就绪。

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

`--appendonly yes` 开启 AOF 持久化，Redis 重启后数据不会丢。

验证：

```bash
docker exec rmqtt-things-redis redis-cli ping
```

输出 `PONG` 就行。

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

1883 端口映射到宿主机，MQTT 设备通过这个端口连接。RMQTT 的管理 API 端口（6060）不对外暴露，只有 App 容器通过 Docker 网络访问。

验证：

```bash
docker logs rmqtt-things-rmqtt --tail 20
```

看到 `rmqtt-web-hook` 和 `rmqtt-auth-http` 插件加载成功的日志就说明配置生效了。

### App

首次部署需要生成 CA 证书（一次性，之后启动不再生成）：

```bash
docker run --rm \
    -e APP_CONFIG=/app/config.toml \
    -v /opt/rmqtt-things/config.production.toml:/app/config.toml:ro \
    -v /opt/rmqtt-things/conf:/app/conf \
    ghcr.io/<owner>/rmqtt-things:<tag> \
    --generate-ca
```

生成后启动 App：

```bash
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /opt/rmqtt-things/config.production.toml:/app/config.toml:ro \
    -v /opt/rmqtt-things/conf:/app/conf \
    ghcr.io/<owner>/rmqtt-things:<tag>
```

把 `<owner>` 换成你的 GitHub 用户名或组织名，`<tag>` 换成版本号（比如 `v0.1.0`）。`conf` 卷挂载让 CA 证书持久化（App 用它签发设备证书），并与 RMQTT 共享同一份。

App 启动时会自动运行数据库迁移（`sqlx::migrate!`）。你不需要手动建表。但每次部署新版本前建议备份数据库，因为迁移不可逆。

验证：

```bash
docker exec rmqtt-things-app wget -qO- http://localhost:8080/api/health
```

返回 `{"status":"health"}` 表示服务正常。

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

Caddy 首次启动时会向 Let's Encrypt 发起证书申请。如果域名 DNS 还没生效，或者 80 端口被防火墙挡了，证书申请会失败，Caddy 会不断重试。

验证：

```bash
curl -I https://your-domain.com
```

返回 `HTTP/2 200` 就表示部署完成。浏览器打开 `https://your-domain.com` 应该能看到前端页面。

## 验证整体部署

部署完成后，按这个清单检查一遍：

1. 浏览器访问 `https://your-domain.com`，能看到前端界面
2. `curl https://your-domain.com/api/health` 返回 200
3. `docker exec rmqtt-things-redis redis-cli ping` 返回 PONG
4. `docker exec rmqtt-things-postgres pg_isready -U rmqtt_user` 返回 accepting connections
5. 用 MQTT 客户端工具连接服务器 1883 端口，能连上

## CI/CD

项目用 GitHub Actions 做自动构建和推送。流程在 `.github/workflows/cd.yml` 里定义。

触发条件：推送以 `v` 开头的 tag（比如 `git tag v0.1.0 && git push origin v0.1.0`）。

流程做的事：
1. 检出代码
2. 登录 GHCR（GitHub Container Registry）
3. 构建 Docker 镜像（利用 GitHub Actions 缓存加速）
4. 推送到 `ghcr.io/<owner>/rmqtt-things:<tag>`

Dockerfile 用多阶段构建，一共五个阶段：

| 阶段 | 基础镜像 | 做什么 |
|------|---------|--------|
| chef | rust:1.96.1-slim | 安装 cargo-chef |
| planner | chef | 分析依赖图，生成 recipe.json |
| builder | chef | 先编译依赖（缓存层），再编译项目 |
| frontend-builder | node:20-slim | 从 builder 导出 OpenAPI spec，生成前端 API 客户端，构建前端 |
| runtime | debian:bookworm-slim | 只拷贝二进制和前端产物，用非 root 用户运行 |

依赖缓存的设计是关键：只要 `Cargo.toml` 和 `Cargo.lock` 没变，依赖层就会命中缓存，只重新编译业务代码。前端也一样，`package.json` 和 `package-lock.json` 不变就复用 `node_modules`。

运行镜像最终只有二进制文件、前端静态资源和 ca-certificates，体积很小。进程以 `rmqtt` 用户运行，不是 root。

### 发布新版本

1. 打 tag 并推送，触发 GitHub Actions 构建镜像：

```bash
git tag v0.2.1
git push origin v0.2.1
```

2. 等 GitHub Actions 构建完成后，SSH 到生产服务器，设置版本号并执行升级：

```bash
VERSION=0.3.0  # 替换为目标版本

# 拉取新镜像
docker pull ghcr.io/timzaak/rmqtt-things:${VERSION}

# 停止并删除旧容器
docker stop rmqtt-things-app
docker rm rmqtt-things-app

# 用新镜像启动
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /server/conf/rmqtt-thing/config.toml:/app/config.toml:ro \
    ghcr.io/timzaak/rmqtt-things:${VERSION}
```

3. 验证：

```bash
# 检查日志，确认启动成功（应看到 "Listening on port 8080"）
docker logs rmqtt-things-app --tail 10

# 健康检查（容器内没有 curl/wget，通过 caddy 容器访问）
docker exec caddy wget -qO- http://rmqtt-things-app:8080/api/health
# 应返回 {"status":"health",...}
```

### 回滚

如果新版本有问题，用旧版本 tag 重新执行停止和启动步骤：

```bash
docker stop rmqtt-things-app
docker rm rmqtt-things-app
docker run -d \
    --name rmqtt-things-app \
    --network rmqtt-things-net \
    --restart unless-stopped \
    -e APP_CONFIG=/app/config.toml \
    -v /server/conf/rmqtt-thing/config.toml:/app/config.toml:ro \
    ghcr.io/timzaak/rmqtt-things:0.3.0  # 回滚到上一个版本
```

### 升级前备份数据库

App 启动时会自动运行数据库迁移（`sqlx::migrate!`），迁移不可逆。升级前建议备份：

```bash
docker exec rmqtt-things-postgres pg_dump -U rmqtt_user rmqtt_things > backup_$(date +%Y%m%d).sql
```

> 升级过程只有 `rmqtt-things-app` 容器需要替换，其他容器不需要变动。stop 到 start 之间会有几秒服务中断。

## 数据持久化

App 本身无状态。所有持久化数据在这几个地方：

| 数据 | 存储位置 | Volume 或挂载 |
|------|---------|--------------|
| 业务数据（设备、产品、事件等） | PostgreSQL | `pgdata` volume |
| Schema 缓存 | Redis | `redisdata` volume（AOF 持久化） |
| TLS 证书 | Caddy | `caddy-data` volume |
| RMQTT 配置 | 宿主机 | `/opt/rmqtt-things/conf/` 目录挂载 |
| App 配置 | 宿主机 | `/opt/rmqtt-things/config.production.toml` 文件挂载 |
| Caddyfile | 宿主机 | `/opt/rmqtt-things/Caddyfile` 文件挂载 |

备份数据库用 `pg_dump`：

```bash
docker exec rmqtt-things-postgres pg_dump -U rmqtt_user rmqtt_things > backup.sql
```

恢复：

```bash
cat backup.sql | docker exec -i rmqtt-things-postgres psql -U rmqtt_user rmqtt_things
```

Redis 的数据丢了不严重，App 会自动重建缓存。如果确实想备份：

```bash
docker exec rmqtt-things-redis redis-cli BGSAVE
docker cp rmqtt-things-redis:/data/dump.rdb ./redis-backup.rdb
```

## 常见问题

### Caddy 证书申请失败

日志里看到 `acme: error` 之类的信息。检查：
- 域名 DNS 是否指向服务器 IP（`dig your-domain.com` 确认）
- 防火墙是否开放 80 和 443 端口
- 服务器 80 端口是否被其他进程占用（`ss -tlnp | grep :80`）

### App 连不上数据库

App 日志里看到 `connection refused` 或 `no route to host`。

检查容器是否在同一个网络：
```bash
docker network inspect rmqtt-things-net
```

应该能看到 postgres、app 等容器都挂在这个网络上。确认配置文件里的数据库地址是 `rmqtt-things-postgres:5432`，不是 `localhost`。

### RMQTT WebHook 不生效

App 收不到设备上报的数据。先确认 RMQTT 配置里的回调地址已经改成容器名：

```bash
grep "rmqtt-things-app" /opt/rmqtt-things/conf/plugins/rmqtt-web-hook.toml
```

应该能看到 `http://rmqtt-things-app:8080` 开头的 URL。如果还是 `host.docker.internal`，说明之前的 `sed` 替换没生效。

### 设备连上 RMQTT 但 App 不认

RMQTT 的认证插件（`rmqtt-auth-http`）每次设备连接时都会调用 App 的 `/api/access/auth` 接口。检查 App 日志里有没有收到认证请求，以及 `[mqtt.access.auth]` 配置里的 `suffix` 是否和设备端配置的一致。

### App 启动后马上退出

通常是数据库迁移失败。看日志：

```bash
docker logs rmqtt-things-app --tail 100
```

常见原因：数据库密码配置错误，或者 PostgreSQL 还没完全启动。如果用 `postgres:18-alpine`，首次启动初始化数据库要几秒钟。等 `pg_isready` 返回正常后再启动 App。

### 修改 Caddyfile 后生效

```bash
docker exec caddy caddy reload --config /etc/caddy/Caddyfile
```

不需要重启 Caddy 容器。

### Herald 认证不生效

配了 `[herald]` 但管理端 API 还是能无认证访问：

1. 检查配置文件里的 `[herald]` 段是否被注释掉了（生产配置模板里默认是注释的）
2. 确认 `base_url` 在 Docker 网络内可达：`docker exec rmqtt-things-app wget -qO- http://herald:3000`
3. 看 App 日志里有没有 Herald 连接错误

### Herald 连接超时

App 日志里看到 `auth service unavailable` 或 503 错误。检查 Herald 容器是否在同一个 Docker 网络里：

```bash
docker network inspect rmqtt-things-net
```

确认 Herald 容器出现在网络里，并且 `base_url` 用的是容器名而不是 `localhost`。

### 管理后台登录后反复跳转

Herald 登录成功但管理后台一直跳回登录页。通常是 Cookie 没写成功：

- 同域子域名模式：检查 Herald 的 `X-Auth` Cookie domain 是否设为 `.your-domain.com`
- 同主机模式：确认 Herald 和 rmqtt-things 在同一主机，浏览器 Cookie 跨端口共享（DevTools → Application → Cookies 检查 `X-Auth` 是否存在）
