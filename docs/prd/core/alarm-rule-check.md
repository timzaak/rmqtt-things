# 告警规则引擎增强 产品需求文档 (PRD)

**创建时间**: 2026-06-03
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-038]` 配置持续时间条件，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为规则配置持续时间条件，条件需持续满足指定时间后才触发告警，避免瞬时波动误报

- `[US-PA-039]` 配置告警清除条件，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为规则配置清除条件，设备数据恢复到正常范围时自动清除活跃告警

- `[US-PA-040]` 查看告警生命周期状态，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员在告警记录列表中查看和按状态筛选 Active/Acknowledged/Cleared 告警

- `[US-PA-041]` 手动清除告警，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员手动将活跃或已确认的告警标记为已清除

### 1.2 已有相关故事（V1，引用不重复）

- `US-PA-029~033`：告警规则 CRUD 管理，来源 `docs/user-stories/01-platform-admin-user-stories.md`
- `US-PA-034~035`：告警记录查看与确认，来源 `docs/user-stories/01-platform-admin-user-stories.md`

### 1.3 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 3 | 配置持续时间条件、配置告警清除条件、查看告警生命周期状态 |
| P1 | 1 | 手动清除告警 |

---

## 2. 范围界定

### 2.1 包含功能

- **持续时间条件（Duration Condition）**：规则可配置条件持续满足 N 分钟后才触发告警，避免瞬时波动误报
- **告警清除条件（Clear Condition）**：规则可独立配置清除条件，设备数据恢复正常时自动将活跃告警转为 Cleared 状态
- **告警生命周期状态机**：告警从 Active → Acknowledged → Cleared 三态流转，替代现有 acknowledged 二态
- **手动清除告警**：管理员可将 Active 或 Acknowledged 状态的告警手动标记为 Cleared
- **告警状态筛选**：告警记录列表支持按 Active/Acknowledged/Cleared 状态筛选

### 2.2 不包含功能 (Out of Scope)

- **N/M 计数窗口**（"5 次采样中 3 次超阈值才告警"）— 第二批改进
- **复合条件（AND/OR 嵌套逻辑）** — 第二批改进
- **多级严重度联动**（同一规则多个阈值对应不同严重度）— 第二批改进
- **变化率条件**（"温度每分钟上升超过 5°C"）— 第二批改进
- **告警升级（Escalation）** — 第三批改进
- **告警分组/关联（Correlation）** — 第三批改进
- **维护窗口（Suppression）** — 第三批改进
- **规则模板共享** — 第三批改进

### 2.3 依赖项

- `docs/prd/core/alarm-rule-engine.md` — V1 告警规则引擎，本次在其基础上增强
- `docs/prd/core/product-device-management.md` — 产品和设备数据模型
- `docs/prd/integration/rmqtt-webhook.md` — Webhook 回调流程（规则评估的数据来源）
- `.ai/tech-research/alarm-rule-check.md` — 业界对比与技术改进预研报告

---

## 3. 需求概述

### 3.1 功能描述

基于与 ThingsBoard、JetLinks、AWS CloudWatch 等 IoT 平台的告警规则引擎对比分析，本项目 V1 告警引擎存在三个高价值缺失：条件瞬时触发导致误报、告警无自动恢复机制、缺少完整的生命周期状态管理。

本次增强聚焦三个 P0 改进方向：持续时间条件解决误报问题，清除条件解决告警自动恢复问题，三态生命周期解决告警管理闭环问题。

改进基于 Rust 技术栈（dashmap + tokio + sqlx）实现；规则状态存储默认内存（DashMap），可选 Redis 后端（见 §4.3 / §5 兼容性）。

### 3.2 关键特性

- 持续时间条件以分钟为粒度，默认 0 表示即时触发（保持 V1 行为）
- 清除条件仅对属性阈值触发类型适用，与触发条件独立配置
- 告警三态模型（Active/Acknowledged/Cleared）向后兼容现有 acknowledged 字段
- 手动清除和自动清除（通过清除条件）都可将告警转为 Cleared 状态
- 持续时间窗口的状态存储默认在内存（DashMap），进程重启后重新积累；可选启用 Redis 后端，启用后重启可恢复状态

---

## 4. 业务规则与状态

### 4.1 业务规则

- **持续时间规则**：条件首次满足时记录开始时间，后续每次评估检查是否已持续满足指定时长；中途条件不满足时重置计时；仅对属性阈值触发类型适用
- **清除条件规则**：仅在属性阈值触发类型的规则上可配置；清除条件使用与触发条件相同的操作符集合；每次属性上报时，对所有该产品下活跃告警的清除条件进行评估
- **去重与持续时间的关系**：throttle_minutes 去重窗口从告警实际触发（持续时间满足）后开始计算，而非从条件首次满足时
- **清除条件与持续时间条件不叠加**：清除条件评估时不检查持续时间，满足即清除

### 4.2 告警生命周期状态机

```
Active ──(管理员确认)──→ Acknowledged ──(管理员清除)──→ Cleared
  │                                                      ↑
  └──────────────(管理员清除 / 自动清除条件满足)──────────┘
