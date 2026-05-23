# OTA 固件升级管理 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**优先级**: P1

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-PA-011]` 创建 OTA 版本，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员在创建页面填写版本信息并上传固件文件，完成 OTA 版本创建

- `[US-PA-012]` 查看 OTA 版本列表，优先级 P1，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看和筛选 OTA 版本列表，支持删除确认

- `[US-PA-013]` 编辑/删除 OTA 版本，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员修改或删除 OTA 版本记录，支持重新上传固件文件

- `[US-PA-021]` 查看 OTA 版本详情，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员查看 OTA 版本的完整详情信息

- `[US-DV-006]` 上报当前版本并接收升级，优先级 P1，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备上报固件版本（含防抖），平台检测并推送升级

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P1 | 3 | 创建 OTA 版本、查看列表、设备上报版本接收升级 |
| P2 | 3 | 编辑/删除 OTA 版本、查看版本详情 |

---

## 2. 范围界定

### 2.1 包含功能
- OTA 版本记录管理（创建、查询、更新、删除）
- 设备版本上报和匹配升级推送
- 管理端文件上传（获取 S3 预签名上传凭证）
- 固件文件下载地址生成（S3 预签名下载 URL）

### 2.2 不包含功能 (Out of Scope)
- OTA 升级进度追踪
- OTA 升级结果确认（设备升级后是否成功的回调）
- 固件文件内容校验（除文件大小和 MD5 外）
- 固件灰度发布/分批推送策略
- 固件回滚

### 2.3 依赖项
- S3 兼容对象存储：存储固件文件，提供预签名上传/下载 URL
- RMQTT Broker：推送升级消息到设备
- PostgreSQL：OTA 版本和设备版本数据存储

---

## 3. 需求概述

### 3.1 功能描述
OTA 固件升级管理允许管理员创建固件版本记录，指定产品、版本 key、版本号、最小版本要求、固件文件位置和目标设备范围。当设备上报当前固件版本时，平台自动检测是否存在适用的升级版本，如果有则通过 MQTT 向设备推送升级消息，包含固件下载地址、版本号和更新日志。

固件文件通过 S3 兼容存储管理，管理员通过管理端获取预签名上传凭证，设备通过预签名下载 URL 获取固件文件。

### 3.2 关键特性
- OTA 版本以 (product_id, key) 为唯一维度
- 版本号编码：major * 100000 + minor * 1000 + patch，前端以 x.y.z 格式交互
- 支持指定目标设备范围（device_ids），为空则对所有设备生效
- 版本匹配逻辑：设备当前版本在 [min_version, max_version) 范围内时有可用升级
- 固件文件通过 S3 预签名 URL 上传和下载，上传时自动计算文件大小和 MD5
- 升级消息通过 MQTT 推送到设备的 OTA 升级主题，包含预签名下载 URL
- 设备版本上报含 10 分钟防抖，避免频繁触发升级检测
- 版本删除为软删除（status=1），非物理删除

---

## 4. 功能需求

### 5.1 核心需求
1. 管理员可创建 OTA 版本记录，指定产品、版本 key、版本号（x.y.z 格式）、最小版本、最大版本（可选）、固件文件、更新日志等
2. 创建/编辑页面支持固件文件上传，系统自动计算文件大小（bin_length）和 MD5 哈希（bin_md5），上传至 S3 存储
3. 可选指定目标设备 ID 列表，限定固件升级范围；未指定则对所有设备生效
4. 管理员可查看版本列表（按产品筛选、分页）、版本详情、编辑版本信息、删除版本（软删除，需二次确认）
5. 编辑页面中产品、版本 key、版本号为只读字段，不可修改
6. 创建/编辑页面具备未保存离开提示
7. 设备上报版本时，平台检测匹配的升级版本（版本号在 [min_version, max_version) 范围内）并自动推送，包含预签名下载 URL
8. 设备版本上报含 10 分钟防抖机制，避免重复上报频繁触发升级检测
9. 版本号在内部以整数存储（major * 100000 + minor * 1000 + patch），前端以 x.y.z 格式展示和输入

### 5.2 验收目标
- OTA 版本创建后可在列表中查看，版本号以 x.y.z 格式展示
- 固件文件上传后，file_key、bin_length、bin_md5 自动填入表单
- 设备上报版本后，如果存在匹配的升级，设备收到 MQTT 推送，推送包含预签名下载 URL
- 管理端文件上传凭证仅在配置允许的目录范围内有效
- 版本删除后不再出现在列表中（软删除）
- 创建/编辑页面有必填字段校验和未保存离开提示

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围
- 设备端回调接口：接收设备版本上报（通过 RMQTT WebHook）
- 管理端 OTA 接口：OTA 版本 CRUD、文件上传凭证获取
- MQTT 推送接口：向设备推送升级消息

### 访问控制原则
- 管理端文件上传接口对目录白名单进行校验
- 设备端文件上传请求对目录白名单进行校验，支持 productId/deviceId 变量替换

### 数据边界
- OTA 版本以 product_id 为一级维度
- 设备版本记录以 (product_id, device_id, key) 为唯一维度

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口
- `/ota` - OTA 版本列表页（已实现）
- `/ota/create` - 创建 OTA 版本页（已实现）
- `/ota/edit/$id` - 编辑 OTA 版本页（已实现）
- `/ota/show/$id` - OTA 版本详情页（已实现）

### 关键交互
- 列表页支持按产品下拉筛选、分页浏览、删除二次确认
- 创建页/编辑页支持固件文件选择上传，上传中/成功/失败状态反馈
- 创建页/编辑页支持设备 ID 逐个添加/移除（Enter 键或 Add 按钮）
- 创建页/编辑页具备未保存离开提示（UnsavedGuard）
- 版本号输入框在失焦时校验 x.y.z 格式
- 详情页展示完整字段和设备 ID 标签列表，提供返回列表入口

---

## 7. 技术设计承接

**适用性**: 不适用
当前后端 API 和前端页面均已完成实现。

---

## 8. 相关文件索引

### 9.1 后端文件
- `backend/src/api/ota_handlers.rs` - 设备端 OTA 版本上报 handler
- `backend/src/api/admin_handlers.rs` - 管理端 OTA 版本 CRUD 和文件上传 handlers
- `backend/src/api/admin_models.rs` - OTA 相关请求/响应模型（CreateOtaVersionRequest, UpdateOtaVersionRequest, OtaVersionQuery）
- `backend/src/db/ota.rs` - OTA 版本数据库操作（CRUD、版本匹配、防抖）
- `backend/src/db/models.rs` - OtaVersion 模型定义
- `backend/src/api/tests/ota_tests.rs` - OTA 后端集成测试

### 9.2 前端文件
- `frontend/src/routes/ota/index.tsx` - OTA 版本列表页
- `frontend/src/routes/ota/create.tsx` - 创建 OTA 版本页
- `frontend/src/routes/ota/edit.$id.tsx` - 编辑 OTA 版本页
- `frontend/src/routes/ota/show.$id.tsx` - OTA 版本详情页
- `frontend/src/hooks/useOta.ts` - OTA 相关 React Query hooks
- `frontend/src/lib/version.ts` - 版本号格式化/解析/校验工具
- `frontend/src/lib/upload.ts` - S3 文件上传工具（含 MD5 计算）
- `frontend/src/routes/ota/__tests__/` - 前端单元测试（index, create, edit, show）

---

## 9. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`, `docs/user-stories/02-iot-device-user-stories.md`
- 相关 PRD：`docs/prd/core/product-device-management.md`
