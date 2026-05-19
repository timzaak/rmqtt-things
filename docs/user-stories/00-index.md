# User Stories 索引

## 角色概览

详见 [_roles.md](_roles.md)

## 文件索引

| 文件 | 角色 | 故事数量 |
|------|------|----------|
| [01-platform-admin-user-stories.md](01-platform-admin-user-stories.md) | Platform Admin | 37 |
| [02-iot-device-user-stories.md](02-iot-device-user-stories.md) | IoT Device | 10 |
| [03-demo-e2e-user-stories.md](03-demo-e2e-user-stories.md) | Demo E2E | 6 |

## 故事 ID 索引

### Platform Admin (US-PA-xxx)

| ID | 标题 | 优先级 | 文件 |
|----|------|--------|------|
| US-PA-001 | 创建产品 | P0 | 01-platform-admin-user-stories.md |
| US-PA-002 | 查看产品列表 | P0 | 01-platform-admin-user-stories.md |
| US-PA-003 | 编辑产品 | P1 | 01-platform-admin-user-stories.md |
| US-PA-004 | 签发设备证书 | P0 | 01-platform-admin-user-stories.md |
| US-PA-005 | 查看证书列表 | P0 | 01-platform-admin-user-stories.md |
| US-PA-006 | 吊销/作废证书 | P0 | 01-platform-admin-user-stories.md |
| US-PA-007 | 创建校验模板 | P0 | 01-platform-admin-user-stories.md |
| US-PA-008 | 查看校验模板列表 | P0 | 01-platform-admin-user-stories.md |
| US-PA-009 | 查看校验模板详情 | P1 | 01-platform-admin-user-stories.md |
| US-PA-010 | 编辑校验模板 | P1 | 01-platform-admin-user-stories.md |
| US-PA-011 | 创建 OTA 版本 | P1 | 01-platform-admin-user-stories.md |
| US-PA-012 | 查看 OTA 版本列表 | P1 | 01-platform-admin-user-stories.md |
| US-PA-013 | 编辑/删除 OTA 版本 | P2 | 01-platform-admin-user-stories.md |
| US-PA-014 | 查看设备状态列表 | P0 | 01-platform-admin-user-stories.md |
| US-PA-015 | 查看设备属性历史 | P1 | 01-platform-admin-user-stories.md |
| US-PA-016 | 下发属性命令 | P1 | 01-platform-admin-user-stories.md |
| US-PA-017 | 查看设备事件历史 | P1 | 01-platform-admin-user-stories.md |
| US-PA-018 | 查看设备状态变更历史 | P2 | 01-platform-admin-user-stories.md |
| US-PA-019 | 设备列表页面 | P0 | 01-platform-admin-user-stories.md |
| US-PA-020 | 设备详情页面 | P0 | 01-platform-admin-user-stories.md |
| US-PA-021 | 查看 OTA 版本详情 | P2 | 01-platform-admin-user-stories.md |
| US-PA-022 | 获取文件上传凭证 | P2 | 01-platform-admin-user-stories.md |
| US-PA-023 | 查看证书详情 | P2 | 01-platform-admin-user-stories.md |
| US-PA-024 | 下载已签发证书和私钥 | P1 | 01-platform-admin-user-stories.md |
| US-PA-025 | 下载 CA 证书 | P2 | 01-platform-admin-user-stories.md |
| US-PA-026 | 管理员登录管理后台 | P0 | 01-platform-admin-user-stories.md |
| US-PA-027 | 管理员权限访问控制 | P0 | 01-platform-admin-user-stories.md |
| US-PA-028 | 会话过期处理 | P1 | 01-platform-admin-user-stories.md |
| US-PA-029 | 创建告警规则 | P0 | 01-platform-admin-user-stories.md |
| US-PA-030 | 查看告警规则列表 | P0 | 01-platform-admin-user-stories.md |
| US-PA-031 | 编辑告警规则 | P1 | 01-platform-admin-user-stories.md |
| US-PA-032 | 启用/禁用告警规则 | P1 | 01-platform-admin-user-stories.md |
| US-PA-033 | 删除告警规则 | P2 | 01-platform-admin-user-stories.md |
| US-PA-034 | 查看告警记录 | P0 | 01-platform-admin-user-stories.md |
| US-PA-035 | 确认告警 | P1 | 01-platform-admin-user-stories.md |
| US-PA-036 | 配置产品自动注册 | P0 | 01-platform-admin-user-stories.md |
| US-PA-037 | 查看设备注册来源 | P1 | 01-platform-admin-user-stories.md |

### Demo E2E (DEMO-xxx)

| ID | 标题 | 关联 | 文件 |
|----|------|------|------|
| DEMO-001 | 管理员查看已创建的产品 | US-PA-002 | 03-demo-e2e-user-stories.md |
| DEMO-002 | 管理员查看设备列表和详情 | US-PA-019 | 03-demo-e2e-user-stories.md |
| DEMO-003 | 管理员查看证书列表并导航到签发页 | US-PA-005 | 03-demo-e2e-user-stories.md |
| DEMO-004 | 管理员查看校验模板列表并导航到创建页 | US-PA-008 | 03-demo-e2e-user-stories.md |
| DEMO-005 | 管理员查看 OTA 版本列表并导航到创建页 | US-PA-012 | 03-demo-e2e-user-stories.md |
| DEMO-006 | 管理员签发设备证书完整 E2E 流程 | US-PA-004 | 03-demo-e2e-user-stories.md |

### IoT Device (US-DV-xxx)

| ID | 标题 | 优先级 | 文件 |
|----|------|--------|------|
| US-DV-001 | 设备 HMAC 认证 | P0 | 02-iot-device-user-stories.md |
| US-DV-002 | 设备 ACL 权限控制 | P0 | 02-iot-device-user-stories.md |
| US-DV-003 | 上报属性数据 | P0 | 02-iot-device-user-stories.md |
| US-DV-004 | 接收属性下发 | P1 | 02-iot-device-user-stories.md |
| US-DV-005 | 上报事件 | P1 | 02-iot-device-user-stories.md |
| US-DV-006 | 上报当前版本并接收升级 | P1 | 02-iot-device-user-stories.md |
| US-DV-007 | 请求文件上传 | P2 | 02-iot-device-user-stories.md |
| US-DV-008 | 上报连接/断开状态 | P0 | 02-iot-device-user-stories.md |
| US-DV-009 | 离线命令排队与上线投递 | P1 | 02-iot-device-user-stories.md |
| US-DV-010 | 设备首次连接自动注册 | P0 | 02-iot-device-user-stories.md |
