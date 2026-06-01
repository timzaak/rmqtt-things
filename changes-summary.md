# 代码变更总结

> 基于 `git diff` 统计，共涉及 **21 个文件**，**+995 / -147** 行变更。

---

## 1. 后端 (Backend)

### 1.1 证书管理增强 (`ca_handlers.rs`, `cert_issue.rs`, `models.rs`)

- **证书状态更新增加状态机校验**：只能操作 `Normal` 状态的证书，不允许将状态设回 `Normal`
- **支持按证书 ID 更新状态**：`UpdateCertStatusRequest` 新增可选 `id` 字段，支持通过 ID 精准操作单条证书
- **新增 `update_status_by_id` 方法**：按 ID 更新证书状态，SQL 加 `AND status = 0` 防止重复操作
- **遗留批量模式保留**：无 `id` 时仍按 `product_id + device_id` 查询
- **`CertStatus` 增加 `TryFrom<i16>`**：支持从整数安全转换为枚举，无效值返回错误
- **查询排序优化**：`find_by_device_id` 增加 `ORDER BY id DESC`，取最新证书

### 1.2 产品创建去重 (`product_handlers.rs`)

- 捕获 PostgreSQL `unique_violation` (错误码 23505)，返回 **409 Conflict**（"产品型号编号已存在"），而非通用 500

### 1.3 数据库常量提取 (`database.rs`)

- 将 `"Cannot update schema of active template"` 提取为 `ACTIVE_TEMPLATE_SCHEMA_ERR` 常量，供 handler 层引用，避免脆弱的字符串匹配

### 1.4 通用错误映射 (`error.rs`)

- 新增 `map_db_err` 辅助函数，统一将数据库错误映射为 500 响应，替代各 handler 中重复的 `map_err` 闭包

### 1.5 OTA 版本匹配修复 (`ota.rs`)

- 最大版本判断从 `max_version >= $3` 改为 `max_version > $3`，修正边界条件（版本号等于 max_version 时不再匹配）

### 1.6 OpenAPI Schema 更新 (`openapi.json`)

- API 标题改为 `RMQTT Things API`，增加描述
- 设备列表 API 新增 `registration_source` 查询参数（筛选注册来源）
- 产品模型新增 `auto_provisioning` 字段（自动注册开关）
- `DeviceStatus` 重构为 `DeviceStatusWithSource`，包含 `registration_source` 和可空 `status`
- `DeviceStatusHistory` 独立为历史记录模型
- 新增 `RegistrationSource` 枚举（`Auto` / `Manual`）
- `UpdateCertStatusRequest` 新增 `id` 字段
- `herald_url` 拆分为 `herald_login_url` 和 `login_url`

---

## 2. 前端 (Frontend)

### 2.1 校验模板编辑页重构 (`valid-templates/edit.$id.tsx`)

- **新增状态选择器**：可直接在编辑页切换 Draft / Active / Inactive 状态
- **Active 模板 Schema 只读**：Active 状态下 Schema 编辑器禁用，显示黄色提示条
- **分离状态更新与内容更新**：使用独立的 `useUpdateEventValidTemplateStatus` hook
- **优化脏检查逻辑**：用 `prevDataKey` 替代 `prevTemplate` 对象引用比较

### 2.2 测试 Fixtures (`fixtures.ts`)

- 新增 `mockDraftValidTemplate` 和 `mockActiveValidTemplate` 测试数据

---

## 3. Demo / E2E 测试

### 3.1 属性命令测试 (`property-command-demo.e2e.ts`)

- 场景 1 标题更新为 "Send command online, device replies and status becomes Success"，更准确反映实际测试内容

---

## 4. 文档更新 (Docs)

### 4.1 PRD 文档

| 文件 | 变更要点 |
|------|---------|
| `auth.md` | 增加告警接口归属 `device` 资源的说明 |
| `alarm-rule-engine.md` | 补充触发类型 API 枚举值（`property`/`event`/`device_online`/`device_offline`） |
| `product-device-management.md` | 设备页面从"待实现"标记为"已实现"；访问控制更新为 Herald 认证模式 |
| `cert-management.md` | ACL 主题范围扩展（增加 `thing/file/*`、`ota/*`）；证书详情页和 CA 下载标记为已实现；技术设计标记为"不适用" |
| `validation-template.md` | 访问控制更新为 Herald 认证模式 |

### 4.2 用户故事

| 文件 | 变更要点 |
|------|---------|
| `01-platform-admin-user-stories.md` | 产品编辑增加 `auto_provisioning` 字段；证书签发流程更新为展示证书和私钥；OTA 创建增加最大版本号字段 |
| `02-iot-device-user-stories.md` | ACL 主题格式从 `/device_id/...` 更新为 `/product_id/device_id/...` |
| `03-demo-e2e-user-stories.md` | **大幅扩展**（+574 行），新增 DEMO-007 ~ DEMO-021 共 15 个 Demo 用户故事，覆盖设备 MQTT 流程、ACL、HMAC 认证、属性命令、文件上传、Auth 集成、告警、自动注册、证书操作等；新增 Demo 索引表 |

---

## 5. 变更影响范围

| 层级 | 文件数 | 关键影响 |
|------|--------|---------|
| 后端 Rust | 8 | 证书状态机、产品去重、OTA 边界修复、错误处理重构 |
| 后端 OpenAPI | 1 | Schema 重组，新增字段和枚举 |
| 前端 React | 2 | 模板编辑页状态管理重构 |
| Demo E2E | 1 | 测试标题更新 |
| 文档 (PRD + Stories) | 8 | 大量标记"已实现"、补充枚举值、新增 15 个 Demo 故事 |
