# Admin 认证与权限管理 产品需求文档 (PRD)

**创建时间**: 2026-05-12
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/01-platform-admin-user-stories.md` 第 7 节。

### 1.1 相关故事

| ID | 标题 | 优先级 | 来源 |
|----|------|--------|------|
| US-PA-026 | 管理员登录管理后台 | P0 | 01-platform-admin-user-stories.md |
| US-PA-027 | 管理员权限访问控制 | P0 | 01-platform-admin-user-stories.md |
| US-PA-028 | 会话过期处理 | P1 | 01-platform-admin-user-stories.md |

角色：Platform Admin

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 2 | US-PA-026 管理员登录、US-PA-027 权限控制 |
| P1 | 1 | US-PA-028 会话过期处理 |

---

## 2. 范围界定

### 2.1 包含功能

- Admin API 统一认证：所有管理端接口需经过认证才能访问
- 基于角色的权限控制：管理端接口按权限规则校验操作合法性
- 前端登录流程：未登录时自动跳转统一认证服务，登录后返回管理后台
- 前端会话失效处理：会话过期时自动引导重新登录
- 与统一认证服务（Herald）集成：通过 Herald SDK 校验 session 和权限

### 2.2 不包含功能 (Out of Scope)

- 用户注册、账号管理、密码修改——由 Herald 独立负责
- OAuth 第三方登录流程——由 Herald 独立负责
- 设备端认证（HMAC）——继续由现有证书管理模块负责，不受本次变更影响
- 管理后台内的角色/权限配置界面——由 Herald 管理端负责
- Herald 服务本身的部署和运维

### 2.3 依赖项

| 依赖 | 状态 | 说明 |
|------|------|------|
| Herald 统一认证服务 | 需已部署 | 提供 session 校验和权限查询能力 |
| Herald SDK | 需可用 | rmqtt-things 通过 SDK 调用 Herald |
| Herald 中已配置 realm 和 API Key | 需已配置 | rmqtt-things 在 Herald 中注册为客户端 |
| Herald 中已预置角色和权限规则 | 需已配置 | 管理端操作所需的权限定义 |

---

## 3. 需求概述

### 3.1 功能描述

rmqtt-things 当前管理端 API 无任何认证保护，任何人都可直接访问。本功能通过集成 Herald 统一认证服务，为管理端 API 增加认证和权限控制层，确保只有经过授权的管理员才能操作系统管理功能。

核心业务价值：
- 保护管理端 API 不被未授权访问
- 支持基于角色的精细化权限控制
- 通过统一认证服务实现单点登录，便于多系统统一管理

### 3.2 关键特性

- **认证保护范围明确**：仅保护管理端 API，设备端 API（MQTT 回调）不受影响
- **权限细粒度控制**：基于资源+操作维度校验权限（如设备管理读取、证书签发写入）
- **统一登录体验**：管理员通过 Herald 统一登录页登录，支持同域子域名和跨域两种部署模式
- **会话生命周期管理**：会话过期时自动引导重新登录，不产生不明确的错误

---

## 4. 功能需求

### 5.1 核心需求

**FR-1：管理端 API 认证**
- 所有管理端接口必须携带有效登录凭据
- 未携带凭据的请求返回未认证错误
- 认证校验通过 Herald SDK 完成，支持缓存以减少远程调用

**FR-2：管理端 API 权限控制**
- 已认证用户访问无权限资源时返回权限不足错误
- 权限校验维度为资源类型 + 操作类型（如设备管理+读取、证书+签发）
- 权限规则由 Herald 统一定义和管理

**FR-3：前端登录流程**
- 未登录用户访问管理后台时，自动跳转到 Herald 统一登录页
- 登录成功后自动返回管理后台，跳转到原访问页面或首页
- 支持两种部署模式：同域子域名（cookie 共享）和跨域（token 回调写入）

**FR-4：前端会话失效处理**
- 管理端请求返回未认证错误时，前端自动跳转到 Herald 登录页
- 页面加载时检测会话状态，已失效则跳转登录

### 5.2 验收目标

- 无有效登录凭据访问任何管理端 API → 返回未认证错误
- 有效登录凭据但无权限 → 返回权限不足错误
- 有效登录凭据且有权限 → 正常完成操作
- 未登录访问前端管理页面 → 跳转到 Herald 登录页，登录后返回原页面
- 设备端 HMAC 认证流程不受影响，设备通信正常

---

## 5. API 相关约束

**适用性**: 必填
### 认证保护范围

- **受保护**：所有管理端 API（`/api/admin/*`）需经过 Herald 认证和权限校验
- **不受保护**：设备端 API（`/api/thing/*`、`/api/access/*`、`/api/device/*`）继续使用 HMAC 认证

### 访问控制原则

- 认证方式：通过 Herald SDK 校验 session token
- 权限模型：资源 + 操作（resource + action），由 Herald 统一管理
- 权限缓存：SDK 内置缓存（约 5 分钟），避免每次请求都调用 Herald
- 用户身份：认证通过后将用户标识注入请求上下文，供后续业务使用

### 数据边界

- rmqtt-things 不存储用户账号信息，用户生命周期由 Herald 管理
- 请求上下文中携带用户标识和所属 realm，用于业务数据隔离

### 权限配置声明

rmqtt-things 要求 Herald 中预置以下权限点，用于管理端 API 的访问控制。共 3 个资源、2 种操作、6 个权限点。

> **注意**：告警规则（`/admin/alarm-rule`）和告警记录（`/admin/alarm`）接口在后端权限映射中归属 `device` 资源，因此访问告警相关接口需要 `device:read` / `device:write` 权限。

#### 资源定义

| 资源标识 | 说明 | 覆盖范围 |
|----------|------|----------|
| `product` | 产品与配置 | 产品、校验模板、文件上传 |
| `device` | 设备与数据 | 设备状态、属性、事件、属性命令 |
| `cert` | 证书与固件 | 证书签发/吊销、OTA 版本管理 |

#### 操作定义

| 操作标识 | 说明 | HTTP Method 映射 |
|----------|------|-------------------|
| `read` | 查看和列表查询 | GET |
| `write` | 新建、修改、删除、签发、吊销等所有写操作 | POST, PUT, PATCH, DELETE |

#### 权限点清单

| 权限点 | 说明 |
|--------|------|
| `product:read` | 查看产品、校验模板 |
| `product:write` | 创建/编辑产品、模板，上传文件 |
| `device:read` | 查看设备状态、属性、事件、命令 |
| `device:write` | 下发和删除属性命令 |
| `cert:read` | 查看证书和 OTA 版本 |
| `cert:write` | 签发/吊销证书，管理 OTA 版本 |

> 具体端点与权限的映射关系见 `/t-design auth` 产出的技术设计文档。

### 兼容性要求

- 现有设备端 HMAC 认证机制完全不变
- Herald 不可用时，管理端 API 应返回服务不可用错误，而非放行

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口

- 管理后台所有页面需经过认证检查
- 未登录用户统一跳转到 Herald 登录页

### 关键交互

- **登录跳转**：访问管理页面 → 检测未登录 → 跳转 Herald 登录页 → 登录成功 → 返回管理后台
- **会话失效**：操作过程中会话过期 → 自动跳转 Herald 登录页 → 重新登录 → 返回当前页面
- **跨域回调**（如适用）：Herald 登录后通过回调页将 token 写入本域 cookie

### 部署模式

- **同域子域名模式**（推荐）：Herald 登录后 cookie 在根域共享，管理后台自动获得认证状态
- **跨域模式**：通过回调页中转，Herald 登录后将 token 传递给管理后台并写入本域 cookie

### 状态反馈

- 管理端 API 返回未认证错误时，前端自动处理跳转，不展示错误提示框
- 跳转登录页时保留原访问地址，登录后自动返回

---

## 7. 技术设计承接

**适用性**: 必填
技术实现细节需通过 `/t-design auth` 产出，包括但不限于：
- Herald SDK 集成方式（依赖管理、客户端初始化）
- 后端 Auth Middleware 实现（Axum layer、session 校验、权限查询）
- 后端 AppState 扩展（SDK 客户端注入）
- Admin handler 用户身份提取方式
- 前端路由守卫实现（TanStack Router beforeLoad）
- 前端 API 客户端 401 拦截器
- 前端认证状态管理
- 配置项设计（Herald 地址、API Key、realm、client_id）
- OpenAPI 安全声明更新

参考技术方案：`.ai/future/integration_herald.md`

---

## 8. 相关文件索引

### 9.1 后端文件

| 文件 | 说明 |
|------|------|
| `backend/src/config.rs` | 需新增 HeraldConfig 配置项 |
| `backend/src/main.rs` | 需初始化 HeraldSdkClient 并注入 AppState |
| `backend/src/api/mod.rs` | 需对 Admin 路由组应用 auth middleware |
| `backend/src/api/middleware/mod.rs` | 需实现 Herald auth middleware |

### 9.2 前端文件

| 文件 | 说明 |
|------|------|
| `frontend/src/lib/auth.ts` | 已实现，认证状态管理 + 登录跳转工具 |
| `frontend/src/lib/api-client.ts` | 已添加 401 拦截器 |
| `frontend/src/routes/__root.tsx` 或路由配置 | 已添加路由守卫 |
| `frontend/src/lib/config.ts` | 需添加 Herald 地址配置 |

### 9.3 参考文件

| 文件 | 说明 |
|------|------|
| `.ai/future/integration_herald.md` | Herald 集成技术方案参考 |
| `docs/prd/integration/cert-management.md` | 设备端 HMAC 认证（不受影响） |

---

## 9. 参考资料

- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`（第 7 节：认证与权限）
- 角色：`docs/user-stories/_roles.md`
- Herald 集成方案参考：`.ai/future/integration_herald.md`
- 相关 PRD：`docs/prd/integration/cert-management.md`（设备端认证，独立于本次变更）
