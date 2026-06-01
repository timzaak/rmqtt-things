# Demo E2E 用户故事

> 每个模块选取一个核心用户故事，作为 Playwright E2E demo 测试的验收场景。

### 索引

| DEMO ID | 标题 | 关联用户故事 | 测试文件 |
|---------|------|-------------|---------|
| DEMO-001 | 管理员查看已创建的产品 | US-PA-002 | products-demo.e2e.ts |
| DEMO-002 | 管理员查看设备列表和详情 | US-PA-019 | devices-demo.e2e.ts |
| DEMO-003 | 管理员查看证书列表并导航到签发页 | US-PA-005 | certs-demo.e2e.ts |
| DEMO-004 | 管理员查看校验模板列表并导航到创建页 | US-PA-008 | valid-templates-demo.e2e.ts |
| DEMO-005 | 管理员查看 OTA 版本列表并导航到创建页 | US-PA-012 | ota-demo.e2e.ts |
| DEMO-006 | 管理员签发设备证书完整 E2E 流程 | US-PA-004 | certs-demo.e2e.ts |
| DEMO-007 | 设备上报属性、事件、接收命令和状态追踪 | US-DV-003/004/005/008/009 | mqtt-device-flow-demo.e2e.ts |
| DEMO-008 | 设备 ACL 权限控制验证 | US-DV-002 | device-acl-demo.e2e.ts |
| DEMO-009 | 设备 HMAC 认证验证 | US-DV-001 | device-hmac-auth-demo.e2e.ts |
| DEMO-010 | 属性命令下发完整流程 | US-PA-016 | property-command-demo.e2e.ts |
| DEMO-011 | 设备文件上传请求验证 | US-DV-007 | device-file-upload-demo.e2e.ts |
| DEMO-012 | 认证集成验证 | US-PA-026/027/028 | auth-demo.e2e.ts |
| DEMO-013 | 告警规则 CRUD 验证 | US-PA-029/030/031/032/033 | alarm-rules-demo.e2e.ts |
| DEMO-014 | 告警记录查看与确认 | US-PA-034/035 | alarms-demo.e2e.ts |
| DEMO-015 | 设备自动注册验证 | US-DV-010 | device-auto-registration-demo.e2e.ts |
| DEMO-016 | 设备列表筛选与导航 | US-PA-014/019 | device-filters-demo.e2e.ts |
| DEMO-017 | 产品自动注册开关验证 | US-PA-036 | product-auto-provisioning-demo.e2e.ts |
| DEMO-018 | 设备详情页面验证 | US-PA-020 | device-detail-demo.e2e.ts |
| DEMO-019 | 证书和私钥下载验证 | US-PA-024/025 | cert-download-demo.e2e.ts |
| DEMO-020 | 证书吊销/作废验证 | US-PA-006 | cert-revoke-demo.e2e.ts |
| DEMO-021 | 证书详情页验证 | US-PA-023 | cert-detail-demo.e2e.ts |

> `demo-basic.e2e.ts` 为基础设施测试（验证测试环境和 Logger），不关联用户故事。

---

## Products

### 故事：管理员查看已创建的产品 [DEMO-001]

**关联**: US-PA-002

**【用户故事】**
**作为**：Platform Admin
**我希望**：进入产品列表页时能看到已创建的演示产品
**从而**：确认产品和前端页面正常工作

**【验收标准】**

**场景 1：查看演示产品**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /products 页面
Then 页面显示 "Products" 标题、"Create Product" 链接、以及种子数据中的 "Demo Smart Light" 产品
```

---

## Devices

### 故事：管理员查看设备列表和详情 [DEMO-002]

**关联**: US-PA-019

**【用户故事】**
**作为**：Platform Admin
**我希望**：通过侧边栏导航到设备列表页，并能查看单个设备的详情
**从而**：确认设备页面导航和展示正常

**【验收标准】**

**场景 1：查看设备列表页**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /devices 页面
Then 页面显示 "Devices" 标题
```

**场景 2：从侧边栏导航到设备页**
```gherkin
Given 管理员在任意页面
When 点击侧边栏 "Devices" 链接
Then 页面跳转到 /devices 并显示 "Devices" 标题
```

