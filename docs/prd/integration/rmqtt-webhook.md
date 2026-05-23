# RMQTT WebHook 集成 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-DV-001]` 设备 HMAC 认证，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备使用 client_id 和 HMAC 签名密码通过 MQTT Broker 认证

- `[US-DV-002]` 设备 ACL 权限控制，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备只能在自己的主题空间内发布和订阅消息

- `[US-DV-003]` 上报属性数据，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备通过 MQTT 上报属性数据

- `[US-DV-004]` 接收属性下发，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备接收属性下发命令并回报处理结果

- `[US-DV-005]` 上报事件，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备上报事件数据

- `[US-DV-006]` 上报当前版本并接收升级，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备上报固件版本，平台检测并推送升级

- `[US-DV-007]` 请求文件上传，优先级 P2，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备向平台请求文件上传凭证

- `[US-DV-008]` 上报连接/断开状态，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备连接/断开时平台自动记录状态

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 4 | 设备认证、ACL 控制、上报属性数据、上报连接/断开状态 |
| P1 | 3 | 接收属性下发、上报事件、上报版本并接收升级 |
| P2 | 1 | 请求文件上传 |

---

## 2. 范围界定

### 2.1 包含功能
- RMQTT WebHook 认证回调：HMAC-SHA1 签名验证
- RMQTT WebHook ACL 回调：设备主题空间隔离
- RMQTT WebHook 消息回调：属性上报、事件上报、属性订阅、属性回复
- RMQTT WebHook 连接回调：设备连接/断开状态记录
- RMQTT WebHook OTA 回调：设备版本上报
- RMQTT WebHook 文件上传回调：设备请求文件上传凭证
- MQTT 消息推送：属性命令推送、OTA 升级推送、文件上传响应、属性/事件 ACK

### 2.2 不包含功能 (Out of Scope)
- RMQTT Broker 配置和管理
- WebHook 回调重试策略
- 设备端 MQTT SDK
- 消息持久化和可靠性保证（由 RMQTT Broker 负责）

### 2.3 依赖项
- RMQTT Broker：作为 MQTT 中间件，配置 WebHook 回调到本服务
- MQTT 主题协议：设备与平台约定的 MQTT 主题格式和数据结构

---

## 3. 需求概述

### 3.1 功能描述

RMQTT WebHook 集成是平台与设备之间的核心桥接层。RMQTT Broker 作为 MQTT 中间件，通过 HTTP WebHook 将设备的认证请求、ACL 检查、消息发布、连接事件等转发到本服务的回调端点。本服务处理这些回调后，将数据持久化到数据库，并通过 MQTT 向设备推送响应消息（如属性命令、OTA 升级、ACK 确认等）。

### 3.2 关键特性
- WebHook 回调端点由 RMQTT Broker 调用，不对外暴露
- 认证采用 HMAC-SHA1，密码格式为 `nonce.timestamp.hash`
- ACL 规则：设备只能访问 `/{client_id}/thing/*` 主题空间
- 消息回调使用 RMQTT 标准消息格式（RMqttPublishMessage），payload 为 Base64 编码
- 连接/断开回调使用 RMQTT 标准连接事件格式
- 设备上线（订阅属性主题）时自动推送待处理的属性命令
- 属性/事件上报支持 ACK 确认机制

---

## 4. 功能需求

### 5.1 核心需求

**认证与 ACL**
1. RMQTT 调用认证回调时，验证 HMAC-SHA1 签名和时间戳有效性（5 分钟窗口）
2. RMQTT 调用 ACL 回调时，检查设备是否只能访问自己 client_id 对应的主题空间

**消息回调**
3. 设备发布属性数据时，RMQTT 转发到属性上报回调，平台解析、校验、存储属性数据
4. 设备发布事件数据时，RMQTT 转发到事件上报回调，平台解析、存储事件数据
5. 设备订阅属性设置主题时，RMQTT 转发订阅事件，平台推送待处理的属性命令
6. 设备回复属性命令执行结果时，RMQTT 转发到属性回复回调，平台更新命令状态

