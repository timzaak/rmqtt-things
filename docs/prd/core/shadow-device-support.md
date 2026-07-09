# 影子设备支持 产品需求文档 (PRD)

**创建时间**: 2026-07-09
**优先级**: P1

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-042]` 设置设备期望状态，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：为设备设置持久期望属性（desired），作为「这台设备应该是什么状态」的单一权威视图

- `[US-PA-043]` 查看设备期望状态与差异，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：查看 desired / reported / 逐属性 delta，判断设备是否收敛到期望

- `[US-PA-044]` 在前端管理设备期望状态，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：在设备管理后台界面查看与设置期望状态及差异

复用既有故事（本 feature 设备端零改动）：

- `[US-PA-016]` 下发属性命令，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：一次性属性命令通道；desired delta 借用此通道投递，但 desired 与命令语义分离（见 §4.1）

- `[US-DV-004]` 接收属性下发，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备订阅属性设置主题，接收 Pending 命令并回报结果——desired delta 复用此链路

- `[US-DV-009]` 离线命令排队与上线投递，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：离线时命令排队、上线后投递——desired delta 离线收敛复用此能力

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P1 | 4 | 设置期望状态、查看期望与差异、下发属性命令（复用）、接收/上线投递属性下发（复用） |
| P2 | 1 | 前端管理期望状态 |

---

## 2. 范围界定

### 2.1 包含功能

- **desired 持久状态视图**：按 `(product_id, device_id)` 维度保存设备期望属性文档，作为「设备应该是什么状态」的单一权威视图。
- **设置期望状态（Set-Desired）**：管理员为设备设置期望属性（非 null 覆盖）；patch 中 `null` 删除对应 desired 属性。仅此入口可写 desired。
- **查看期望与差异（Get-Delta）**：管理员查看 desired、reported 快照及逐属性 delta。
- **delta 收敛推送**：Set-Desired 时计算 delta，非空则借现有属性命令通道尝试投递（在线即时 / 离线排队 / 上线投递），设备端零改动。
- **前端期望状态管理**：在设备管理后台界面查看与设置期望状态及差异。

### 2.2 不包含功能 (Out of Scope)

- 完整 AWS 式 Shadow 协议（shadow topic / version 并发 / 设备端 SDK）。注：此处排除的是**完整协议机制**（含 topic、version、设备 SDK），**不**包括 R5 的「patch 中 null=删除」合并规则——后者属于基础 patch 语义，已纳入范围（见 R5）。
- 设备主动拉取/订阅 desired 的新协议层——平台推送，设备端零改动。
- **自动重推 / 自动控制器**：设备上报偏离 desired 时，平台**不**自动重新下发；desired 是「持久意图视图」而非自动控制器（被动收敛）。
- desired 与 reported 差异告警。
- 独立的 desired 删除端点（DELETE）；属性级删除通过 Set-Desired patch 中 `null` 实现（见 R5），整体清空通过全 null patch。
- desired 变更历史 / 版本号 / 审计。
- 多设备批量设置 desired（product 级批量）；仅单设备维度。

### 2.3 依赖项

- RMQTT WebHook：设备属性上报、属性订阅、属性回复回调（reported 来源 + delta 投递/回报闭环）。
- PostgreSQL：desired 持久存储 + reported 快照存储。
- 既有属性命令通道（Pending / Sent / Success / Failed / Deleted 状态机、在线推送、离线排队、上线投递、ack 回写）：desired delta 借用此通道，**不修改其语义**。

---

## 3. 需求概述

### 3.1 功能描述

当前管理员无法随时查询设备的「期望状态」，也无法判断「我下发的设置是否已生效」。现有属性命令是一次性动作记录，执行后即终态，不构成持续状态视图。

影子设备支持为 Platform Admin 提供一份**持久 desired 状态视图**：管理员可为设备设置期望属性值，随时查询「设备应该是什么状态」，并计算 desired 与 reported（实际上报）之间的 delta；Set-Desired 时系统借现有属性命令通道尝试把 delta 推送给设备以收敛。设备端零改动。

核心产品语义：

- **desired = 持久意图，命令 = 一次性动作**。desired 是「这台设备应该是什么状态」的持续视图；一次性命令只是临时动作。
- **被动收敛**：平台不在每次设备上报偏离时自动重推；desired 与 reported 的持续不一致是「可观测的预期偏差」，由管理员判断是否再次 Set-Desired。
- **desired 写入收口**：只有 Set-Desired 入口写 desired；一次性命令和设备上报都不写 desired。

### 3.2 关键特性

- desired 文档按 `(product_id, device_id)` 单维度，对称于现有 reported 快照，支持部分属性更新（非 null 覆盖、null 删除）。
- delta 以 desired 为基准、与 reported 逐属性比对：reported 缺失或上报值 ≠ 期望值即为待收敛。
- delta 借用属性命令通道投递，天然获得离线队列、上线投递与 ack 闭环，设备端零改动。
- 管理员可在前端设备管理界面查看 desired/reported/delta 并设置期望状态。

---

## 4. 业务规则与状态

### 4.1 业务规则

- **R1 desired 写入唯一性**：desired 文档仅由 Set-Desired 动作写入；一次性属性命令（US-PA-016）与设备 reported 上报都不写 desired。一次性命令的临时值不污染 desired 视图（desired 优先）。
- **R2 被动收敛**：设备上报偏离 desired 时，平台不自动重推。仅当管理员显式 Set-Desired 时，系统计算当前 delta 并尝试一次投递。desired 与 reported 的持续不一致是预期偏差，不是重推信号。
- **R3 delta 收敛路径**：Set-Desired 计算 delta，delta 非空则作为一条属性命令投递（在线即时推送 / 离线排队 / 上线投递），设备端零改动。delta 为空时不投递。
- **R4 desired 优先与命令并存**：一次性命令通道（临时改值）与持久 desired 并存且互不引用。若一次性命令与 desired delta 同时在投递队列且含相同属性，最终下发值由队列合并行为决定——**同属性冲突的最终值不保证严格「最后写入优先」**（见 §4.2）。
- **R5 set/overwrite + null=删除**：Set-Desired 支持设置/覆盖期望属性；patch 中值为 `null` 的属性表示**删除该 desired 属性**（与 AWS IoT Shadow / Azure IoT Hub Twin 一致的 null-delete 语义）。即 Set-Desired 的 patch 遵循「非 null 覆盖、null 删除」的合并规则。整体清空 desired 可用全属性为 null 的 patch 实现，无独立 DELETE 端点。
- **R6 无版本/无审计**：desired 无版本号、无变更历史。并发 Set-Desired 以最后写入为准（无 AWS Shadow 式 version 并发控制）。

### 4.2 关键状态与异常

- **delta 收敛状态（可观测，非新状态机）**：通过 delta 视图呈现——desired 有但 reported 缺失、或 reported 值 ≠ desired 值，即为「待收敛」。reported 收敛到期望后该属性从 delta 中消失。
- **设备回报 Failed**：delta 对应命令回报失败时，desired 文档**保持原期望值不变**，delta 视图仍显示待收敛；由管理员决定是否再次 Set-Desired 重试（被动收敛）。
- **同属性并发合并顺序未保证**：一次性命令与 desired delta 同属性冲突时，最终下发值不保证严格 last-write-wins（既有命令通道未在数据层保证 Pending 返回顺序）。本 PRD 接受该行为，不要求新增设备协议。
- **权限可见性**：管理端 desired 写入/查询接口与现有属性命令接口遵循同一访问控制原则（Herald 配置时受认证保护，未配置时单租户部署不做额外鉴权）；设备端零改动，设备仍只访问自身 client_id 主题空间。

---

## 5. 功能需求

### 5.1 核心需求

1. 管理员可为指定设备设置持久期望属性（desired），按部分属性更新（仅覆盖提交的字段）。
2. 管理员可查询设备的当前期望状态、实际上报状态（reported 快照），以及两者逐属性的 delta。
3. 设置期望状态时，系统计算 delta；delta 非空则借现有属性命令通道尝试投递（在线即时 / 离线排队 / 上线投递），delta 为空则不投递。
4. 投递/回报完全复用既有属性命令链路与 US-DV-004/009 行为，设备端零改动、命令语义不变。
5. 一次性命令通道临时值不改变 desired 视图（desired 优先）。
6. 管理员可在设备管理前端界面查看 desired/reported/delta 并设置期望状态。

### 5.2 验收目标

- 设置期望状态后，管理员可随时查询到该期望值，且期望值独立于一次性命令与设备上报持续可见。
- 管理员可看到 desired 与 reported 之间逐属性的 delta；设备上报收敛到期望后，相应属性从 delta 中消失。
- 在线设备设置期望后，差异在合理时间内被投递并随设备回报反映在 reported/delta 视图；离线设备在重新连接后收到排队的差异。
- 设备回报失败时，desired 视图保持期望值，delta 视图仍显示待收敛，平台不自动重推。
- 一次性命令的临时值不污染 desired 视图，desired 与命令的临时偏离在 delta 视图中可观测。
- 前端可在设备上下文中查看与设置期望状态并看到差异反馈。

---

## 6. API 相关约束

**适用性**: 适用

### 接口能力范围

- 新增管理端能力：为设备设置期望状态（Set-Desired）；查询设备期望状态、reported 快照与 delta（Get-Delta）。
- 复用既有能力（不改语义）：一次性属性命令创建、在线推送、离线排队、上线投递、ack 回写（US-PA-016 / US-DV-004 / US-DV-009）。

### 访问控制原则

- 管理端 desired 接口遵循现有管理端接口的访问控制原则（Herald 配置时受认证保护，未配置时单租户部署不做额外鉴权），与属性命令接口一致。
- desired 写入收口到 Set-Desired 入口；一次性命令通道与设备回调均不写 desired。

### 数据边界

- desired 文档以 `(product_id, device_id)` 为维度组织，与现有 reported 快照、属性命令的 product_id/device_id 维度一致。
- delta 计算以 desired 为基准、与 reported 逐属性比对（适配 reported 的现有存储形态）。
- 设备端零改动：不引入设备固件依赖，不新增设备拉取/订阅 desired 的协议层。

### 兼容性要求

- 不修改既有 `property_command`、reported 快照、属性回调的状态机与语义。
- 不引入设备端协议变更（设备零改动）。

> 具体端点、请求/响应参数表、HTTP 状态码与表结构属设计范畴，由技术设计承接，不在本 PRD 展开。

---

## 7. 前端/交互约束

**适用性**: 适用

### 页面入口

- 在设备管理后台的设备上下文中提供期望状态区域（与现有设备详情/设备管理页保持一致的导航层级）。

### 关键交互

- 在期望状态区域同时展示 desired、reported 及逐属性 delta，未收敛（待收敛）属性被清晰标出。
- 管理员可从界面填写并提交期望属性值；提交成功后刷新展示新的 desired/reported/delta。
- 设置失败（如提交内容为空）时显示明确错误提示，已有期望状态视图不被破坏。
- 差异下发结果随设备上报逐步反映在 delta 视图中（被动收敛：平台不在设备偏离时自动重推）。

### 状态反馈

- 期望状态保存成功/失败的即时反馈。
- delta 视图随 reported 更新动态变化；设备回报失败时 desired 保持、delta 仍显示待收敛，不自动重推。

### 权限可见性

- 前端期望状态管理与现有设备管理界面遵循同一访问控制；设备端不受影响（零改动）。

---

## 8. 已确认决策

- **D0 真问题定义**：持久 desired 状态视图（非完整 AWS Shadow 协议、非纯可观测性增强）。
- **D0 收敛路径**：平台推送，复用现有属性命令通道，设备零改动。
- **D0 desired×命令关系**：desired 仅由 Set-Desired 写入；命令与设备上报都不写 desired。单一「何时更新 desired」规则：只有管理员显式 Set-Desired 时更新。
- **被动收敛**：设备上报偏离 desired 时平台不自动重推，由管理员判断是否再次 Set-Desired。
- **desired 优先**：一次性命令的临时值不污染 desired 视图。
- **D0-R5 删除语义**：Set-Desired 的 patch 中，`null` = 删除该 desired 属性（非 null 覆盖、null 删除，对齐 AWS IoT Shadow / Azure IoT Hub）。
- **D-前端面板**：包含前端 desired/delta 管理界面。
- **D-合并顺序**：接受「一次性命令与 desired delta 同属性冲突时最终下发值不保证严格 last-write-wins」的既有命令通道行为，不要求新增设备协议。

---

## 9. 参考资料

- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（US-PA-042 / US-PA-043 / US-PA-044 / US-PA-016）、`docs/user-stories/02-iot-device-user-stories.md`（US-DV-004、US-DV-009）
- 相关 PRD：`docs/prd/core/product-device-management.md`（属性命令下发、reported 快照）
- 相关 PRD：`docs/prd/integration/rmqtt-webhook.md`（属性上报/属性订阅/属性回复回调）
