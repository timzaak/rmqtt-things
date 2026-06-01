# IoT Device 用户故事

> 角色定义以 `docs/user-stories/_roles.md` 为准。

---

## 1. 设备认证

### 故事 1：设备 HMAC 认证 [US-DV-001]

**优先级**: P0

**【用户故事】**
**作为**：IoT Device
**我希望**：使用 client_id 和 HMAC 签名密码通过 MQTT Broker 认证
**从而**：安全地接入 MQTT 网络

**【验收标准】**

**场景 1：认证成功**
```gherkin
Given 设备持有正确的 client_id 和 HMAC 签名密码（格式为 nonce.timestamp.hash）
When 设备通过 RMQTT 连接并提交认证请求
Then 认证通过，设备可正常收发 MQTT 消息
```

**场景 2：密码格式错误**
```gherkin
Given 设备提交的密码格式不是三段式（nonce.timestamp.hash）
When RMQTT 调用认证回调
Then 认证被拒绝
```

**场景 3：时间戳超时**
```gherkin
Given 设备密码中的时间戳与服务器时间相差超过 5 分钟
When RMQTT 调用认证回调
Then 认证被拒绝
```

**场景 4：签名校验失败**
```gherkin
Given 设备密码中的 HMAC 签名与服务器计算的期望签名不匹配
When RMQTT 调用认证回调
Then 认证被拒绝
```

---

### 故事 2：设备 ACL 权限控制 [US-DV-002]

**优先级**: P0

**【用户故事】**
**作为**：IoT Device
**我希望**：只能在自己的主题空间内发布和订阅消息
**从而**：确保设备间数据隔离

**【验收标准】**

**场景 1：设备访问自己的事件/服务主题**
```gherkin
Given 设备 client_id 为 "sensor-001"，所属产品为 "product-a"
When 设备发布或订阅主题 "/product-a/sensor-001/thing/event/*" 或 "/product-a/sensor-001/thing/service/*"
Then ACL 允许操作
```

**场景 2：设备访问其他设备的主题**
```gherkin
Given 设备 client_id 为 "sensor-001"
When 设备尝试发布或订阅 "/product-a/sensor-002/thing/event/*"
Then ACL 拒绝操作
```

---

## 2. 属性上报

### 故事 3：上报属性数据 [US-DV-003]

**优先级**: P0

**【用户故事】**
**作为**：IoT Device
**我希望**：将采集到的属性数据上报到平台
**从而**：平台可以存储和展示设备的实时状态

**【验收标准】**

**场景 1：正常上报属性**
```gherkin
Given 设备已连接 MQTT 并在正确主题上发布属性数据
When 平台收到 RMQTT WebHook 回调
Then 系统存储属性到最新属性表和历史表，如请求应答则通过 MQTT 返回确认
```

**场景 2：属性数据 Schema 校验失败**
```gherkin
Given 产品已配置 Active 状态的属性校验模板
When 设备上报的属性数据不符合 Schema 定义
Then 系统拒绝本次上报，返回校验失败信息
```

---

### 故事 4：接收属性下发 [US-DV-004]

**优先级**: P1

**【用户故事】**
**作为**：IoT Device
**我希望**：订阅属性设置主题以接收平台下发的属性命令
**从而**：根据管理员的指令调整设备参数

**【验收标准】**

**场景 1：接收待处理的属性命令**
```gherkin
Given 设备订阅了属性设置主题
When 平台收到设备的订阅事件
Then 系统将所有该设备状态为 Pending 的属性命令通过 MQTT 推送给设备
```

**场景 2：上报属性下发结果**
```gherkin
Given 设备收到属性下发命令
When 设备处理完成后在回复主题上报处理结果（成功或失败）
Then 系统更新对应命令的状态为 Success 或 Failed
```

---

## 3. 事件上报

### 故事 5：上报事件 [US-DV-005]

**优先级**: P1

**【用户故事】**
**作为**：IoT Device
**我希望**：将运行中产生的事件上报到平台
**从而**：平台可以记录和分析设备事件

**【验收标准】**

**场景 1：正常上报事件**
```gherkin
Given 设备已连接 MQTT 并在事件主题上发布事件数据
When 平台收到 RMQTT WebHook 回调
Then 系统存储事件到事件历史表，如请求应答则通过 MQTT 返回确认
```

---

## 4. OTA 升级

### 故事 6：上报当前版本并接收升级 [US-DV-006]

**优先级**: P1

