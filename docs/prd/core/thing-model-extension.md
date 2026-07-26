# 物模型能力扩展 产品需求文档 (PRD)

**域**: core
**优先级**: P1
**状态**: 发布（`/t-prd-publish`）

---

## 1. 相关用户故事

> 详细故事与验收标准见 `docs/user-stories/`。角色定义以 `docs/user-stories/_roles.md` 为准。

### 1.1 相关故事

**影子设备（产品语义由本 PRD 与 `docs/prd/core/shadow-device-support.md` 共同承载）**：

- `[US-PA-042]` 设置设备期望状态，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
- `[US-PA-043]` 查看设备期望状态与差异，优先级 P1，来源同上
- `[US-PA-044]` 在前端管理设备期望状态，优先级 P2，来源同上

> 影子设备的完整业务规则、状态机与验收细节以 `docs/prd/core/shadow-device-support.md` 为权威基线；本 PRD 仅在物模型协议层面定义其位置，不重复其验收文本。

**动作 / 服务调用（本 PRD 引入的新能力）**：

- `[US-PA-048]` 调用设备动作 / 服务（action / service invocation），优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：向设备下发一次性动作指令（如重启、开锁、触发蜂鸣），收到执行结果，且不污染属性状态视图
- `[US-PA-049]` 在前端区分动作调用与属性下发，优先级 P2，来源同上
  - 角色：Platform Admin
  - 摘要：动作调用与属性下发有独立入口和展示，操作意图清晰

**复用既有故事（动作调用传输层复用）**：

- `[US-PA-016]` 下发属性命令，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
- `[US-DV-004]` 接收属性下发，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
- `[US-DV-009]` 离线命令排队与上线投递，优先级 P1，来源同上

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P1 | 5 | 设置/查看期望状态、动作调用、下发属性命令（复用）、接收/上线投递（复用） |
| P2 | 2 | 前端管理期望状态、前端区分动作与属性 |

---

## 2. 范围界定

### 2.1 包含功能

**A. 影子设备在物模型中的位置定义**：

- 把 desired/reported/delta 能力正式纳入物模型协议规范与 API 参考，明确其在物模型中的位置：平台侧状态模型，对设备透明，复用属性命令通道，不引入新协议层。
- 范围与 `docs/prd/core/shadow-device-support.md` 一致：持久 desired 视图、被动收敛、Set-Desired 唯一写入入口、null=删除、复用属性命令通道、设备端零改动。

**B. 动作 / 服务调用（action / service invocation）**：

- 在物模型协议层把 `service_type` 从「仅 property」扩展到「property + action」，让一次性动作（重启、开锁、蜂鸣等）有独立于属性下发通道的表达。
- 动作调用是一次性的：不进入 desired/reported 状态视图，不参与收敛，回复只表示「执行了」而非「状态变了」。
- 全栈贯通：RMQTT WebHook / 自动订阅配置 → 后端回调与数据模型 → 前端入口与展示。

### 2.2 不包含功能 (Out of Scope)

- **影子设备的业务规则变更**：以 `docs/prd/core/shadow-device-support.md` 为权威，本 PRD 不重开其决策。
- **完整 AWS 式 Shadow 协议**（shadow topic / version 并发 / 设备端 SDK）——已由 shadow PRD 排除，本 PRD 承接该边界。
- **动作调用的复杂编排**（事务、Saga、多设备动作联动）——本 feature 仅做单设备单次动作调用。
- **动作调用的权限分级**（哪些角色能调用哪些动作）——复用现有管理端访问控制，不引入新的动作级权限模型。
- **物模型模板（JSON Schema）的动作定义编辑器**——物模型模板当前只校验事件/属性上报，扩展到动作参数校验不在本 feature 范围。动作清单来源不在本 PRD 定义（管理员手动指定 `service_type`，协议层不枚举）。

### 2.3 依赖项

- RMQTT WebHook：动作调用的回复回调转发（service 通配规则）。
- RMQTT 自动订阅：设备订阅动作主题（service 通配订阅）。
- RMQTT ACL：现有 `%c/#` 规则已覆盖动作主题，无需改动。
- 既有属性命令通道：动作调用的传输/排队/回报机制在传输层复用，但在数据层与属性命令可区分。
- 影子设备能力（`docs/prd/core/shadow-device-support.md`）。

