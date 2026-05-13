# RMQTT Things

RMQTT Things 把 MQTT 设备接到管理后台。设备通过 RMQTT Broker 上报属性和事件，后端收到 WebHook 回调后写入 PostgreSQL，前端提供设备管理、OTA 升级、证书签发、属性下发的操作界面。

## 给谁看

需要部署或二次开发这套系统的后端工程师。假设你有 Rust 和 React 的项目经验。如果你只是想把它跑起来用，看完快速上手就够了。

## 前置知识

- Rust 基础，能读懂 Axum handler 和 SQLx 查询
- PostgreSQL 和 Redis 的基本操作
- MQTT 协议的概念（topic、QoS、retain）
- Docker 和 docker compose 的日常使用

## 章节

- [快速上手](getting-started.md) — 从零到跑起来
- [物模型协议规范](thing-model-spec.md) — MQTT Topic、消息格式、认证、OTA 等设备通信协议
- [架构](architecture.md) — 目录结构、技术选型、核心数据流
- [API 参考](api-reference.md) — 所有 HTTP 接口
- [连接第一个设备](device-integration.md) — 用 mosquitto 走完属性上报和命令接收
- [设备端开发参考](device-guide.md) — 连接参数、密码生成、主题速查表
- [配置](configuration.md) — 配置项说明
- [认证与权限](auth.md) — 管理端 Herald SSO 认证和设备端 HMAC 认证
- [部署](deployment.md) — 生产环境部署步骤
- [用 Claude Code 二次开发](ai-development.md) — AI 开发流水线的具体操作步骤
