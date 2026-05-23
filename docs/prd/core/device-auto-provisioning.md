# 设备自动注册 (Device Auto-Provisioning) 产品需求文档 (PRD)

**创建时间**: 2026-05-19
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-036]` 配置产品自动注册，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为产品开启或关闭设备自动注册功能

- `[US-PA-037]` 查看设备注册来源，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员在设备列表中区分自动注册和手动注册的设备

- `[US-DV-010]` 设备首次连接自动注册，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：产品开启自动注册时，设备首次 HMAC 认证连接即自动创建设备身份记录

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 2 | 配置产品自动注册、设备首次连接自动注册 |
| P1 | 1 | 查看设备注册来源 |

---

## 2. 范围界定

### 2.1 包含功能
- 新增 `devices` 表作为设备**身份注册表**（区别于 `device_status` 连接状态表）
- 产品级别的设备自动注册开关（产品维度开启/关闭，默认关闭）
- HMAC 连接路径：产品开关开启时，设备首次 HMAC 认证连接自动创建设备身份记录
- 证书连接路径：管理员签发证书时同时在 `devices` 表创建记录（手动注册）
- 产品开关关闭时，未注册设备（`devices` 表中无记录）通过 HMAC 连接被拒绝
- 设备注册来源标记（区分自动注册与手动注册）
- 管理后台设备列表中展示注册来源并支持筛选

### 2.2 不包含功能 (Out of Scope)
- 设备注销/删除流程
- 设备黑名单机制（禁止特定 device_id 自动注册）
- 设备名称自定义（自动注册设备默认使用 device_id 作为名称）
- 动态注册主题协议（如阿里云一型一密的 MQTT 专用注册通道）
- 批量设备导入/预注册
- 设备注册审批流程
- 设备启用/禁用生命周期管理

### 2.3 依赖项
- `docs/prd/integration/cert-management.md` — 证书连接路径：签发证书时创建设备身份记录
- `docs/prd/integration/rmqtt-webhook.md` — HMAC 连接路径：auth webhook 阶段触发自动注册
- `docs/prd/core/product-device-management.md` — 产品与设备管理（产品模型扩展、设备列表展示）

### 2.4 关联说明

`docs/prd/core/product-device-management.md` 原始范围标注"设备注册/注销流程"为 Out of Scope。本 PRD 补充了设备自动注册部分，注销流程仍不在范围内。建议后续更新 `product-device-management.md` 的 Out of Scope 说明以引用本文档。

---

## 3. 需求概述

### 3.1 功能描述

当前 RMQTT Things 平台没有设备身份注册机制。设备通过 MQTT 连接时，平台仅在 `device_status` 表中隐式记录连接状态（UPSERT 语义），任何持有有效 HMAC 凭据的设备都能连接，缺少独立的设备身份准入控制。

本功能引入 **`devices` 设备身份注册表**，并支持两种注册路径：

| 连接方式 | 注册路径 | 触发时机 |
|---------|---------|---------|
| HMAC 认证 | 自动注册 | auth webhook 阶段，产品开关 ON 时首次连接自动创建 |
| TLS 证书 | 手动注册 | 管理员签发证书时同时创建 |

**两种连接方式互不依赖**：证书连接不走 HMAC，HMAC 连接不需要证书。

### 3.2 两张表的职责区分

| | `devices` 表（新增） | `device_status` 表（现有） |
|--|--|--|
| 职责 | 设备身份注册：设备是谁、怎么来的、何时注册 | 连接状态：在线/离线、IP、最后在线时间 |
| 生命周期 | 设备注册后永久存在（除非被删除） | 随连接/断开实时更新 |
| 创建时机 | 自动注册或证书签发时 | 设备连接时 UPSERT |
| 关联 | 通过 `(product_id, device_id)` 关联 `device_status` | - |

### 3.3 关键特性
- **产品维度控制**：自动注册以产品为单位开启/关闭，默认关闭
- **双路径注册**：HMAC 自动注册 + 证书手动注册，两种连接方式独立运作
- **准入控制**：`devices` 表成为 HMAC 连接的准入依据——无记录且产品开关 OFF 则拒绝
- **幂等注册**：同一设备重复连接不会创建重复记录（UPSERT 语义）
- **注册来源可追溯**：自动注册的设备标记 `auto`，手动注册标记 `manual`

---

## 4. 功能需求

### 5.1 核心需求

1. **新增 devices 表**：作为设备身份注册表，记录 product_id、device_id、registration_source、created_at。
2. **产品自动注册开关**：管理员可在产品编辑页面开启或关闭"设备自动注册"功能。新建产品默认关闭。
3. **HMAC 路径 — 自动注册**：产品开关 ON 时，设备首次通过 HMAC 认证后，平台自动在 `devices` 表创建记录（registration_source = auto）并允许连接。
4. **HMAC 路径 — 准入拒绝**：产品开关 OFF 时，未在 `devices` 表中有记录的设备，HMAC 认证通过后仍被拒绝连接。
5. **证书路径 — 手动注册**：管理员签发证书时，同时在 `devices` 表创建记录（registration_source = manual）。设备后续可用证书连接。
6. **注册幂等性**：同一 product_id + device_id 组合的注册请求幂等，重复操作不创建重复记录。
7. **注册来源标记**：设备记录包含 registration_source 字段，区分 `auto` 和 `manual`。

### 5.2 验收目标

- `devices` 表存在，可通过 `(product_id, device_id)` 唯一标识一台设备
- 产品编辑页面展示"设备自动注册"开关，开关状态可保存并可持久化
- 开关 ON 的产品：新设备 HMAC 首次认证后自动在 `devices` 表创建记录（auto），设备可正常连接
- 开关 OFF 的产品：新设备 HMAC 认证通过但 `devices` 表无记录 → 连接被拒绝
- 已有记录的设备：不受产品开关影响，HMAC 认证通过后正常连接
- 管理员签发证书时，`devices` 表同步创建记录（manual）
- 设备列表页面展示注册来源列（自动注册/手动注册），支持按注册来源筛选
- 同一设备重复连接不产生重复记录

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围
- 产品管理接口需扩展：支持读取和更新产品的自动注册开关（`auto_provisioning` 字段）
- 证书签发接口需扩展：签发证书时同时创建设备身份记录
- 设备查询接口需扩展：支持查询设备列表时包含注册来源信息，支持按注册来源筛选
- auth webhook 回调接口：认证通过后增加设备准入检查和自动注册逻辑

### 访问控制原则
- 产品自动注册开关的读写遵循现有管理端 API 权限控制
- auth webhook 回调由 RMQTT Broker 调用，不做额外鉴权（保持现有行为）
- 自动注册不改 HMAC 认证本身的验证逻辑，仅在验证通过后增加准入判断

### 兼容性要求
- 自动注册功能默认关闭，对现有产品无影响
- 已有的连接状态记录（device_status 表）与新的设备身份记录（devices 表）独立存在，通过 (product_id, device_id) 关联
- 证书连接路径不受自动注册开关影响

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口
- `/products/edit/$id` — 产品编辑页，需增加"设备自动注册"开关

### 关键交互
- 产品编辑页面中，"设备自动注册"为一个开关控件（toggle/switch），默认关闭
- 开关状态变更后需点击保存按钮统一提交
- 设备列表页面需增加"注册来源"列，展示"自动注册"或"手动注册"标签
- 设备列表页面增加按注册来源筛选的下拉选项

### 状态反馈
- 开关状态变更保存后，页面展示保存成功提示
- 自动注册功能仅在产品编辑页面可配置，不在创建页面展示（保持创建流程简洁）

---

## 7. 技术设计承接

**适用性**: 必填
技术预研报告：`.ai/tech-research/device-auto-provisioning.md`

预研报告中的关键设计决策：
- 推荐在 auth webhook 阶段触发自动注册（方案 A），而非 connect webhook 阶段
- 需要新增 `devices` 表存储设备身份记录
- 需要在 `products` 表增加 `auto_provisioning` 字段
- 无需引入新依赖，现有技术栈（axum、sqlx、hmac）可满足

如需接口细节、数据库设计、迁移方案，建议通过 `/t-design device-auto-provisioning` 产出技术设计文档。

---

## 8. 已确认决策与待确认假设

### 9.1 已确认决策
- 新建 `devices` 表作为设备身份注册表，与 `device_status`（连接状态）职责分离
- 两种连接方式独立运作：HMAC 认证和 TLS 证书是两条不同的连接路径
- HMAC 路径：auth webhook 中检查 `devices` 表 + 产品开关，决定自动注册或拒绝
- 证书路径：签发证书时同步创建 `devices` 记录（手动注册）
- 产品开关 OFF 时，`devices` 表中无记录的 HMAC 设备被拒绝连接
- 自动注册设备默认使用 device_id 作为名称，不提供自定义入口
- 不考虑向后兼容性问题

### 9.2 待确认假设
- 设备唯一性以 product_id + device_id 组合为准（沿用现有 device_status 模式） — 需技术设计确认
- 自动注册的设备默认为启用状态（active），不提供独立的启用/禁用管理 — 如需设备生命周期管理，建议作为后续独立需求

---

## 9. 相关文件索引

### 10.1 后端文件
- `backend/src/api/auth_handlers.rs` — HMAC 认证回调处理器（需修改：增加设备准入检查和自动注册逻辑）
- `backend/src/api/ca_handlers.rs` — 证书签发处理器（需修改：签发证书时同步创建设备记录）
- `backend/src/db/models.rs` — 数据模型定义（需修改：新增 Device 模型和注册来源枚举）
- `backend/src/db/product.rs` — 产品数据库操作（需修改：增加 auto_provisioning 字段读写）
- `backend/src/db/database.rs` — 数据库服务入口（需修改：注册设备 repo）
- `backend/src/db/cert_issue.rs` — 证书记录操作（签发流程需联动设备注册）

### 10.2 前端文件
- `frontend/src/routes/products/edit.$id.tsx` — 产品编辑页（需修改：增加自动注册开关）
- `frontend/src/routes/devices/index.tsx` — 设备列表页（需修改：增加注册来源列和筛选）
- `frontend/src/hooks/useProducts.ts` — 产品 hooks（需适配：支持自动注册字段）
- `frontend/src/hooks/useDevices.ts` — 设备 hooks（需适配：支持注册来源筛选）

---

## 10. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（US-PA-036、US-PA-037），`docs/user-stories/02-iot-device-user-stories.md`（US-DV-010）
- 相关 PRD：`docs/prd/integration/cert-management.md`，`docs/prd/integration/rmqtt-webhook.md`，`docs/prd/core/product-device-management.md`
- 技术预研：`.ai/tech-research/device-auto-provisioning.md`
