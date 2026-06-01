# 事件校验模板管理 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-007]` 创建校验模板，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为产品创建事件校验模板

- `[US-PA-008]` 查看校验模板列表，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看和筛选校验模板列表

- `[US-PA-009]` 查看校验模板详情，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看模板完整详情

- `[US-PA-010]` 编辑校验模板，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员修改模板描述、Schema 和状态

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 2 | 创建校验模板、查看模板列表 |
| P1 | 2 | 查看模板详情、编辑模板 |

---

## 2. 范围界定

### 2.1 包含功能
- 校验模板 CRUD 管理（创建、列表查询、详情查看、编辑）
- 模板状态管理（Draft/Active/Inactive）
- JSON Schema 编辑和校验
- 属性 Schema 缓存管理（模板状态变更或更新时清除缓存）

### 2.2 不包含功能 (Out of Scope)
- Schema 版本管理
- Schema 导入/导出
- 批量操作
- Schema 可视化设计器（当前为 JSON 文本编辑）
- Schema 校验报告和历史

### 2.3 依赖项
- JSON Schema 校验库（jsonschema crate）
- 缓存层（Redis 或内存缓存，用于属性 Schema 运行时校验）
- 产品管理（模板关联到产品）

---

## 3. 需求概述

### 3.1 功能描述
事件校验模板管理允许管理员为产品创建 JSON Schema 校验规则，用于校验设备上报的属性数据。每个模板关联一个产品和一个事件名称（event 字段为 "property" 时表示属性校验模板）。

模板有三种状态：Draft（草稿，不生效）、Active（生效中，参与属性上报校验）、Inactive（已停用）。同一产品下只能有一个 Active 状态的同名事件模板。

当模板状态变更为 Active 或模板内容更新时，系统自动清除该产品的 Schema 缓存，确保下次属性上报时使用最新的 Schema。

### 3.2 关键特性
- 模板以 (product_id, event) 为维度，Active 状态下唯一
- JSON Schema 在创建和编辑时进行元数据校验（确保是合法的 JSON Schema）
- Active 状态模板的 Schema 不允许直接修改（需先停用再修改，或仅修改描述）
- event 字段为 "property" 的模板用于属性上报校验，其他值用于事件校验
- Schema 变更触发缓存清除

---

## 4. 功能需求

### 5.1 核心需求
1. 管理员可为产品创建校验模板，填写事件名称、描述和 JSON Schema
2. 创建时系统校验 JSON Schema 格式的合法性
3. 管理员可查看模板列表，按产品和事件名称筛选
4. 管理员可查看模板详情，包含完整的 Schema 内容
5. 管理员可编辑模板的描述、Schema 和状态
6. Active 状态模板的 Schema 为只读，防止运行时校验规则被意外修改
7. 模板更新或状态变更时，如果是属性模板，系统自动清除 Schema 缓存

### 5.2 验收目标
- 模板创建后立即出现在列表中
- 无效的 JSON Schema 被拒绝并给出明确错误提示
- Active 状态模板的 Schema 编辑器为只读状态
- 状态变更后，属性上报使用更新后的 Schema 进行校验

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围
- 管理端模板接口：模板 CRUD、状态更新
- 运行时校验接口：属性上报时自动使用 Active 状态的 Schema 进行校验

### 访问控制原则
- 管理端接口在 Herald 配置时受认证保护，未配置时不做鉴权（单租户部署模式）
- 运行时校验由后端自动执行，设备端无感知

### 数据边界
- 模板以 product_id 为一级维度
- Active 状态模板在 (product_id, event) 维度上唯一

### 兼容性要求
- Schema 变更可能影响设备上报的成功率，建议先在 Draft 状态测试后再激活

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口
- `/valid-templates` - 模板列表页（已实现）
- `/valid-templates/create` - 创建模板页（已实现）
- `/valid-templates/edit/$id` - 编辑模板页（已实现）
- `/valid-templates/show/$id` - 模板详情页（已实现）

### 关键交互
- 模板列表页支持按产品（下拉选择）和事件名称筛选，支持分页
- 创建页面提供 JSON Schema 编辑器，产品从 API 动态加载
- 编辑页面中，product_id 和 event 为只读字段
- Active 状态模板的 Schema 编辑器为只读
- 状态可选 Draft/Active/Inactive，通过下拉选择
- 详情页展示模板完整信息和 Schema，提供 Edit 入口
- 所有表单支持未保存离开确认（Unsaved Guard）

---

## 7. 技术设计承接

**适用性**: 不适用
当前功能已实现，技术细节直接体现在代码中。

---

## 8. 相关文件索引

### 9.1 后端文件
- `backend/src/api/admin_handlers.rs` - 校验模板 CRUD handlers（含缓存清除逻辑）
- `backend/src/db/database.rs` - 校验模板数据库操作
- `backend/src/db/models.rs` - EventValidTemplate、EventValidTemplateStatus 模型定义
- `backend/src/api/admin_models.rs` - 校验模板相关请求/响应模型
- `backend/src/cache.rs` - Schema 缓存管理

### 9.2 前端文件
- `frontend/src/routes/valid-templates/index.tsx` - 模板列表页（已实现）
- `frontend/src/routes/valid-templates/create.tsx` - 创建模板页（已实现）
- `frontend/src/routes/valid-templates/edit.$id.tsx` - 编辑模板页（已实现）
- `frontend/src/routes/valid-templates/show.$id.tsx` - 模板详情页（已实现）
- `frontend/src/components/schema/schema-editor.tsx` - JSON Schema 编辑器组件
- `frontend/src/components/schema/schema-display.tsx` - JSON Schema 展示组件
- `frontend/src/hooks/useEvents.ts` - 校验模板相关 React Query hooks

---

## 9. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`
- 相关 PRD：`docs/prd/core/product-device-management.md`
