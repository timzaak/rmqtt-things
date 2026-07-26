# 设备详情信息架构 产品需求文档 (PRD)

**域**: core
**创建时间**: 2026-07-26
**优先级**: P1
**状态**: 发布（`/t-prd-publish`）

---

## 1. 相关用户故事

> 详细故事与验收标准见 `docs/user-stories/`。角色定义以 `docs/user-stories/_roles.md` 为准。

### 1.1 相关故事

- `[US-PA-020]` 设备详情页面，P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
- `[US-PA-015]` 查看设备属性历史，P1，来源同上
- `[US-PA-016]` 下发属性命令，P1，来源同上
- `[US-PA-043]` 查看设备期望状态与差异，P1，来源同上
- `[US-PA-044]` 在前端管理设备期望状态，P2，来源同上
- `[US-PA-048]` 调用设备动作 / 服务，P1，来源同上
- `[US-PA-049]` 在前端区分动作调用与属性下发，P2，来源同上
- `[US-PA-050]` 按业务意图理解设备详情，P1，来源同上
- `[US-PA-051]` 区分目标同步、直接属性写入和动作调用，P1，来源同上

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|---|---:|---|
| P0 | 1 | 设备详情页面 |
| P1 | 6 | 属性历史、属性命令、状态差异、动作、信息架构与操作语义 |
| P2 | 2 | 前端管理期望状态、区分动作与属性下发 |

---

## 2. 范围界定

### 2.1 包含功能

- 设备详情按“设备现状、持久目标、平台操作、设备上报”组织。
- 将持久目标以 `Target State` 的产品语义呈现，保留既有被动收敛规则。
- 将一次性属性写入、目标同步和动作调用作为不同操作类型展示。
- 区分投递/执行状态与属性同步状态。
- 提供统一操作历史视图及按类型、状态筛选能力。
- 原始属性和操作 JSON 作为详情信息展示，不作为主要识别信息。

### 2.2 不包含功能

- 不改变 MQTT topic、设备协议或设备固件。
- 不改变 desired、reported、属性命令及动作调用的持久化语义。
- 不增加设备偏离目标后的自动重推或自动控制器。
- 不增加 desired 版本、变更审计或多设备批量操作。
- 不把属性上报重新定义为完整时序遥测分析能力。

### 2.3 依赖项

- 既有设备详情、属性上报、属性命令、影子和动作调用能力。
- 属性命令来源可区分一次性写入与目标同步。
- 相关管理能力沿用现有设备访问控制原则。

---

## 3. 需求概述

### 3.1 功能描述

设备详情若以 Shadow、Commands、Actions、Property History 等同级技术区域组织，且目标同步复用属性命令通道，内部同步记录会出现在命令列表中；管理员难以判断一条记录是人工写入、目标同步还是动作，也容易把设备回复成功误认为属性已经达到目标。

本功能按管理员意图组织设备详情，不改变底层业务和传输边界。页面使用“目标值、当前值、操作、上报”描述产品概念，内部通道复用只在操作详情中按需说明。

### 3.2 关键特性

- 当前值与目标值在同一上下文中逐属性对照。
- 操作历史统一查看，但目标同步、直接属性写入和动作调用保持类型可辨识。
- 投递/执行与状态同步是两条独立状态轴。
- 直接属性写入作为高级的一次性操作，明确提示其不会更新目标值。

---

## 4. 业务规则与状态

### 4.1 业务规则

- **R1 意图优先**：页面按用户意图组织，不按 MQTT topic、接口或数据库表组织。
- **R2 目标语义**：Target 是持久目标记录；仅显式更新目标时改变。设备偏离后不自动重推。
- **R3 操作分类**：
  - Target sync：更新目标后产生的同步尝试。
  - Direct property write：不改变 Target 的一次性属性写入。
  - Action invocation：不进入 Target/Current 状态视图的一次性行为。
- **R4 内部记录隔离**：Target sync 不作为 Direct property write 展示；可在统一操作历史中以自身类型查看。
- **R5 上报语义**：Current 和 Property Reports 只反映设备实际上报，不因命令回复直接改变。
- **R6 直接写入提示**：存在 Target 时执行不同值的 Direct property write，页面必须说明可能产生 Out of sync。

### 4.2 关键状态与异常

**投递/执行状态**：