---

## 3. 需求概述

### 3.1 功能描述

物模型协议在规范层把平台下行定义为「服务调用（service invocation）」，`service_type` 协议上可扩展。本 PRD 把平台下行明确分成两类语义：

1. **一次性动作**（开门、重启、蜂鸣）——action 服务类型，本 PRD 引入。
2. **属性设置 / 期望状态**——property 服务类型，含一次性属性下发与影子 desired 收敛。

两类语义在数据层、管理后台、前端均严格区分，避免动作塌缩进属性命令通道造成的语义混淆。同时，影子设备的 desired/reported/delta 能力在物模型协议规范中被正式记录位置。

### 3.2 关键特性

- 影子设备在物模型中的位置被正式定义：平台侧状态模型，对设备透明，复用属性命令通道，不引入新协议层。
- 动作调用作为与 `property` 并列的 `service_type`，独立于 desired/reported 状态视图。
- 动作调用复用既有命令通道的离线排队 / 上线投递 / ack 闭环，但语义上与属性命令可区分。
- `service_type` 在协议层支持任意合法标识（如 `reboot`、`unlock`），后端按服务类型路由，不在协议层枚举固定值。

---

## 4. 业务规则与状态

### 4.1 业务规则

**影子设备（承接已发布 PRD，不重开）**：

- **R1 desired 写入唯一性**：只有 Set-Desired 写 desired；一次性命令与设备上报都不写。
- **R2 被动收敛**：设备上报偏离 desired 时平台不自动重推。
- **R3 delta 收敛路径**：Set-Desired 算 delta，非空则借属性命令通道投递，设备零改动。
- **R5 null=删除**：Set-Desired patch 中 `null` 删除对应 desired 属性。
- （完整规则见 `docs/prd/core/shadow-device-support.md` §4.1）

**动作 / 服务调用**：

- **A1 动作一次性语义**：动作调用是一次性的，不进入 desired/reported 状态视图，不参与收敛。动作的回复表示「执行结果」，不更新 reported 快照。
- **A2 与属性命令语义分离**：动作调用与属性命令（含一次性属性下发 / desired delta）在数据层和展示层可区分。动作不压进属性命令的数据通道。
- **A3 复用传输通道**：动作调用的投递（在线即时 / 离线排队 / 上线投递 / ack 回写）在传输层复用既有命令通道机制，但用独立的服务类型标识。
- **A4 服务类型可扩展**：`service_type` 在协议层支持任意合法标识（字符集 `[a-zA-Z0-9_-]`，长度 1–32，如 `reboot`、`unlock`），后端按服务类型路由，不在协议层枚举固定值。

### 4.2 关键状态与异常

- **动作调用状态**：复用命令通道的 Pending / Sent / Success / Failed 状态语义，但能按「动作」维度过滤查询，与属性命令区分。
- **动作执行失败**：动作回报失败时，仅记录该次调用失败，不影响 desired/reported（动作不碰状态视图）。是否重试由管理员决定（与影子被动收敛一致的「不自动重推」原则）。
- **权限可见性**：动作调用管理端接口遵循现有管理端访问控制原则（Herald 配置时受认证保护，未配置时单租户部署不做额外鉴权）；设备端仍只访问自身 client_id 主题空间（ACL 已覆盖）。

---

## 5. 功能需求

### 5.1 核心需求

**影子设备（物模型规范化）**：

1. 物模型协议规范正式记录 desired/reported/delta 语义、被动收敛、与一次性命令的区别、对设备透明（详见 `docs/tutorials/thing-model-spec.md` 及英文版）。
2. API 参考记录已实现的设置期望状态、查询影子接口（详见 `docs/tutorials/api-reference.md` 及英文版）。

**动作 / 服务调用**：

3. 管理员可对设备调用动作（带入参），设备执行后通过回复通道返回结果。
4. 离线设备的动作调用排队，上线后投递（复用既有能力）。
5. 动作调用不进入 desired/reported 状态视图，与属性命令在管理后台可区分查看。
6. 物模型协议规范记录动作 / 服务调用作为独立服务类型（详见 `docs/tutorials/thing-model-spec.md` 服务调用章节）。
7. RMQTT WebHook / 自动订阅配置覆盖动作服务类型（service 通配规则与通配订阅）。
8. 前端设备详情页提供动作调用独立入口，动作历史与属性命令历史可区分（US-PA-049，P2）。