**连接管理**
7. 设备连接成功时，RMQTT 回调连接事件，平台更新设备状态为 Online
8. 设备断开连接时，RMQTT 回调断开事件，平台更新设备状态为 Offline

**OTA**
9. 设备上报版本时，RMQTT 转发到 OTA 回调，平台记录版本并检测升级

**文件上传**
10. 设备请求文件上传时，RMQTT 转发到文件上传回调，平台返回 S3 预签名凭证

### 5.2 验收目标
- 合法 HMAC 签名 + 有效时间戳 → 认证通过
- 非法签名、超时时间戳、错误密码格式 → 认证拒绝
- 设备只能在自己 client_id 的主题空间内操作
- 属性上报后数据可查询（最新值和历史）
- 事件上报后数据可查询
- 设备上线后自动收到待处理的属性命令
- 命令回复后状态正确更新（Pending → Sent → Success/Failed）
- 设备连接/断开后状态正确更新
- 设备版本上报后如有升级可收到推送

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围

- 认证与 ACL 回调：由 RMQTT Broker 调用，验证设备 HMAC 签名和主题空间权限
- Thing 消息回调：由 RMQTT Broker 通过 Publish/Subscribe Hook 转发设备属性、事件、属性订阅、属性回复和文件上传请求
- 设备连接回调：由 RMQTT Broker 通过 Connect/Disconnect Hook 通知设备上下线事件
- OTA 回调：由 RMQTT Broker 通过 Publish Hook 转发设备版本上报
- MQTT 推送接口：向设备推送属性命令、OTA 升级消息、文件上传响应和 ACK 确认
- 回调端点明细和 RMQTT 消息格式详见技术设计文档

### 访问控制原则
- 所有回调端点仅由 RMQTT Broker 内部调用，不对外暴露鉴权
- 生产环境应通过网络策略限制只有 RMQTT Broker 可访问这些端点

### 数据边界
- 设备通过 MQTT topic 中的 `{product_id}/{device_id}` 关联到产品和设备
- MQTT 主题格式约定为 `/{product_id}/{device_id}/thing/{action}`

### 兼容性要求
- HMAC 密码格式 `nonce.timestamp.hash` 为设备端协议约定，变更需协调设备固件更新
- MQTT 主题格式 `/{product_id}/{device_id}/thing/{action}` 为平台与设备的协议约定
- ACL 规则变更影响所有设备的主题访问权限

---

## 6. 前端/交互约束

**适用性**: 不适用
本模块为 RMQTT Broker 与后端服务之间的集成层，无直接前端交互。

---

## 7. 技术设计承接

**适用性**: 不适用
当前功能已实现，技术细节直接体现在代码中。

---

## 8. 相关文件索引

### 9.1 后端文件
- `backend/src/api/auth_handlers.rs` - 认证和 ACL 回调 handlers
- `backend/src/api/handlers.rs` - Thing 和 Device 回调 handlers（属性上报、事件上报、属性订阅、属性回复、设备连接/断开、设备文件上传）
- `backend/src/api/ota_handlers.rs` - OTA 版本上报回调 handler
- `backend/src/api/utils.rs` - 工具函数（属性命令发送、MQTT 消息推送）
- `backend/src/api/web_models.rs` - RMQTT WebHook 消息模型定义
- `backend/src/rmqtt_client.rs` - MQTT 消息推送客户端

### 9.2 前端文件
- 无（本模块为后端集成层）

---

## 9. 参考资料
- 用户故事：`docs/user-stories/02-iot-device-user-stories.md`
- 相关 PRD：`docs/prd/core/product-device-management.md`, `docs/prd/integration/cert-management.md`, `docs/prd/integration/ota-management.md`, `docs/prd/integration/file-upload.md`
- RMQTT WebHook 文档：https://github.com/rmqtt/rmqtt
