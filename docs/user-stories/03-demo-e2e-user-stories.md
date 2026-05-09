# Demo E2E 用户故事

> 每个模块选取一个核心用户故事，作为 Playwright E2E demo 测试的验收场景。

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
Then 页面跳转到 /certs 证书列表页，列表中出现刚签发的新证书记录，且状态为 Normal
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
