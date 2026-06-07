# 文件上传服务 产品需求文档 (PRD)

**创建时间**: 2026-05-06
**优先级**: P2

---

## 1. 相关用户故事

> 详细故事与验收标准请查看 `docs/user-stories/` 中对应文档。

### 1.1 相关故事

- `[US-DV-007]` 请求文件上传，优先级 P2，来源 `docs/user-stories/02-iot-device-user-stories.md`
  - 角色：IoT Device
  - 摘要：设备向平台请求文件上传凭证，将文件上传到对象存储

- `[US-PA-022]` 获取文件上传凭证，优先级 P2，来源 `docs/user-stories/01-platform-admin-user-stories.md`
  - 角色：Platform Admin
  - 摘要：管理员通过后台获取文件上传凭证，上传文件到对象存储

### 1.2 优先级汇总

| 优先级 | 数量 | 关键故事 |
|--------|------|----------|
| P2 | 2 | 设备请求文件上传、管理员获取上传凭证 |

---

## 2. 范围界定

### 2.1 包含功能
- 管理端文件上传凭证获取（S3 预签名 POST URL）
- 设备端文件上传凭证获取（S3 预签名 POST URL）
- 目录白名单校验
- 文件名唯一性处理

### 2.2 不包含功能 (Out of Scope)
- 文件内容校验
- 文件下载管理（除 OTA 固件下载外，见 ota-management PRD）
- 文件删除
- 存储配额管理
- 文件版本管理

### 2.3 依赖项
- S3 兼容对象存储（MinIO、AWS S3 等）
- S3 配置（bucket、credentials、目录权限规则）

---

## 3. 需求概述

### 3.1 功能描述

文件上传服务为管理端和设备端提供统一的文件上传能力。两端均通过请求获取 S3 预签名上传凭证（presigned POST URL），然后使用该凭证直接将文件上传到 S3 兼容的对象存储。服务端不存储文件内容，仅生成上传凭证。

管理端通过 REST API 获取上传凭证，用于 OTA 固件文件等上传场景。设备端通过 MQTT 发送上传请求，平台通过 MQTT 返回预签名凭证。

### 3.2 关键特性
- 文件上传基于 S3 预签名 POST，客户端直传 S3，不经过服务端中转
- 目录白名单机制：仅允许上传到配置中明确允许的目录
- 文件名策略：默认生成唯一文件名（UUID 前缀），可选保留原始文件名
- 设备端目录支持 `${productId}` 和 `${deviceId}` 变量替换

---

## 4. 功能需求

### 5.1 核心需求
1. 管理员可通过管理端 API 获取文件上传凭证（S3 预签名 POST URL 和字段）
2. 设备可通过 MQTT 请求文件上传凭证
3. 上传凭证仅在配置允许的目录范围内有效
4. 默认生成唯一文件名，可选保留原始文件名

### 5.2 验收目标
- 管理端获取凭证后可直接上传文件到 S3
- 设备端获取凭证后可通过 MQTT 响应中的信息上传文件
- 非白名单目录的请求被拒绝
- useOriginName=false 时文件名包含 UUID 前缀确保唯一

---

## 5. API 相关约束

**适用性**: 必填
### 接口能力范围

- 管理端上传接口：管理员通过前端请求获取 S3 预签名上传凭证（presigned POST URL）
- 设备端上传接口：设备通过 RMQTT WebHook（Publish Hook）请求上传凭证，平台通过 MQTT 返回预签名信息
- 接口字段明细详见技术设计文档

### 访问控制原则
- 管理端上传接口当前不做鉴权（单租户部署模式）
- 设备端上传请求由 RMQTT Broker 转发
- 目录白名单校验是唯一的安全屏障

### 数据边界
- 文件以 directory + filename 为存储路径
- 设备端目录支持 `${productId}` 和 `${deviceId}` 变量替换为实际值

---

## 6. 前端/交互约束

**适用性**: 必填
### 页面入口
- 无独立页面。文件上传功能嵌入在 OTA 版本创建/编辑页面的固件文件上传组件中。

### 关键交互
- OTA 创建/编辑页面选择固件文件后，前端自动请求上传凭证并直传 S3
- 上传过程中展示进度和状态反馈
- 上传完成后自动计算并填入 file_key、bin_length、bin_md5

---

## 7. 技术设计承接

**适用性**: 不适用
当前功能已实现，技术细节直接体现在代码中。

---

## 8. 相关文件索引

### 9.1 后端文件
- `backend/src/api/admin_handlers.rs` - 管理端文件上传 handler (admin_file_upload_handler)
- `backend/src/api/handlers.rs` - 设备端文件上传 handler (file_upload_handler)
- `backend/src/api/tests/s3_tests.rs` - S3 预签名上传测试

### 9.2 前端文件
- `frontend/src/lib/upload.ts` - S3 文件上传工具（含 MD5 计算）
- `frontend/src/routes/ota/create.tsx` - OTA 创建页（使用文件上传）
- `frontend/src/routes/ota/edit.$id.tsx` - OTA 编辑页（使用文件上传）

---

## 9. 参考资料
- 用户故事：`docs/user-stories/01-platform-admin-user-stories.md`, `docs/user-stories/02-iot-device-user-stories.md`
- 相关 PRD：`docs/prd/integration/ota-management.md`, `docs/prd/integration/rmqtt-webhook.md`
