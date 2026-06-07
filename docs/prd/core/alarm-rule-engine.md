# 告警规则引擎 产品需求文档 (PRD)

**创建时间**: 2026-05-19
**优先级**: P0
**版本说明**: V1 基础版本。告警状态模型已由 `alarm-rule-check.md` 升级为三态（Active/Acknowledged/Cleared），持续时间条件和清除条件参见该文档。

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-029]` 创建告警规则，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为产品创建告警规则，定义触发条件、判断逻辑和执行动作

- `[US-PA-030]` 查看告警规则列表，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看和筛选告警规则列表

- `[US-PA-031]` 编辑告警规则，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员修改规则的条件、动作和去重配置

- `[US-PA-032]` 启用/禁用告警规则，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员暂停或恢复规则触发

- `[US-PA-033]` 删除告警规则，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员删除不再需要的规则

- `[US-PA-034]` 查看告警记录，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看告警触发的历史记录，支持按产品、设备、级别和确认状态筛选

- `[US-PA-035]` 确认告警，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员确认已处理的告警，标记已知悉

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 3 | 创建告警规则、查看告警规则列表、查看告警记录 |
| P1 | 3 | 编辑告警规则、启用/禁用告警规则、确认告警 |
| P2 | 1 | 删除告警规则 |

---

## 2. 范围界定

### 2.1 包含功能

- **规则管理 CRUD**：创建、查看列表、编辑、启用/禁用、删除告警规则
- **属性阈值触发**：设备上报属性值满足条件时触发（如 temperature > 50），对应 API 枚举值 `property`
- **事件触发**：设备上报特定事件时触发，对应 API 枚举值 `event`
- **设备状态触发**：设备上线（`device_online`）或离线（`device_offline`）时触发
- **单条件判断**：支持 >, >=, <, <=, ==, !=, between, contains, always 操作符
- **动作执行**：触发后创建告警记录、发送 Webhook 回调
- **告警去重**：同一规则对同一设备在设定时间窗口内不重复触发
- **告警记录管理**：查看告警历史、按多维度筛选、确认告警
- **规则按产品维度配置**：每条规则绑定一个产品

### 2.2 不包含功能 (Out of Scope)

- 多条件组合（AND/OR 嵌套逻辑）
- 按设备粒度配置规则（仅支持产品维度）
- 告警通知推送（邮件/短信/钉钉/企微等），依赖后续通知系统
- 告警统计/聚合看板
- 告警升级和流转工作流
- 规则版本管理和审计日志
- MQTT 命令下发作为动作（V2 考虑）

### 2.3 依赖项

- `docs/prd/core/product-device-management.md` — 产品和设备数据模型
- `docs/prd/integration/rmqtt-webhook.md` — Webhook 回调（属性上报、事件上报、设备连接/断开是规则触发的数据来源）
- `docs/prd/integration/validation-template.md` — 参考其 CRUD 管理模式
- 后续通知系统（`.ai/future/todo_list.md` P0 #2）— 告警动作中的通知推送由通知系统负责

---

## 3. 需求概述

### 3.1 功能描述

告警规则引擎为 RMQTT Things 平台提供自动化监控能力。管理员为产品定义告警规则后，当设备上报的属性、事件或状态变化满足规则条件时，系统自动创建告警记录并执行预配置的动作（如 Webhook 回调）。这解决了当前平台"无法实现温度超限触发告警等场景"的核心缺失。

规则引擎在数据存储流程中异步执行评估，不影响属性/事件上报的正常响应时间。通过去重机制防止高频数据导致告警风暴。

### 3.2 关键特性

- 规则以产品为维度绑定，同一产品下可配置多条规则
- 支持四种触发类型：属性阈值（`property`）、事件匹配（`event`）、设备上线（`device_online`）、设备离线（`device_offline`）
- 规则评估异步执行，不阻塞数据存储主路径
- 告警去重基于"规则 + 设备"维度，在 throttle_minutes 时间窗口内不重复触发
- 告警记录持久化存储，支持确认操作标记已处理

---

## 4. 功能需求

### 5.1 核心需求

1. 管理员可通过 Web 后台为产品创建告警规则，指定触发类型、条件和动作
2. 规则支持属性阈值（`property`）、事件匹配（`event`）、设备上线（`device_online`）、设备离线（`device_offline`）四种触发类型
3. 条件支持数值比较（>, >=, <, <=, ==, !=）、区间判断（between，闭区间 [min, max]）、包含判断（contains）和无条件触发（always）
4. 触发后执行预配置的动作：创建告警记录（必选）、发送 Webhook 回调（可选）
5. 告警记录包含告警级别（info/warning/critical）、触发时的数据值和可配置的消息模板。每条告警记录还包含以下字段用于 Webhook 重试和触发类型追踪：
   - `trigger_type: String` — 触发类型，记录触发告警的具体类型（property / event / device_online / device_offline）
   - `webhook_retries_left: i16` — Webhook 剩余重试次数，初始值为配置的最大重试次数，每次重试减 1，耗尽后标记为最终失败
   - `webhook_next_retry_at: Option<OffsetDateTime>` — 下次 Webhook 重试时间，由后台 `webhook_retry_task` 调度使用
6. 去重机制：同一规则对同一设备在 throttle_minutes 时间窗口内不重复触发
7. 规则支持启用/禁用切换，禁用后不参与评估
8. 规则评估异步执行，不阻塞属性/事件存储主路径
9. 管理员可查看告警记录列表，按产品、设备、级别和确认状态筛选
10. 管理员可确认告警，标记已处理

### 5.2 验收目标

- 属性上报后，满足阈值条件的规则在 2 秒内产生告警记录
- 事件上报后，匹配事件标识的规则产生告警记录
- 设备上线/离线后，对应规则产生告警记录
- 去重机制生效：同一规则 + 同一设备在 throttle_minutes 内只产生一条告警
- 规则禁用后，设备数据不再触发该规则
- 告警记录列表按时间倒序展示，支持按产品、设备、级别和确认状态筛选
- 规则评估不增加属性/事件上报接口的响应延迟

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围

- 规则管理接口：告警规则的 CRUD 操作、启用/禁用切换
- 告警记录查询接口：告警记录列表查询、告警确认操作
- 内部评估接口：规则引擎作为内部模块被 webhook 回调流程调用，不对外暴露独立接口

### 访问控制原则

- 规则管理和告警记录查询接口为管理端 API，需登录认证
- 规则评估为内部逻辑，由系统自动触发，无需额外鉴权

### 数据边界

- 规则以 product_id 为维度绑定，管理员只能查看和管理自己权限范围内的产品规则
- 告警记录按 product_id 组织，查询时按产品维度筛选

### 与通知系统的边界

- 规则引擎只负责触发条件判断和告警记录创建
- Webhook 动作中的通知推送（邮件/短信等）由后续通知系统负责
- 当前 Webhook 动作仅支持 HTTP 回调，不包含通知推送能力

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口

- `/alarm-rules` — 告警规则列表页
- `/alarm-rules/create` — 创建告警规则页
- `/alarm-rules/edit/$id` — 编辑告警规则页
- `/alarms` — 告警记录列表页

### 关键交互

- 规则列表页展示规则名称、所属产品、触发类型、启用状态、创建时间，支持按产品筛选
- 创建/编辑页根据触发类型动态展示对应的条件配置区域
- 属性阈值触发时，展示属性名、操作符选择和阈值输入
- 事件触发时，展示事件标识输入
- 设备状态触发时，无需额外条件配置（always）
- 规则列表页支持快捷启用/禁用切换
- 告警记录列表页支持按产品、设备、级别（info/warning/critical）和确认状态筛选
- 告警记录列表支持快捷确认操作

### 状态反馈

- 规则创建成功后跳转到规则列表
- 规则启用/禁用操作即时反映在列表中
- 告警确认后，该记录的确认状态即时更新

---

## 7. 技术设计承接

**适用性**: 必填
本功能涉及数据库新增表、规则评估引擎和现有 webhook 回调流程修改，需通过 `/t-design` 产出详细技术设计文档，至少覆盖：

- 数据库表结构（alarm_rule、alarm 表）和迁移方案
- 规则评估引擎模块设计（条件评估、动作执行、缓存策略）
- 与现有 webhook 回调流程的集成点（property_post / event_post / device_connect / device_disconnect）
- 去重机制的实现方案（缓存策略、key 设计）
- Webhook 动作执行器的错误处理和重试策略

技术预研报告：`.ai/tech-research/alarm-rule-engine.md`

---

## 8. 已确认决策与待确认假设

### 9.1 已确认决策

- **通知推送不在 V1 范围**：告警通知推送（邮件/短信/钉钉/企微等）依赖后续通知系统，当前 Webhook 动作仅支持 HTTP 回调
- **V1 仅支持单条件判断**：不支持多条件组合（AND/OR 嵌套逻辑），每条规则包含一个触发类型和一个条件
- **告警确认在 V1 范围内**：管理员可确认已处理的告警记录（US-PA-035），支持按确认状态筛选
- **规则仅按产品维度配置**：不支持按设备粒度配置规则，每条规则绑定一个产品
- **告警统计看板不在 V1 范围**：不做告警统计/聚合看板
- **MQTT 命令下发作为动作推迟到 V2**：当前动作仅包含创建告警记录和 Webhook 回调

### 9.2 已确认决策

- **Webhook 动作执行失败策略**：已确认采用自动重试机制。最大重试次数和重试间隔可配置。重试通过后台任务 `webhook_retry_task` 执行，基于 `alarm_records.webhook_retries_left` 和 `webhook_next_retry_at` 字段调度。重试耗尽后标记为最终失败。

---

## 9. 相关文件索引

### 10.1 后端文件（已实现）

- `backend/src/api/handlers.rs` — 已修改 — webhook 回调中集成规则评估调用
- `backend/src/api/mod.rs` — 已修改 — 注册告警路由，AppState 包含规则引擎组件
- `backend/src/cache.rs` — 参考复用 — SchemaCache 缓存模式
- `backend/src/rmqtt_client.rs` — 参考复用 — MQTT 命令下发通道
- `backend/src/db/database.rs` — 已修改 — 规则和告警的 DB 操作
- `backend/src/rule_engine/` — 已实现 — 规则引擎模块（evaluator、actions、cache）
- `backend/src/api/alarm_handlers.rs` — 已实现 — 规则和告警管理 Admin API
- `backend/migrations/` — 已实现 — alarm_rule 和 alarm 表迁移文件

### 10.2 前端文件（已实现）

- `frontend/src/routes/alarm-rules/` — 已实现 — 规则管理页面（index、create、edit）
- `frontend/src/routes/alarms/` — 已实现 — 告警记录查询页面

---

## 10. 参考资料

- 技术预研报告：`.ai/tech-research/alarm-rule-engine.md`
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（故事 29-35）
- 相关 PRD：`docs/prd/core/product-device-management.md`
- 相关 PRD：`docs/prd/integration/rmqtt-webhook.md`
- 相关 PRD：`docs/prd/integration/validation-template.md`