### 5.2 验收目标

- 读者查阅物模型协议规范，能看到影子设备和动作调用各自的明确定义与边界。
- 管理员能对动作型设备下发动作指令并收到执行结果；动作与属性下发在后台可区分。
- 动作调用不污染 desired/reported 视图；影子视图仍只反映属性状态。
- 设备离线时动作调用排队，上线后投递成功。
- 动作调用的回复遵循协议规范：任意 2xx 表示成功，其它表示失败。

---

## 6. API 相关约束

**适用性**: 适用

### 接口能力范围

- **影子设备**：设置期望状态（Set-Desired）、查询 desired/reported/delta（Get-Delta）。详细接口见 API 参考文档。
- **动作调用**：管理端能力——对设备发起动作 / 服务调用，查询动作调用历史（可按服务类型 / 状态过滤），软删动作调用记录。复用既有命令通道的投递与回报机制。

### 访问控制原则

- 影子与动作调用管理端接口遵循现有管理端访问控制（Herald 配置时受认证保护，未配置时单租户部署不做额外鉴权）。
- 设备端零协议变更：动作调用的设备侧接收复用既有服务调用主题空间，ACL 已通过 `%c/#` 覆盖。

### 数据边界

- 动作调用在数据层与属性命令（一次性属性下发 / desired delta）可区分，避免动作塌缩进属性命令数据通道。
- 影子数据边界不变（desired / reported 按 `(product_id, device_id)` 组织）。

### 兼容性要求

- 不破坏影子设备能力与既有属性命令状态机。
- 动作调用的引入对设备端是「新服务类型」，不修改既有 `property` 服务类型的语义。

> 具体端点、请求/响应参数表、HTTP 状态码与数据模型属设计范畴，由 `/t-design` 承接，不在本 PRD 展开。

---

## 7. 前端/交互约束

**适用性**: 适用

### 页面入口

- 设备详情页现有「命令」「影子」两个区域保持不变。
- 新增动作调用入口（独立于命令和影子），与设备支持的动作清单关联。

### 关键交互

- 管理员从动作入口选择动作、填写入参、提交；提交后看到调用状态（排队 / 已投递 / 已回报 / 失败）。
- 动作调用历史与属性命令历史可区分查看，不互相混入。

### 状态反馈

- 动作调用提交成功 / 失败的即时反馈。
- 动作状态随设备回报动态更新；设备回报失败时不自动重试（与被动收敛一致）。

### 权限可见性

- 前端动作调用入口与现有设备管理界面遵循同一访问控制。

---

## 8. 已确认决策

- **D0 feature 范围**：物模型能力扩展 = 影子设备在物模型中的位置定义 + 动作 / 服务调用（新能力）。
- **D0 feature 名与路径**：feature = `thing-model-extension`，domain = `core`。
- **D0 动作语义独立性**：动作是一次性的，不进入 desired/reported，不参与收敛（A1）。
- **D0 配置提前铺**：WebHook / 自动订阅的动作规则以 service 通配规则承载，与 property 共用统一 service 回调入口，避免重复命中。
- **D-影子边界承接**：影子设备语义以已发布 PRD `docs/prd/core/shadow-device-support.md` 为准，本 PRD 不重开其决策。

---

## 9. 参考资料

- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（US-PA-042 / 043 / 044 / 048 / 049 / 016）、`docs/user-stories/02-iot-device-user-stories.md`（US-DV-004 / 009）
- 相关 PRD：`docs/prd/core/shadow-device-support.md`（影子设备，已发布）
- 相关 PRD：`docs/prd/integration/rmqtt-webhook.md`（WebHook 集成，含影子/动作边界）
- 决策简报：`.ai/decision/shadow-device-support.md`（影子设备立项决策）
- 物模型协议规范：`docs/tutorials/thing-model-spec.md`（设备影子章节、服务调用章节）
- API 参考：`docs/tutorials/api-reference.md`（影子端点、动作调用端点章节）
- 配置文件：`conf/plugins/rmqtt-web-hook.toml`、`conf/plugins/rmqtt-auto-subscription.toml`
