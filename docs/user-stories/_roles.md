# 角色定义

| 角色 | 说明 | 典型场景 |
|------|------|----------|
| Platform Admin | 平台管理员，通过 Web 后台管理 IoT 产品、设备、证书、OTA 固件和校验模板 | 创建产品、签发证书、发布固件升级、管理事件校验模板 |
| IoT Device | 接入 MQTT 的物联网设备，通过 RMQTT WebHook 与平台交互 | 上报属性、上报事件、接收属性下发、上报版本、接收 OTA 升级 |
| Backend Service | 后端 API 服务，接收 RMQTT WebHook 回调并提供管理 API | 处理设备数据上报、提供管理接口 |