**场景 3：查看设备详情**
```gherkin
Given 管理员在设备列表页
When 导航到 /devices/show/{device_id}
Then 页面显示 "Device Detail: {device_id}" 标题
```

---

## Certificates

### 故事：管理员查看证书列表并导航到签发页 [DEMO-003]

**关联**: US-PA-005

**【用户故事】**
**作为**：Platform Admin
**我希望**：在证书列表页查看已签发证书，并能导航到签发新证书的表单
**从而**：确认证书管理页面和表单正常展示

**【验收标准】**

**场景 1：查看证书列表页**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /certs 页面
Then 页面显示 "Certificates" 标题和 "Issue Certificate" 链接
```

**场景 2：证书列表有产品和设备筛选器**
```gherkin
Given 管理员在证书列表页
Then 页面显示 Product 筛选下拉框、Device ID 输入框和 Search 按钮
```

**场景 3：导航到签发证书页**
```gherkin
Given 管理员在证书列表页
When 点击 "Issue Certificate" 链接
Then 页面跳转到 /certs/create 并显示签发证书表单，包含 Product、Device ID、Start At、End At 字段
```

---

### 故事：管理员签发设备证书完整 E2E 流程 [DEMO-006]

**关联**: US-PA-004

**【用户故事】**
**作为**：Platform Admin
**我希望**：在签发证书页面填写表单并成功签发设备证书，签发后在证书列表中看到新证书
**从而**：确认证书签发完整流程从前端表单到后端签发到列表展示均正常工作

**【验收标准】**

**场景 1：完整签发证书流程**
```gherkin
Given 后端已启动且前端可访问，且系统中已有演示产品 "Demo Smart Light"
When 管理员导航到 /certs/create 页面，选择产品 "Demo Smart Light"，输入设备 ID，设定起止时间并提交
Then 页面跳转到 /certs 证书列表页，列表中出现刚签发的新证书记录，且状态为 Active（对应后端 Normal 状态）
```

**场景 2：签发表单必填校验**
```gherkin
Given 管理员在 /certs/create 页面
When 未填写 Device ID 就点击 Issue 按钮
Then 表单提示必填字段缺失，页面不跳转
```

---

## Valid Templates (Schema)

### 故事：管理员查看校验模板列表并导航到创建页 [DEMO-004]

**关联**: US-PA-008

**【用户故事】**
**作为**：Platform Admin
**我希望**：在校验模板列表页查看模板，并能导航到创建新模板的表单
**从而**：确认校验模板管理页面正常展示

**【验收标准】**

**场景 1：查看模板列表页**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /valid-templates 页面
Then 页面显示 "Schema Templates" 标题、"Manage event validation templates" 描述和 "Create Template" 链接
```

**场景 2：模板列表有产品和事件筛选器**
```gherkin
Given 管理员在模板列表页
Then 页面显示 Product 筛选下拉框、Event 输入框和 Search 按钮
```

**场景 3：导航到创建模板页**
```gherkin
Given 管理员在模板列表页
When 点击 "Create Template" 链接
Then 页面跳转到 /valid-templates/create 并显示创建模板表单，包含 Product、Event、Description 字段和 Create 按钮
```

---

## OTA

### 故事：管理员查看 OTA 版本列表并导航到创建页 [DEMO-005]

**关联**: US-PA-012

**【用户故事】**
**作为**：Platform Admin
**我希望**：在 OTA 版本列表页查看固件版本，并能导航到创建新版本的表单
**从而**：确认 OTA 版本管理页面正常展示

**【验收标准】**

