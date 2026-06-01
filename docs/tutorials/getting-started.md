# 快速上手

## 你需要先装好

- Rust 最新稳定版（用 [rustup](https://rustup.rs/) 安装）
- Node.js 20 或更高版本
- Docker（用来跑 PostgreSQL 和 RMQTT Broker）
- Git


## 启动 PostgreSQL

```bash
docker run --rm --name postgres \
  -e POSTGRES_DB=rmqtt_things \
  -e POSTGRES_USER=rmqtt_user \
  -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 \
  postgres:18-alpine \
  postgres -c log_statement=all -c log_destination=stderr
```

这条命令启动一个 PostgreSQL 18 容器，数据库名 `rmqtt_things`，用户名 `rmqtt_user`，密码 `rmqtt_pass`，映射到本机 5432 端口。`--rm` 表示容器停了就删除，数据不持久化，开发环境够用了。


## 启动 RMQTT Broker

```bash
docker run --rm --name rmqtt \
  -p 1883:1883 \
  -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf \
  rmqtt/rmqtt:0.21.0 \
  -f conf/rmqtt.toml
```

在项目根目录下跑这条命令。它把项目里的 `conf/` 目录挂载到容器内，RMQTT 会加载里面的配置，包括 WebHook 插件。

两个端口的作用：
- 1883：MQTT 协议端口，设备连这个
- 6060：HTTP API 端口，后端通过这个给 Broker 发消息

WebHook 配置在 `conf/plugins/rmqtt-web-hook.toml`，已经配好了把设备连接/断开/属性上报等事件转发到 `http://host.docker.internal:8080`，也就是你的后端服务。

## 配置后端

```bash
cd backend
cp config.example.toml config.toml
```

然后打开 `config.toml` 看一眼。默认配置开箱即用，以下几项按需改：

| 配置项 | 默认值 | 什么时候要改 |
|--------|--------|-------------|
| `database.url` | `postgres://rmqtt_user:rmqtt_pass@localhost:5432/rmqtt_things` | 如果你改了 PostgreSQL 的用户名或密码 |
| `mqtt.url` | `http://127.0.0.1:6060/api/v1` | 如果你把 RMQTT 的 HTTP 端口改了 |
| `api.openapi_enabled` | `true` | 关掉可以隐藏 Swagger UI |
| `s3.*` | MinIO 默认配置 | 文件上传功能依赖对象存储，本地开发可以先不管 |
| `ca.*` | 自签名 CA | 证书颁发功能用的，本地开发可以不改 |

如果你 PostgreSQL 和 RMQTT 都是按上面的命令启动的，那 `database.url` 和 `mqtt.url` 都不用动，直接能用。

## 启动后端

```bash
cd backend
cargo run
```

第一次编译要几分钟，后面改了代码再跑会快很多。后端启动后监听 8080 端口。

看到类似这样的日志就说明启动成功：

```
Listening on 0.0.0.0:8080
```

后端会自动执行数据库迁移，建好需要的表。你不用手动建表。

## 启动前端

再开一个终端：

```bash
cd frontend
npm install
npm run dev
```

前端启动后监听 3000 端口。打开 http://localhost:3000 能看到管理界面。

`npm run dev` 启动的是 Vite 开发服务器，支持热更新，改了代码浏览器会自动刷新。

## 验证服务跑起来了

后端和前端都启动后，按顺序检查：

1. 打开 http://localhost:8080/swagger，能看到 Swagger 文档页面，说明后端 API 正常。

2. 打开 http://localhost:3000，能看到管理界面，说明前端正常。

3. 在 Swagger 页面里调一个接口，比如查设备状态（GET `/api/admin/device/status`），返回 200 空列表而不是 500 错误，说明数据库连接正常。

到这一步，整个开发环境就跑起来了。
