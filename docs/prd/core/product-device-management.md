# 产品与设备管理 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-001]` 创建产品，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员创建 IoT 产品，填写名称、型号和描述

- `[US-PA-002]` 查看产品列表，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看和搜索产品列表

- `[US-PA-003]` 编辑产品，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员修改产品名称和描述

- `[US-PA-014]` 查看设备状态列表，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看设备在线/离线状态

- `[US-PA-015]` 查看设备属性历史，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看设备最新属性和属性上报历史

- `[US-PA-016]` 下发属性命令，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员向设备下发属性设置命令

- `[US-PA-017]` 查看设备事件历史，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看设备事件上报历史

- `[US-PA-018]` 查看设备状态变更历史，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看设备连接/断开历史

- `[US-PA-019]` 设备列表页面，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员通过 Web 后台设备列表页查看和筛选设备

- `[US-PA-020]` 设备详情页面，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员在设备详情页查看设备完整信息，包括属性、事件、命令和状态历史

- `[US-DV-003]` 上报属性数据，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备通过 MQTT 上报属性数据

- `[US-DV-004]` 接收属性下发，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备接收属性下发命令并回报处理结果

- `[US-DV-005]` 上报事件，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备上报事件数据

- `[US-DV-008]` 上报连接/断开状态，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备连接/断开时平台自动记录状态

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 6 | 创建产品、查看产品列表、查看设备状态列表、上报属性数据、设备列表页面、设备详情页面 |
| P1 | 6 | 编辑产品、查看属性历史、下发属性命令、查看事件历史、接收属性下发、上报事件 |
| P2 | 1 | 查看设备状态变更历史 |

---

## 2. 范围界定

### 2.1 包含功能
- 产品 CRUD 管理（创建、列表查询、编辑）
- 设备状态实时监控（在线/离线状态、IP 地址）
- 设备属性数据存储（最新属性快照、属性历史记录）
- 设备事件数据存储（事件历史记录）
- 属性命令下发（创建命令、在线设备立即推送、离线设备缓存待推、命令状态追踪）
- 设备连接/断开状态变更历史记录
- 设备属性上报时的可选 Schema 校验

### 2.2 不包含功能 (Out of Scope)
- 设备注册/注销流程（设备通过 MQTT 自动接入）
- 设备分组管理
- 设备批量操作
- 属性数据聚合/统计分析
- 实时数据推送（WebSocket/SSE）
- 设备地理位置管理

### 2.3 依赖项
- RMQTT Broker：作为 MQTT 中间件和 WebHook 回调源
- PostgreSQL：数据存储
- Redis 或内存缓存（用于 Schema 校验缓存，可选）

---

## 3. 需求概述

### 3.1 功能描述
产品与设备管理是 RMQTT Things 平台的核心功能。产品作为设备的逻辑分组，每个产品有唯一的型号编号。设备通过 MQTT 接入后，平台通过 RMQTT WebHook 接收设备的属性上报、事件上报、连接/断开状态变化等回调，将数据持久化到 PostgreSQL 并提供管理 API 和 Web 界面供管理员查看和操作。

属性命令下发允许管理员通过平台向设备发送属性设置指令。系统根据设备当前是否订阅了属性设置主题来决定立即推送或缓存待推。

### 3.2 关键特性
- 产品以型号编号（model_no）为唯一标识
- 设备通过 MQTT client_id 自动关联到产品（从 topic 中提取 product_id）
- 属性数据同时维护最新快照和历史记录
- 属性命令支持 Pending/Sent/Success/Failed/Deleted 五种状态
- 设备上线时自动检查并发送待处理的属性命令
- 属性上报支持可选的 JSON Schema 校验（基于校验模板管理）

---

## 4. 功能需求

### 5.1 核心需求
1. 管理员可通过 Web 后台创建、查看、搜索、编辑产品
2. 产品创建时需提供名称和型号编号，型号编号全局唯一且创建后不可修改
3. 设备通过 MQTT 接入时，平台自动记录连接状态
4. 设备上报属性时，平台存储最新属性快照和属性历史
5. 设备上报事件时，平台存储事件历史
6. 管理员可查看设备的在线/离线状态、最新属性、属性历史、事件历史
7. 管理员可向设备下发属性命令，系统根据设备订阅状态决定即时推送或缓存
8. 设备回报属性命令执行结果后，系统更新命令状态