**场景 1：查看 OTA 版本列表页**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /ota 页面
Then 页面显示 "OTA Versions" 标题和 "Create OTA Version" 链接
```

**场景 2：OTA 列表有产品筛选器**
```gherkin
Given 管理员在 OTA 版本列表页
Then 页面显示 Product 筛选下拉框和 Search 按钮
```

**场景 3：导航到创建 OTA 版本页**
```gherkin
Given 管理员在 OTA 版本列表页
When 点击 "Create OTA Version" 链接
Then 页面跳转到 /ota/create 并显示创建表单，包含 Product、Key、Version、Min Version、Firmware File 字段和 Create 按钮
```

---

## MQTT Device Flow

### 故事：设备上报属性、事件、接收命令和状态追踪 [DEMO-007]

**关联**: US-DV-003, US-DV-004, US-DV-005, US-DV-008, US-DV-009

**【用户故事】**
**作为**：IoT Device / Platform Admin
**我希望**：通过真实 MQTT 连接验证设备上报属性、事件、接收命令和在线/离线状态追踪的完整闭环
**从而**：确认 MQTT 协议集成和数据流从设备端到管理 API 全链路正常

**【验收标准】**

**场景 1：设备上报属性并查询 [US-DV-003]**
```gherkin
Given 真实 MQTT Broker 运行中
When 设备通过 MQTT 连接并上报属性（temperature, humidity, power）
Then 管理员通过 API 查询到与上报值一致的最新属性数据
```

**场景 2：设备上报事件并查询 [US-DV-005]**
```gherkin
Given 真实 MQTT Broker 运行中
When 设备通过 MQTT 上报事件（含唯一 marker）
Then 管理员通过 API 查询到包含该 marker 的事件记录
```

**场景 3：管理员下发命令，设备接收并回复 [US-DV-004]**
```gherkin
Given 设备已通过 MQTT 连接
When 管理员通过 API 创建属性命令
Then 设备收到命令，回复后命令状态变为 Success
```

**场景 4：离线命令排队并上线投递 [US-DV-009]**
```gherkin
Given 设备未连接 MQTT
When 管理员通过 API 创建属性命令（命令状态为 Pending）
And 设备连接并订阅属性主题后
Then 设备收到排队命令，回复后命令状态变为 Success
```

**场景 5：设备在线/离线状态追踪 [US-DV-008]**
```gherkin
Given 设备通过 MQTT 连接
When 设备连接成功
Then API 查询设备状态为 Online
When 设备断开连接
Then API 查询设备状态为 Offline
```

---

## Device ACL

### 故事：设备 ACL 权限控制验证 [DEMO-008]

**关联**: US-DV-002

**【用户故事】**
**作为**：IoT Device
**我希望**：只能在自己的主题空间内发布和订阅消息，访问其他设备主题被拒绝
**从而**：确认 ACL 机制正确实现设备间数据隔离

**【验收标准】**

**场景 1：设备可访问自己的主题**
```gherkin
Given 设备 client_id 为 "device-a"
When 设备发布和订阅自己对应的 thing/event 和 thing/service 主题
Then 操作成功完成
```

**场景 2：设备不可订阅其他设备的主题**
```gherkin
Given 设备 client_id 为 "device-a"
When 设备尝试订阅其他设备的主题
Then ACL 拒绝操作（返回 QoS 128 或连接断开）
```

**场景 3：设备不可发布到其他设备的主题**
```gherkin
Given 设备 client_id 为 "device-a"
When 设备尝试发布到其他设备的主题
Then ACL 拒绝操作（连接断开）
```

---

## Device HMAC Auth

### 故事：设备 HMAC 认证验证 [DEMO-009]

**关联**: US-DV-001

**【用户故事】**
**作为**：IoT Device
**我希望**：使用正确的 HMAC 签名密码通过 MQTT 认证，错误密码被拒绝
**从而**：确认 HMAC 认证机制在各种场景下正确工作

**【验收标准】**

**场景 1：正确凭据认证成功**
```gherkin
Given 设备持有正确的 client_id 和 HMAC 签名密码
When 设备通过 RMQTT 连接
Then 认证通过，设备可正常订阅和发布消息
```

**场景 2：密码格式错误被拒绝**
```gherkin
Given 设备提交的密码不是三段式格式
When RMQTT 调用认证回调
Then 认证被拒绝
```

**场景 3：过期时间戳被拒绝**
```gherkin
Given 设备密码中的时间戳与服务器时间相差超过 5 分钟
When RMQTT 调用认证回调
Then 认证被拒绝
```

**场景 4：错误签名被拒绝**
```gherkin
Given 设备密码中的 HMAC 签名与期望值不匹配
When RMQTT 调用认证回调
Then 认证被拒绝
```

---

## Property Command

### 故事：属性命令下发完整流程 [DEMO-010]

**关联**: US-PA-016

**【用户故事】**
**作为**：Platform Admin
**我希望**：通过 UI 向在线设备发送属性命令，设备接收回复后状态变为 Success，离线设备命令状态为 Pending
**从而**：确认属性命令从创建到投递到回复的完整生命周期正常

**【验收标准】**

**场景 1：在线设备发送命令并回复 [US-PA-016]**
```gherkin
Given 设备通过 MQTT 在线
When 管理员在设备详情页通过 UI 发送属性命令
Then 设备收到命令，回复后命令状态变为 Success
```

**场景 2：离线设备创建命令状态为 Pending**
```gherkin
Given 设备未连接 MQTT
When 管理员通过 API 创建属性命令
Then 命令状态为 Pending，在设备详情页可见
```

**场景 3：删除 Pending 命令**
```gherkin
Given 存在 Pending 状态的属性命令
When 管理员点击删除按钮
Then 命令状态变为 Deleted
```

---

## Device File Upload

### 故事：设备文件上传请求验证 [DEMO-011]

**关联**: US-DV-007

**【用户故事】**
**作为**：IoT Device
**我希望**：通过 MQTT 请求文件上传凭证，允许的目录返回预签名 URL，不允许的目录被拒绝
**从而**：确认设备文件上传的 MQTT 协议和目录白名单机制正常工作

**【验收标准】**

**场景 1：上传到允许的目录**
```gherkin
Given 设备通过 MQTT 连接
When 设备请求上传到允许的目录（如 own directory 或 public）
Then 设备收到响应（S3 可用时返回预签名 URL，不可用时返回 503）
```

**场景 2：上传到不允许的目录**
```gherkin
Given 设备通过 MQTT 连接
When 设备请求上传到不在白名单中的目录
Then 设备未收到成功响应（请求被拒绝）
```

---

## Auth Integration

### 故事：认证集成验证 [DEMO-012]

**关联**: US-PA-026, US-PA-027, US-PA-028

**【用户故事】**
**作为**：Platform Admin
**我希望**：验证 Herald SSO 认证配置检测、管理员登录和页面访问正常工作
**从而**：确认认证集成在启用和未启用两种模式下均可正常运行

**【验收标准】**

**场景 1：检测认证配置 [US-PA-026]**
```gherkin
Given 后端 API 运行中
When 查询认证配置端点
Then 返回 enabled 布尔值，表示是否启用认证
```

**场景 2：管理员登录后访问 API [US-PA-027]**
```gherkin
Given 管理员通过认证
When 访问管理端 API（如 /api/admin/products）
Then 返回非 401 状态码
```

**场景 3：登录后访问管理页面**
```gherkin
Given 管理员已通过认证
When 导航到管理后台页面（如 /devices）
Then 页面正常加载并显示内容
```

---

## Alarm Rules

### 故事：告警规则 CRUD 验证 [DEMO-013]

**关联**: US-PA-029, US-PA-030, US-PA-031, US-PA-032, US-PA-033

**【用户故事】**
**作为**：Platform Admin
**我希望**：通过 UI 和 API 完成告警规则的创建、列表查看、编辑、启用/禁用和删除操作
**从而**：确认告警规则管理的完整 CRUD 功能正常工作

**【验收标准】**

**场景 1：查看告警规则列表页 [US-PA-030]**
```gherkin
Given 后端已启动且前端可访问
When 管理员导航到 /alarm-rules 页面
Then 页面显示告警规则列表和产品筛选器
```

**场景 2：创建告警规则 [US-PA-029]**
```gherkin
Given 管理员在创建告警规则页面
When 填写规则名称、选择产品和触发条件后提交
Then 规则创建成功，出现在规则列表中
```

**场景 3：按产品筛选规则 [US-PA-030]**
```gherkin
Given 系统中存在多个产品的告警规则
When 选择特定产品进行筛选
Then 仅展示该产品的规则
```

**场景 4：编辑告警规则 [US-PA-031]**
```gherkin
Given 存在一个告警规则
When 管理员修改条件或动作后提交
Then 修改已保存并反映在列表中
```

**场景 5：启用/禁用告警规则 [US-PA-032]**
```gherkin
Given 存在一个告警规则
When 管理员切换启用/禁用状态
Then 规则状态即时更新
```

---

## Alarms

### 故事：告警记录查看与确认 [DEMO-014]

**关联**: US-PA-034, US-PA-035

**【用户故事】**
**作为**：Platform Admin
**我希望**：查看告警触发的历史记录并确认已处理的告警
**从而**：确认告警记录的端到端流程（规则触发 -> 记录创建 -> 列表展示 -> 确认操作）正常工作

**【验收标准】**

**场景 1：查看告警记录列表 [US-PA-034]**
```gherkin
Given 通过 API 创建规则并通过 MQTT 触发告警
When 管理员导航到 /alarms 页面
Then 页面显示告警记录列表，包含触发时间、规则名称、设备和级别
```

**场景 2：按产品筛选告警 [US-PA-034]**
```gherkin
Given 系统中存在多个产品的告警记录
When 选择特定产品进行筛选
Then 仅展示该产品的告警记录
```

**场景 3：确认告警 [US-PA-035]**
```gherkin
Given 存在一条未确认的告警记录
When 管理员点击确认操作
Then 告警确认状态已更新
```

---

## Device Auto-Registration

### 故事：设备自动注册验证 [DEMO-015]

**关联**: US-DV-010

**【用户故事】**
**作为**：IoT Device
**我希望**：在产品开启自动注册时，首次 HMAC 认证连接即自动完成注册；关闭时未注册设备被拒绝
**从而**：确认设备自动注册机制和产品级开关正确工作

**【验收标准】**

**场景 1：开启自动注册，设备首次连接自动注册**
```gherkin
Given 产品已开启自动注册
When 新设备首次通过 HMAC 认证连接
Then 平台自动创建设备记录（注册来源为 Auto）
```

**场景 2：关闭自动注册，未注册设备被拒绝**
```gherkin
Given 产品已关闭自动注册
When 未注册的设备尝试连接
Then 连接被拒绝（认证回调返回 deny）
```

**场景 3：已注册设备不受开关影响**
```gherkin
Given 设备已注册（无论注册来源）
When 产品关闭自动注册后该设备再次连接
Then 设备正常连接
```

---

## Device Filters

### 故事：设备列表筛选与导航 [DEMO-016]

**关联**: US-PA-014, US-PA-019

**【用户故事】**
**作为**：Platform Admin
**我希望**：在设备列表页查看设备状态信息、使用产品和状态筛选器、点击设备 ID 跳转到详情页
**从而**：确认设备列表页的筛选和导航交互正常工作

**【验收标准】**

**场景 1：查看设备状态列表 [US-PA-014]**
```gherkin
Given 系统中已有设备连接记录
When 管理员导航到 /devices 页面
Then 设备列表展示 Online/Offline 状态和 IP 地址
```

**场景 2：按在线/离线状态筛选 [US-PA-019]**
```gherkin
Given 系统中已有在线和离线设备
When 管理员筛选 Online 状态
Then 列表仅展示在线设备
```

**场景 3：点击设备进入详情 [US-PA-019]**
```gherkin
Given 设备列表页已展示设备数据
When 管理员点击某个设备的链接
Then 页面跳转到该设备的详情页面
```

---

## Product Auto-Provisioning

### 故事：产品自动注册开关验证 [DEMO-017]

**关联**: US-PA-036

**【用户故事】**
**作为**：Platform Admin
**我希望**：在产品编辑页面看到自动注册开关，新建产品默认关闭，切换开关后通过 API 验证生效
**从而**：确认产品级自动注册配置的 UI 交互和后端一致性

**【验收标准】**

**场景 1：编辑页面展示自动注册开关**
```gherkin
Given 存在一个产品
When 管理员进入产品编辑页
Then 页面展示设备自动注册开关（Auto Provisioning）
```

**场景 2：新建产品默认关闭**
```gherkin
Given 管理员创建了一个新产品
When 进入产品编辑页
Then 自动注册开关默认为未选中状态
```

**场景 3：通过 UI 启用自动注册**
```gherkin
Given 自动注册当前为关闭状态
When 管理员勾选开关并保存
Then 通过 API 查询确认 auto_provisioning 为 true
```

---

## Device Detail

### 故事：设备详情页面验证 [DEMO-018]

**关联**: US-PA-020

**【用户故事】**
**作为**：Platform Admin
**我希望**：在设备详情页查看设备完整信息，包括基本信息、最新属性、属性历史、事件历史、命令历史和连接状态历史
**从而**：确认设备详情页各区域正确展示数据

**【验收标准】**

**场景 1：详情页展示所有区域标题**
```gherkin
Given 系统中已有设备和种子数据
When 管理员导航到 /devices/show/{device_id}
Then 页面展示 Device Info、Latest Properties、Property History、Event History、Property Commands、Connection History 区域标题
```

**场景 2：设备信息展示种子数据**
```gherkin
Given seed 数据中有 demo-device 设备
When 管理员查看设备详情页
Then Device Info 区域展示 device_id 和 product_id，状态为 Online
```

**场景 3：最新属性和属性历史展示数据**
```gherkin
Given 设备已上报属性
When 管理员查看设备详情页
Then Latest Properties 区域展示属性数据，Property History 区域展示上报历史
```

---

## Cert Download

### 故事：证书和私钥下载验证 [DEMO-019]

**关联**: US-PA-024, US-PA-025

**【用户故事】**
**作为**：Platform Admin
**我希望**：签发证书后页面展示证书和私钥内容并提供下载按钮，同时可下载 CA 证书
**从而**：确认证书签发后的下载功能完整可用

**【验收标准】**

**场景 1：签发后展示证书和私钥 [US-PA-024]**
```gherkin
Given 管理员在签发证书页面提交签发请求
When 签发成功
Then 页面展示证书 PEM 和私钥 PEM 内容，提供下载按钮，并提示私钥仅显示一次
```

**场景 2：下载证书 PEM 文件 [US-PA-024]**
```gherkin
Given 签发成功且页面展示了下载区域
When 管理员点击下载证书按钮
Then 浏览器下载设备证书 PEM 文件
```

**场景 3：下载私钥 PEM 文件 [US-PA-024]**
```gherkin
Given 签发成功且页面展示了下载区域
When 管理员点击下载私钥按钮
Then 浏览器下载私钥 PEM 文件
```

**场景 4：下载 CA 证书 [US-PA-025]**
```gherkin
Given 管理员在证书列表页
When 点击下载 CA 证书按钮
Then 浏览器下载 CA 证书 PEM 文件
```

---

## Cert Revoke

### 故事：证书吊销/作废验证 [DEMO-020]

**关联**: US-PA-006

**【用户故事】**
**作为**：Platform Admin
**我希望**：吊销或作废 Normal 状态的证书，非 Normal 状态证书不显示操作按钮
**从而**：确认证书状态变更操作和 UI 交互正确

**【验收标准】**

**场景 1：吊销 Normal 证书**
```gherkin
Given 存在一条 Active（Normal）状态的证书
When 管理员点击 Revoke 并确认
Then 证书状态变更为 Revoked
```

**场景 2：作废 Normal 证书**
```gherkin
Given 存在一条 Active（Normal）状态的证书
When 管理员点击 Invalidate 并确认
Then 证书状态变更为 Invalid
```

**场景 3：非 Active 状态证书不显示操作按钮**
```gherkin
Given 存在 Revoked 或 Invalid 状态的证书
When 证书列表渲染该行
Then 该行不显示 Revoke 和 Invalidate 操作按钮
```

---

## Cert Detail

### 故事：证书详情页验证 [DEMO-021]

**关联**: US-PA-023

**【用户故事】**
**作为**：Platform Admin
**我希望**：在证书列表点击某条证书查看其完整详情，包括 PEM 证书内容
**从而**：确认证书详情页展示完整信息且可返回列表

**【验收标准】**

**场景 1：查看证书详情**
```gherkin
Given 存在一条证书记录
When 管理员在证书列表点击 Show 链接
Then 页面跳转到 /certs/show/$id，展示证书完整信息（ID、产品、设备 ID、PEM 内容、起止时间、状态、创建时间）
```

**场景 2：返回证书列表**
```gherkin
Given 管理员正在查看证书详情页
When 点击 Back to Certificates 链接
Then 页面跳转回证书列表页
```
