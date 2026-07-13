[English](README.md) | [中文](README_zh.md)

# RMQTT Things

基于 [RMQTT](https://github.com/rmqtt/rmqtt) 的物联网物模型管理平台，全程使用 AI 开发，内置完整 skill 配置，支持 AI 驱动二次开发。

**在线演示：** https://mqtt.fornetcode.com

演示登录账号（Herald）：

| 字段 | 值 |
|---|---|
| 邮箱 | `admin@rmqtt-things.local` |
| 密码 | `password` |
| Realm | `rmqtt` |

<!-- TODO: 添加管理界面截图 -->

## 为什么关注这个项目

这个项目不是又一个 IoT 平台。它的重点是：**展示如何用 AI 完整开发一个生产级项目，并且让后续迭代也能用 AI 完成。**

项目内置了一套 skill 系统（`.claude/` 目录），把需求、设计、编码、测试串成完整流水线。配合 Claude Code 或 AidCode + 国产大模型，clone 下来就能用 AI 做二开。你只需要描述需求，AI 帮你走完剩下的流程。

Skill 配置也是独立的：[web-dev-skills](https://github.com/timzaak/web-dev-skills)，可以套到其他 Rust + React 项目上。

选 Rust + React 是有考量的：编译器和类型系统是 AI 编码最好的质检员，OpenAPI-to-TypeScript 代码生成保持前后端 API 同步。

## 功能

设备走 MQTT 上报数据，Rust 后端接 WebHook 写 PostgreSQL，React 前端做管理界面。

- 设备生命周期管理：上下线跟踪、属性上报、事件历史
- 命令下发与 OTA 固件升级
- TLS 证书签发（内置 CA）

技术栈：Rust / Axum / SQLx / PostgreSQL / React 19 / TanStack

## 快速开始

前置条件：Docker、Rust 工具链、Node.js。完整步骤见[入门指南](docs/tutorials/getting-started.md)。

```shell
docker run postgres:18-alpine
docker run rmqtt/rmqtt:0.21.0
cd backend && cargo run
cd frontend && npm install && npm run dev
```

## 文档

| 文档 | 说明 |
|---|---|
| [入门指南](docs/tutorials/getting-started.md) | 完整安装配置 |
| [用 AI 二次开发](docs/tutorials/ai-development.md) | Skill 流水线与操作步骤 |
| [架构说明](docs/tutorials/architecture.md) | 系统设计与技术决策 |
| [物模型规范](docs/tutorials/thing-model-spec.md) | MQTT topic 格式与消息 schema |
| [API 参考](docs/tutorials/api-reference.md) | 管理端与 WebHook API 文档 |
| [部署指南](docs/tutorials/deployment.md) | 生产环境部署 |

## 许可证

[MIT](LICENSE-MIT) / [Apache 2.0](LICENSE-APACHE)