### 5.2 验收目标
- 产品 CRUD 操作通过前端 Web 页面完成，操作结果实时反映在列表中
- 设备属性上报后，管理员可在管理界面查询到最新属性和历史记录
- 属性命令下发后，在线设备立即收到推送，离线设备在上线后收到推送
- 所有查询支持按 product_id 和 device_id 筛选，支持分页
- 设备列表页支持按产品筛选、按在线/离线状态筛选，支持分页，点击设备可进入详情
- 设备详情页在同一页面内展示设备基本信息、最新属性、属性历史、事件历史、命令历史和状态历史

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围
- 设备端回调接口：接收 RMQTT WebHook 的属性上报、事件上报、属性订阅、属性回复、设备连接/断开回调
- 管理端查询接口：产品 CRUD、设备状态查询、属性/事件历史查询、属性命令 CRUD、状态历史查询

### 访问控制原则
- 设备端回调接口由 RMQTT Broker 调用，不做额外鉴权
- 管理端接口在 Herald 配置时受认证保护，未配置时不做鉴权（单租户部署模式）
- 设备只能访问自己 client_id 对应的主题空间（通过 ACL 控制）

### 数据边界
- 所有数据以 product_id 为一级维度组织
- 设备通过 MQTT topic 中的 product_id 与产品关联
- 属性命令的 product_id、device_id 与设备上报的保持一致

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口
- `/products` - 产品列表页（已实现）
- `/products/create` - 创建产品页（已实现）
- `/products/edit/$id` - 编辑产品页（已实现）
- `/devices` - 设备列表页（已实现）
- `/devices/show/$id` - 设备详情页（已实现）

### 关键交互
- 产品列表支持按名称或型号编号搜索
- 产品编辑页面中，型号编号为只读字段
- 创建和编辑表单提交后跳转到列表页
- 表单未保存时离开页面需提示确认（Unsaved Guard）

### 设备页面交互（已实现）
- 设备列表页支持按产品筛选、按在线/离线状态筛选
- 设备列表页展示 device_id、product_id、状态、IP 地址、最后在线时间，支持分页
- 设备列表页点击 device_id 可跳转到设备详情页
- 设备详情页展示设备基本信息（device_id、product_id、状态、IP、最后在线/离线时间）
- 设备详情页分区展示最新属性、属性上报历史、事件上报历史、属性命令历史、连接状态变更历史
- 各历史区域支持独立分页浏览

---

## 7. 技术设计承接

**适用性**: 不适用
当前功能已实现，技术细节直接体现在代码中。如需扩展（如实时推送、批量操作），建议通过 `/t-design` 产出设计文档。

---

## 8. 相关文件索引

### 9.1 后端文件
- `backend/src/api/product_handlers.rs` - 产品 CRUD handlers（已实现）
- `backend/src/api/handlers.rs` - 设备回调 handlers（属性上报、事件上报、设备连接/断开等）
- `backend/src/api/admin_handlers.rs` - 管理端 handlers（设备状态查询、属性/事件历史、属性命令管理）
- `backend/src/db/product.rs` - 产品数据库操作
- `backend/src/db/database.rs` - 数据库服务入口
- `backend/src/db/models.rs` - 数据模型定义
- `backend/src/api/utils.rs` - 工具函数（属性命令发送等）
- `backend/src/cache.rs` - Schema 校验缓存

### 9.2 前端文件
- `frontend/src/routes/products/index.tsx` - 产品列表页（已实现）
- `frontend/src/routes/products/create.tsx` - 创建产品页（已实现）
- `frontend/src/routes/products/edit.$id.tsx` - 编辑产品页（已实现）
- `frontend/src/routes/devices/index.tsx` - 设备列表页（已实现）
- `frontend/src/routes/devices/show.$id.tsx` - 设备详情页（已实现）
- `frontend/src/hooks/useProducts.ts` - 产品相关 React Query hooks（已实现）
- `frontend/src/hooks/useDevices.ts` - 设备状态和状态历史 hooks（已实现）
- `frontend/src/hooks/useProperties.ts` - 属性最新值、历史和命令 hooks（已实现）
- `frontend/src/hooks/useEvents.ts` - 事件历史 hooks（已实现）

---

## 9. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`, `docs/user-stories/02-iot-device-user-stories.md`
- 相关 PRD：`docs/prd/integration/cert-management.md`, `docs/prd/integration/validation-template.md`