- Queued：等待设备上线或订阅。
- Sent：已投递，等待设备回复。
- Succeeded：设备成功回复。
- Failed：设备回复失败。
- Cancelled：待处理操作已取消。

**状态同步状态**：

- In sync：Current 等于 Target。
- Out of sync：Current 缺失或不等于 Target。
- Target not set：该属性没有持久目标。

Succeeded 不等于 In sync。设备回复成功但尚未上报目标值时，两种状态必须同时、如实显示。

---

## 5. 功能需求

### 5.1 核心需求

1. 顶层区域为 Overview、State & Configuration、Operations、Reported Data、Events、Connectivity、Metadata。
2. Overview 展示设备身份、连接状态、当前属性摘要、未同步目标和待处理/失败操作摘要。
3. State & Configuration 逐属性展示 Current、Target、同步状态、最后上报时间及可用操作。
4. 管理员可更新 Target，并对未同步属性再次 Apply；Apply 仍遵循被动收敛。
5. Operations 统一展示三类操作，支持按类型和状态筛选。
6. Direct property write 保留为高级操作，提交前明确其一次性语义和目标冲突影响。
7. Reported Data 展示最新值和属性上报历史；原始 JSON 通过详情展开查看。
8. 空态、加载失败、提交失败不得破坏其它已加载区域。

### 5.2 验收目标

- 管理员无需理解 DesiredDelta 即可区分三类平台操作。
- Target sync 不混入 Direct property write 列表。
- 任一操作均可独立识别投递/执行结果；有 Target 的属性可独立识别同步结果。
- 一次性写入与 Target 冲突时，页面在提交前提示并在设备上报后显示 Out of sync。
- 既有 Shadow、属性命令、动作调用和属性上报业务能力保持兼容。

---

## 6. API 相关约束

**适用性**: 适用

- 查询属性命令时应支持按操作来源筛选，且筛选在分页前完成。
- 统一操作历史可由现有操作来源聚合，但不得改变既有写入入口和状态机。
- 管理接口沿用现有设备管理访问控制和 `(product_id, device_id)` 数据边界。
- 现有客户端若不传新增筛选条件，查询行为保持兼容。

> 具体接口能力（设备最新属性、属性历史、属性命令、影子 desired/delta、动作/服务调用历史、设备状态等）见 API 参考文档 `docs/tutorials/api-reference.md`，本 PRD 不重复端点与参数。

---

## 7. 前端/交互约束

**适用性**: 适用

- 设备详情路由保持不变。
- 一级导航使用面向业务的名称，不出现 DesiredDelta 等内部术语。
- State & Configuration 以 Current/Target 对照表为核心；编辑失败后保留原视图。
- Operations 以可读摘要为主，原始参数通过详情查看。
- 直接属性写入不作为主要入口；动作调用和更新目标保持明确入口。
- 页面权限可见性沿用现有设备管理规则。

---

## 8. 已确认决策

- **D1 信息架构**：设备详情按状态、操作和上报数据组织，不按传输实现组织。
- **D2 Shadow 命名**：用户界面使用 Target State 表达现有被动收敛语义。
- **D3 操作历史**：历史视图可统一，写入意图和业务状态机不合并。
- **D4 状态拆分**：投递/执行状态与同步状态独立展示。
- **D5 直接属性写入**：保留为高级能力，不修改 Target。
- **D6 基线覆盖**：本 PRD 取代 `docs/prd/core/thing-model-extension.md` 中「设备详情页『命令』『影子』区域保持不变」的前端信息架构约束；其动作、属性和影子业务边界继续有效。
- **D7 协议边界**：不改变 MQTT、设备协议和被动收敛规则。

---

## 9. 参考资料

- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（US-PA-020 / 015 / 016 / 043 / 044 / 048 / 049 / 050 / 051）
- 相关 PRD：`docs/prd/core/product-device-management.md`（设备详情页基础能力）
- 相关 PRD：`docs/prd/core/shadow-device-support.md`（被动收敛与 desired 语义基线）
- 相关 PRD：`docs/prd/core/thing-model-extension.md`（动作 / 服务调用业务边界）
- 外部产品依据：AWS IoT Device Shadow / Commands、Azure IoT Hub Device Twins / Direct Methods、ThingsBoard Attributes / RPC