```

- **Active**：告警刚触发，尚未被任何人处理
- **Acknowledged**：管理员已确认告警，表示已知悉
- **Cleared**：告警已关闭（自动或手动），为终态，不可回退

状态转换规则：
- Active → Acknowledged：管理员确认操作
- Active → Cleared：管理员手动清除，或自动清除条件满足
- Acknowledged → Cleared：管理员手动清除，或自动清除条件满足
- Cleared → *：不可转换（终态）

### 4.3 关键异常

- 进程重启后持续时间窗口和计数窗口状态在默认内存后端下会丢失，正在计时中的规则需要重新积累 — 未启用 Redis 时可接受（Redis 后端可恢复，见下条）
- 规则状态存储支持两种后端：内存（DashMap，默认）和 Redis。启用 Redis 后，进程重启后持续时间窗口和计数窗口状态可通过 Redis 恢复。未启用 Redis 时退化为内存存储，重启后状态丢失。Redis 后端通过 `RedisRuleStateStore` 实现，与默认的 `InMemoryRuleStateStore` 实现相同的 trait 接口
- 告警清除条件评估增加每次属性上报的处理开销，通过按 product_id 分区和活跃告警数量控制

---

## 5. 功能需求

### 5.1 核心需求

1. 管理员可在创建/编辑告警规则时，为属性阈值触发类型配置持续时间（单位：分钟），默认为 0（即时触发）
2. 管理员可在创建/编辑告警规则时，为属性阈值触发类型独立配置清除条件，使用与触发条件相同的操作符
3. 规则引擎在条件评估时，对配置了持续时间的规则追踪首次满足时间，仅在条件持续满足指定时长后才触发告警
4. 规则引擎在每次属性上报评估时，检查活跃告警的清除条件是否满足，满足则自动将告警转为 Cleared 状态
5. 告警记录从二态（new/acknowledged）升级为三态（Active/Acknowledged/Cleared），新增 cleared_at 时间戳
6. 管理员可手动清除 Active 或 Acknowledged 状态的告警
7. 告警记录列表新增状态筛选维度，支持按 Active/Acknowledged/Cleared 筛选
8. 告警记录列表使用颜色区分状态：Active 红色、Acknowledged 黄色、Cleared 绿色

### 5.2 验收目标

- 配置持续时间为 5 分钟的规则，设备属性持续满足条件超过 5 分钟后产生告警记录
- 配置持续时间为 5 分钟的规则，设备属性满足条件仅 3 分钟后恢复，不产生告警记录
- 配置清除条件 temperature < 45 的规则，告警触发后属性回落到 45 以下时告警自动变为 Cleared
- 告警记录列表正确展示 Active/Acknowledged/Cleared 三种状态和对应颜色
- 管理员确认告警后，状态从 Active 变为 Acknowledged
- 管理员手动清除告警后，状态变为 Cleared，记录清除时间
- 现有不配置持续时间和清除条件的规则保持原有行为（向后兼容）

---

## 6. API 相关约束

**适用性**: 必填

### 接口能力范围

- 规则管理接口：在现有规则 CRUD 基础上扩展 duration_minutes 和 clear_condition 字段
- 告警记录接口：在现有告警查询基础上增加 status 筛选维度和清除操作
- 规则评估为内部逻辑，不对外暴露独立接口

### 访问控制原则

- 规则管理和告警操作接口为管理端 API，需登录认证
- 规则评估为内部逻辑，由系统自动触发，无需额外鉴权

### 数据边界

- 规则以 product_id 为维度绑定，管理员只能操作自己权限范围内的产品规则
- 告警记录按 product_id 组织

### 兼容性要求

- 现有不带 duration_minutes 和 clear_condition 的规则保持即时触发行为
- 现有 acknowledged 布尔字段平滑迁移到 status 状态机：`acknowledged=true` 等价于 `status ∈ {acknowledged, cleared}`（即非 active），`acknowledged=false` 等价于 `status=active`；前端如需精确区分状态应读取 `status` 字段
- 清除条件和持续时间条件仅对属性阈值触发类型的规则生效，其他触发类型忽略

---

## 7. 前端/交互约束

**适用性**: 必填

### 页面入口

- `/alarm-rules/create` — 创建告警规则页（扩展持续时间条件和清除条件配置）
- `/alarm-rules/edit/$id` — 编辑告警规则页（扩展持续时间条件和清除条件配置）
- `/alarms` — 告警记录列表页（展示三态状态，增加状态筛选和手动清除操作）

### 关键交互

- 创建/编辑规则页：当触发类型为"属性阈值"时，展示持续时间输入框（单位：分钟，默认 0）和清除条件配置区域
- 创建/编辑规则页：当触发类型为"事件"或"设备状态"时，不展示持续时间和清除条件配置
- 告警记录列表页：状态列使用颜色标签区分（Active 红色、Acknowledged 黄色、Cleared 绿色）
- 告警记录列表页：新增状态下拉筛选器（All/Active/Acknowledged/Cleared）
- 告警记录列表页：Active 和 Acknowledged 状态的告警行展示"清除"操作按钮，Cleared 状态不展示

### 状态反馈

- 规则编辑后，持续时间和清除条件立即生效
- 告警状态变更后（确认/清除），列表即时更新状态标签和可用操作
- 自动清除的告警在列表中展示清除时间

---

## 8. 已确认决策

- **改进分批实施**：本次仅实施 P0 三项（持续时间条件、清除条件、三态生命周期），P1/P2 能力列入后续批次
- **持续时间粒度为分钟**：以分钟为单位，最小值为 0（即时触发，即 V1 默认行为）
- **清除条件仅适用于属性阈值触发类型**：事件和设备状态触发类型不配置清除条件
- **告警状态为终态模型**：Cleared 为终态，不可回退到 Active 或 Acknowledged
- **状态存储双后端**：规则状态（持续时间窗口、计数窗口）默认内存（DashMap），可选 Redis 后端；启用 Redis 后进程重启可恢复状态，未启用时退化为内存（重启后状态丢失，可接受）
- **可选引入 redis crate**：Redis 后端通过 `RedisRuleStateStore` 实现，与默认 `InMemoryRuleStateStore` 实现相同 trait 接口；未启用 Redis 时不依赖该 crate

---

## 9. 参考资料

- 技术预研报告：`.ai/tech-research/alarm-rule-check.md`
- V1 告警引擎 PRD：`docs/prd/core/alarm-rule-engine.md`
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（故事 38-41）
- 相关 PRD：`docs/prd/core/product-device-management.md`
- 相关 PRD：`docs/prd/integration/rmqtt-webhook.md`
- 业界参考：ThingsBoard Alarm Rules、AWS CloudWatch Alarm Evaluation
