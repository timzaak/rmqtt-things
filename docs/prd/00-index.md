# PRD 索引

## 域分布

| 域 | PRD 数量 | 说明 |
|----|----------|------|
| auth | 1 | Admin 认证与权限管理（Herald 集成） |
| core | 3 | 产品与设备管理、告警规则引擎、设备自动注册（核心业务） |
| integration | 5 | 证书管理、OTA 固件升级、事件校验模板、RMQTT WebHook 集成、文件上传 |

## PRD 列表

| PRD | 域 | 优先级 | 说明 |
|-----|----|--------|------|
| [product-device-management.md](core/product-device-management.md) | core | P0 | 产品 CRUD、设备状态/属性/事件管理、属性命令下发、设备列表/详情前端页面 |
| [alarm-rule-engine.md](core/alarm-rule-engine.md) | core | P0 | 告警规则引擎：属性阈值/事件/设备状态触发、规则 CRUD、告警记录管理 |
| [device-auto-provisioning.md](core/device-auto-provisioning.md) | core | P0 | 设备自动注册：产品级自动注册开关、首次 HMAC 认证自动创建设备身份记录、注册来源标记 |
| [cert-management.md](integration/cert-management.md) | integration | P0 | TLS 证书签发/吊销、HMAC 设备认证、ACL 控制 |
| [ota-management.md](integration/ota-management.md) | integration | P1 | OTA 固件版本管理、设备版本上报与升级推送 |
| [validation-template.md](integration/validation-template.md) | integration | P0 | 事件/属性校验模板管理、JSON Schema 校验 |
| [rmqtt-webhook.md](integration/rmqtt-webhook.md) | integration | P0 | RMQTT WebHook 回调集成：认证、ACL、属性/事件上报、连接管理、MQTT 推送 |
| [file-upload.md](integration/file-upload.md) | integration | P2 | 管理端和设备端文件上传服务（S3 预签名） |
| [auth.md](auth/auth.md) | auth | P0 | Admin 认证与权限管理：Herald 集成、session 校验、权限控制、前端登录流程 |

## 关联关系

```
core/device-auto-provisioning
  --> core/product-device-management (产品模型扩展、设备列表展示)
  <-- integration/cert-management (HMAC 认证作为自动注册前置条件)
  <-- integration/rmqtt-webhook (auth webhook 阶段触发自动注册)

core/product-device-management

core/alarm-rule-engine
  --> core/product-device-management (规则绑定产品维度)
  <-- integration/rmqtt-webhook (规则评估在回调流程中触发)

integration/cert-management
  --> core/product-device-management (产品关联)
  <-- integration/rmqtt-webhook (认证和 ACL 回调)

integration/ota-management
  --> core/product-device-management (产品关联)
  <-- integration/rmqtt-webhook (设备版本上报回调)
  <-- integration/file-upload (固件文件上传)

integration/rmqtt-webhook
  --> core/product-device-management (属性/事件/连接数据持久化)
  --> integration/cert-management (认证和 ACL 校验)
  --> integration/ota-management (版本上报和升级推送)
  --> integration/file-upload (设备端文件上传)

integration/file-upload
  <-- integration/ota-management (管理端固件上传)
  <-- integration/rmqtt-webhook (设备端文件上传回调)

auth/auth
  --> core/product-device-management (所有管理端 API 认证保护)
  --> integration/cert-management (证书管理 API 认证保护)
  --> integration/ota-management (OTA 管理 API 认证保护)
  --> integration/validation-template (校验模板 API 认证保护)
  --> integration/file-upload (文件上传 API 认证保护)
```
