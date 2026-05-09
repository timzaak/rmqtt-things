[English](README.md) | [中文](README_zh.md)

# RMQTT Things

基于 [RMQTT](https://github.com/rmqtt/rmqtt) 的物联网物模型管理平台。设备通过 MQTT 上报属性和事件，Rust 后端写入 PostgreSQL 并提供管理 API，React 前端提供管理界面。生产可用。

内置 Claude Code 技能，覆盖完整开发流程：需求、技术设计、任务规划、编码实现、测试验证。技术栈针对 AI 编程优化：Rust 编译器捕获动态语言容易遗漏的错误，OpenAPI-to-TypeScript 代码生成保持前后端 API 同步。

## 功能

后端 (Rust / Axum / SQLx / PostgreSQL):
- 设备生命周期：上下线跟踪、属性上报、事件历史
- 命令下发：向设备发送指令并跟踪状态
- OTA 固件升级与版本管理
- TLS 证书签发（内置 CA，基于 rcgen）
- S3 文件上传（presigned URL）
- Swagger UI: `/api/swagger-ui`

前端 (React 19 / TanStack Router + Query / Tailwind / Radix UI):
- 设备管理与监控
- 证书签发与生命周期管理
- OTA 调度与部署
- 产品配置
- 属性与事件查看器

MQTT 集成:
- 基于 RMQTT WebHook 的事件路由
- HMAC-SHA1 设备认证
- 设备级 Topic ACL

测试:
- Playwright E2E 测试覆盖设备注册、证书签发、OTA、属性命令

## AI 开发流水线

内置 Claude Code 技能将 PRD、设计、编码、测试串联为完整流水线，阶段间设有质量关卡。详见 [用 Claude Code 二次开发](docs/tutorials/ai-development.md)。

## 快速开始

前置条件：Docker、Rust 工具链、Node.js。

```shell
# PostgreSQL
docker run --rm --name=postgres \
  -e POSTGRES_DB=rmqtt_things -e POSTGRES_USER=rmqtt_user -e POSTGRES_PASSWORD=rmqtt_pass \
  -p 5432:5432 postgres:18-alpine \
  postgres -c log_statement=all -c log_destination=stderr

# RMQTT broker
docker run --rm --name rmqtt -p 1883:1883 -p 6060:6060 \
  -v ${PWD}/conf:/app/rmqtt/conf rmqtt/rmqtt:0.20.0 -f conf/rmqtt.toml

# 后端
cd backend && cp config.example.toml config.toml && cargo run

# 前端
cd frontend && npm install && npm run dev
```

Swagger UI: http://localhost:8080/api/swagger-ui

## 项目结构

```
backend/         Rust 后端 (Axum + SQLx + PostgreSQL)
frontend/        React SPA (TanStack Router/Query + Tailwind + Radix UI)
demo/            Playwright E2E 测试
conf/            RMQTT broker 配置和插件规则
docs/            教程、API 参考、架构说明
.claude/         技能、代理、指南、协议
```

## 文档

| 文档 | 说明 |
|---|---|
| [入门指南](docs/tutorials/getting-started.md) | 完整安装配置 |
| [物模型规范](docs/tutorials/thing-model-spec.md) | MQTT topic 格式与消息 schema |
| [API 参考](docs/tutorials/api-reference.md) | 管理端与 WebHook API 文档 |
| [架构说明](docs/tutorials/architecture.md) | 系统设计与技术决策 |
| [部署指南](docs/tutorials/deployment.md) | 生产环境部署 |
| [用 Claude Code 二次开发](docs/tutorials/ai-development.md) | AI 开发流水线与操作步骤 |

## 二次开发

完整可用的产品。Fork 后可自定义 payload 格式、RPC 确认、Topic ACL、设备认证。AI 驱动开发请配合 Claude Code 使用上述技能。

RMQTT 插件配置: [conf/plugins](conf/plugins)。生产环境: `rmqtt.toml` 中设置 `allow_anonymous = false`。

## 许可证

[MIT](LICENSE-MIT) / [Apache 2.0](LICENSE-APACHE)