**【用户故事】**
**作为**：IoT Device
**我希望**：上报当前固件版本，并在有可用升级时收到推送
**从而**：设备固件保持最新

**【验收标准】**

**场景 1：上报版本后收到升级推送**
```gherkin
Given 设备在 OTA 版本主题上报当前版本号
When 平台检测到有适用于该设备且版本号大于当前版本的固件
Then 系统通过 MQTT 向设备推送升级消息，包含固件下载地址（S3 预签名 URL）、版本号和更新日志
```

**场景 2：无可用升级**
```gherkin
Given 设备在 OTA 版本主题上报当前版本号
When 平台未检测到适用的固件升级
Then 系统不推送升级消息，仅应答确认收到版本上报
```

**场景 3：版本上报防抖**
```gherkin
Given 设备已上报版本号且平台已记录
When 设备在 10 分钟内再次上报相同版本号
Then 平台忽略该重复上报，不触发升级检测
```

---

## 5. 文件上传

### 故事 7：请求文件上传 [US-DV-007]

**优先级**: P2

**【用户故事】**
**作为**：IoT Device
**我希望**：向平台请求文件上传凭证
**从而**：将文件上传到对象存储

**【验收标准】**

**场景 1：正常获取上传凭证**
```gherkin
Given 平台已配置 S3 存储，设备请求上传到允许的目录
When 设备通过 MQTT 发送文件上传请求
Then 系统返回 S3 预签名上传地址和字段，设备可使用该凭证上传文件
```

**场景 2：目录不被允许**
```gherkin
Given 设备请求上传到不在白名单中的目录
When 设备通过 MQTT 发送文件上传请求
Then 系统拒绝请求，提示"Directory not allowed"
```

---

## 6. 设备连接状态

### 故事 8：上报连接/断开状态 [US-DV-008]

**优先级**: P0

**【用户故事】**
**作为**：IoT Device
**我希望**：设备连接和断开时平台自动记录状态
**从而**：管理员可以监控设备的在线状况

**【验收标准】**

**场景 1：设备连接**
```gherkin
Given 设备通过 MQTT Broker 连接成功
When RMQTT WebHook 回调设备连接事件
Then 系统更新设备状态为 Online，记录 IP 地址和连接时间
```

**场景 2：设备断开**
```gherkin
Given 设备已连接 MQTT
When 设备断开连接，RMQTT WebHook 回调断开事件
Then 系统更新设备状态为 Offline，记录断开时间
```

---

## 7. 离线命令排队

### 故事 9：离线命令排队与上线投递 [US-DV-009]

**优先级**: P1

**【用户故事】**
**作为**：Device Admin
**我希望**：在设备离线时也能创建属性命令
**从而**：命令在设备下次上线时自动投递

**【验收标准】**

**场景 1：离线创建命令，设备上线后投递**
```gherkin
Given 设备未连接到 MQTT broker
When 管理员通过 API 创建属性命令 (POST /api/admin/property/command)
Then 命令存储为 "Pending" 状态
And 设备连接并订阅属性 topic 后，命令自动投递到设备
And 设备回复后，命令状态变为 "Success"
```

**场景 2：命令状态追踪**
```gherkin
Given 管理员已创建离线命令
When 查询命令列表 (GET /api/admin/property/command)
Then 可以看到命令状态从 "Pending" → "Sent" → "Success/Failed" 的完整生命周期
```

---

## 8. 设备自动注册

### 故事 10：设备首次连接自动注册 [US-DV-010]

**优先级**: P0

**【用户故事】**
**作为**：IoT Device
**我希望**：在所属产品开启自动注册时，首次通过 HMAC 认证连接即自动完成设备注册
**从而**：无需管理员预先手动注册，即可正常接入平台

**【验收标准】**

**场景 1：自动注册成功**
```gherkin
Given 设备所属产品已开启自动注册
And 设备持有有效的 HMAC 认证凭据
When 设备首次通过 MQTT 连接并通过认证
Then 平台自动创建该设备的身份记录，设备后续可正常上报属性、事件和接收命令
```

**场景 2：自动注册关闭时未注册设备被拒绝连接**
```gherkin
Given 设备所属产品未开启自动注册
And 设备未在 devices 表中有注册记录
And 设备持有有效的 HMAC 认证凭据
When 设备通过 MQTT 连接并通过认证
Then 设备连接被拒绝（认证回调返回 deny）
```

**场景 3：重复连接幂等**
```gherkin
Given 设备已通过自动注册创建了身份记录
When 设备再次连接
Then 平台不创建重复的设备记录，设备正常连接
```
