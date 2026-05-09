# 证书管理 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**更新时间**: 2026-05-07
**状态**: Active
**优先级**: P0

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-004]` 签发设备证书，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员为设备签发 TLS 证书，设定有效期

- `[US-PA-005]` 查看证书列表，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看和筛选已签发证书

- `[US-PA-006]` 吊销/作废证书，优先级 P0，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员吊销或作废设备证书

- `[US-PA-023]` 查看证书详情，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看单条证书的完整详情（含 PEM 内容）

- `[US-PA-024]` 下载已签发证书和私钥，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：签发成功后下载证书和私钥 PEM 文件

- `[US-PA-025]` 下载 CA 证书，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：下载平台 CA 根证书用于设备端信任配置

- `[US-DV-001]` 设备 HMAC 认证，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备使用 HMAC 签名密码通过 MQTT 认证

- `[US-DV-002]` 设备 ACL 权限控制，优先级 P0，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备只能在自己的主题空间内发布和订阅

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P0 | 5 | 签发证书、查看证书列表、吊销/作废证书、设备认证、ACL 控制 |
| P1 | 1 | 下载已签发证书和私钥 |
| P2 | 2 | 查看证书详情、下载 CA 证书 |

---

## 2. 范围界定

### 2.1 包含功能
- 证书签发：基于内置 CA 为设备签发 TLS 客户端证书，签发后提供证书和私钥下载
- 证书列表查询：按产品和设备 ID 筛选，分页浏览
- 证书状态管理：吊销（Revoked）和作废（Invalid）操作
- 证书详情查看：查看单条证书的完整信息，包括 PEM 证书内容
- CA 证书下载：管理员可下载平台 CA 根证书
- 设备 MQTT 认证：基于 HMAC-SHA1 签名的密码验证
- 设备 ACL 控制：设备只能访问自己 client_id 对应的 thing/event 和 thing/service 主题

### 2.2 不包含功能 (Out of Scope)
- CA 证书管理（轮换、更新）
- 证书自动续签
- 证书到期提醒
- 证书吊销列表（CRL）/ OCSP
- 多租户证书隔离
- 外部 CA 集成

### 2.3 依赖项
- CA 证书和密钥文件（平台启动时自动生成或从配置加载）
- RMQTT Broker：调用认证和 ACL 回调

---

## 3. 需求概述

### 3.1 功能描述
证书管理模块负责为 IoT 设备签发 TLS 客户端证书，以及管理设备 MQTT 连接的身份认证和访问控制。

证书签发使用平台内置的 CA，管理员通过 Web 界面为指定产品和设备签发证书，设定有效期。系统会检查是否已存在有效证书以避免重复签发，同时提供强制重签选项。签发成功后，管理员应能获取并下载证书文件和对应的私钥文件，以便部署到目标设备。

平台启动时自动初始化 CA（生成或加载已有 CA 证书和密钥），同时生成服务器证书供 MQTT TLS 连接使用。管理员可下载 CA 根证书，用于设备端信任配置。

设备认证采用 HMAC-SHA1 签名机制：设备使用 client_id、随机 nonce、时间戳和预共享的 suffix 计算密码哈希，平台验证签名和时间戳有效性（5 分钟窗口）。

ACL 控制确保设备只能在自己 client_id 对应的主题空间内发布和订阅消息。

### 3.2 关键特性
- 证书状态流转：Normal -> Revoked/Invalid
- 同一设备同一产品默认不允许重复签发有效证书
- 签发后提供证书 PEM 和私钥 PEM 的下载能力（当前实现中私钥未持久化保存，签发后仅返回证书 PEM）
- CA 启动时自动初始化：不存在则生成，存在则加载校验，同时生成服务器证书
- HMAC 密码格式：`nonce.timestamp.hash`，nonce 长度 6 位，时间戳有效期 5 分钟
- ACL 规则：只允许 client_id 匹配的 `/thing/event/*` 和 `/thing/service/*` 主题

---

## 4. 当前实现状态

| 功能模块 | 状态 | 备注 |
|---------|------|------|
| CA 初始化（自动生成/加载） | ✅ 已实现 | 启动时自动处理，含服务器证书生成 |
| 证书签发 API | ✅ 已实现 | 支持防重复签发和强制重签 |
| 证书列表查询 API | ✅ 已实现 | 支持按产品和设备筛选、分页 |
| 证书状态更新 API | ✅ 已实现 | 支持吊销和作废 |
| 证书签发前端页面 | ✅ 已实现 | 创建页面已完成 |
| 证书列表前端页面 | ✅ 已实现 | 列表页已完成，含吊销/作废确认对话框 |
| HMAC 认证回调 | ✅ 已实现 | 完整的签名验证逻辑 |
| ACL 回调 | ✅ 已实现 | 主题空间隔离 |
| 私钥下载 | ❌ 未实现 | 当前 `issue_cert_handler` 仅返回证书 PEM，私钥生成后未返回或持久化 |
| 证书详情页面 | ❌ 未实现 | 无 `/certs/:id` 详情页 |
| CA 证书下载 API | ❌ 未实现 | 无管理端接口提供 CA 证书下载 |

---

## 5. 功能需求

### 5.1 核心需求
1. 管理员可为指定产品和设备签发 TLS 证书，设定起止时间
2. 签发时检查设备是否已有有效证书，有则拒绝（除非强制重签）
3. 签发成功后管理员可获取并下载证书 PEM 和私钥 PEM
4. 管理员可查看所有已签发证书列表，按产品和设备 ID 筛选
5. 管理员可吊销或作废状态为 Normal 的证书
6. 管理员可查看单条证书的完整详情（含 PEM 内容）
7. 管理员可下载平台 CA 根证书
8. 设备使用 HMAC-SHA1 签名密码通过 MQTT 认证
9. 设备只能在自己 client_id 对应的主题空间内发布和订阅

### 5.2 验收目标
- 证书签发后可在列表中立即看到新记录
- 签发成功后管理员可下载证书和私钥文件
- 非法密码格式、超时时间戳、错误签名均导致认证失败
- 非 Normal 状态证书不显示吊销/作废操作按钮
- 吊销/作废操作需二次确认
- 证书详情页展示完整证书信息，包括 PEM 内容

---

## 6. API 相关约束

**状态**: 必填

### 接口能力范围
- 认证回调接口：由 RMQTT Broker 调用，返回 allow/deny
- ACL 回调接口：由 RMQTT Broker 调用，返回 allow/deny
- 管理端证书接口：签发、列表查询、状态更新
- 待补充：证书详情查询接口（按证书 ID）、CA 证书下载接口

### 访问控制原则
- 认证和 ACL 回调接口由 RMQTT Broker 内部调用，不对外暴露鉴权
- 管理端接口当前不做鉴权（单租户部署模式）

### 兼容性要求
- HMAC 认证密码格式为三段式 `nonce.timestamp.hash`，格式变更需协调设备端固件更新
- ACL 规则变更影响设备主题访问权限，需确保向后兼容

---

## 7. 前端/交互约束

**状态**: 必填

### 页面入口
- `/certs` - 证书列表页（已实现）
- `/certs/create` - 签发证书页（已实现）
- `/certs/:id` - 证书详情页（待实现）

### 关键交互
- 证书列表页支持按产品（下拉选择）和设备 ID 筛选
- 证书状态以不同颜色标签展示（Active/Invalid/Revoked）
- 吊销和作废操作需弹出确认对话框
- 非Normal状态证书行不显示操作按钮
- 签发证书页的产品下拉列表从产品 API 动态加载
- 签发表单默认起止时间为当前时间至一年后
- 签发成功后展示证书内容和私钥内容，并提供下载按钮（待实现）
- 证书列表页提供下载 CA 证书的入口（待实现）
- 签发页面有未保存离开提示（UnsavedGuard）

---

## 8. 技术设计承接

**状态**: 必填

待补充的技术设计：
- **私钥持久化与下载**：当前 `issue_cert_handler` 调用 `generator::issue_cert` 生成 `(cert_pem, key_pem)`，但 `key_pem` 未存入数据库也未返回前端。需要设计私钥的存储策略（是否持久化）和下载方案。
- **证书详情页**：需设计按证书 ID 查询单条证书的接口（当前 `find_by_device_id` 按 product_id + device_id 查询，非按唯一 ID）。
- **CA 证书下载接口**：需设计管理端读取并返回 CA 证书 PEM 的接口。
- **证书列表总数**：当前 `list` 查询未返回总数，分页无法显示总页数。

建议通过 `/t-design certificate-management` 产出详细技术设计文档。

---

## 9. 相关文件索引

### 9.1 后端文件
- `backend/src/api/ca_handlers.rs` - 证书签发、列表、状态更新 handlers
- `backend/src/api/auth_handlers.rs` - HMAC 认证和 ACL 回调 handlers
- `backend/src/ca/generator.rs` - CA、设备证书、服务器证书生成逻辑
- `backend/src/ca/mod.rs` - CA 初始化（自动生成或加载 CA 和服务器证书）
- `backend/src/db/cert_issue.rs` - 证书数据库操作
- `backend/src/db/models.rs` - CertIssue、CertStatus 模型定义
- `backend/src/config.rs` - CA 和 MQTT 认证配置

### 9.2 前端文件
- `frontend/src/routes/certs/index.tsx` - 证书列表页（已实现）
- `frontend/src/routes/certs/create.tsx` - 签发证书页（已实现）
- `frontend/src/hooks/useCerts.ts` - 证书相关 React Query hooks

---

## 10. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`, `docs/user-stories/02-iot-device-user-stories.md`
- 相关 PRD：`docs/prd/core/product-device-management.md`
