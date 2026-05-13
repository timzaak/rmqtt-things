# 认证与权限

系统里有两套独立的认证体系，分别服务设备和管理后台，互不影响。

| | 设备端 | 管理端 |
|--|--------|--------|
| 认证方式 | HMAC-SHA1 | Herald SSO |
| 保护范围 | MQTT 连接 | Admin HTTP API (`/api/admin/*`) |
| 凭证存储 | 设备本地生成密码 | Herald 统一管理 session |
| 代码位置 | `auth_handlers.rs` | `middleware/mod.rs` |

设备端认证（HMAC）的细节见[架构文档](architecture.md)的"设备认证和 ACL 流程"一节。本文只讲管理端 Herald SSO 的集成。

## Herald 是什么

Herald 是一个独立的统一认证服务，负责用户注册、登录、session 管理、权限定义。rmqtt-things 不存用户信息，只通过 Herald SDK 校验请求里的 session token 和权限。

两个服务的关系：

```
浏览器 → Caddy → rmqtt-things App → Herald SDK → Herald 服务
                  (后端 API)                       (校验 token + 查权限)
```

rmqtt-things 不直接连 Herald 的数据库。所有认证和权限查询走 Herald 的 HTTP API（`POST /api/ext/permission/check`），SDK 内部有约 5 分钟的缓存，不是每个请求都调 Herald。

## 后端集成

### 配置

在 `config.toml` 中添加 `[herald]` 段：

```toml
[herald]
base_url = "http://127.0.0.1:3000"  # Herald 服务地址
api_key = "your-api-key"            # 调用 Herald ext API 的密钥
realm_id = "default"                # rmqtt-things 所属的 realm
client_id = "rmqtt-things-admin"    # 客户端标识
```

不配这个段，管理端 API 就没有认证保护（本地开发可以不配）。生产环境必须配。

`main.rs` 启动时根据这个配置初始化 Herald SDK 客户端：

```rust
let herald_client = config.herald.as_ref().map(|herald| {
    Arc::new(herald_sdk::Client::new(
        herald.base_url.clone(),
        herald.api_key.clone(),
        None,
    ))
});
```

### 中间件

`middleware/mod.rs` 里的 `herald_auth_middleware` 是一个 Axum 中间件函数，运行在所有 Admin API 请求上。处理流程：

```
请求进来
  → 从 Cookie 中提取 X-Auth token
    → 没有 token → 401 Unauthorized
  → 根据请求路径和 HTTP 方法生成权限规则（resource + action）
    → 路径不在保护范围 → 403 Forbidden
  → 调用 Herald SDK 校验权限（带缓存）
    → allowed=true → 把 user_id 注入 request extensions，放行
    → allowed=false → 403 Forbidden
    → Herald 不可用 → 503 Service Unavailable
```

Herald 不可用时返回 503 而不是放行。宁可管理后台暂时不能用，也不能让未认证的请求进来。

### 权限模型

权限分两个维度：资源（resource）和操作（action）。

**资源映射**（请求路径 → 资源标识）：

| 路径前缀 | 资源 | 覆盖的管理端接口 |
|----------|------|-----------------|
| `/admin/product*`、`/admin/valid*`、`/admin/file*` | `product` | 产品、校验模板、文件上传 |
| `/admin/device*`、`/admin/property*`、`/admin/event*` | `device` | 设备状态、属性、事件、命令 |
| `/admin/ca*`、`/admin/ota*` | `cert` | 证书签发/吊销、OTA 版本 |

**操作映射**（HTTP 方法 → 操作标识）：

| HTTP 方法 | 操作 |
|-----------|------|
| GET | `read` |
| POST, PUT, PATCH, DELETE | `write` |

**完整的 6 个权限点**：

| 权限点 | 说明 |
|--------|------|
| `product:read` | 查看产品、校验模板 |
| `product:write` | 创建/编辑产品、模板，上传文件 |
| `device:read` | 查看设备状态、属性、事件、命令 |
| `device:write` | 下发和删除属性命令 |
| `cert:read` | 查看证书和 OTA 版本 |
| `cert:write` | 签发/吊销证书，管理 OTA 版本 |

这些权限点需要在 Herald 管理端预先配置好角色和规则。rmqtt-things 本身不管角色定义，只查 Herald "这个用户能不能做这件事"。

### 路由保护范围

在 `api/mod.rs` 的 `create_router()` 里，中间件只在 Herald 配置存在时才生效：

```rust
let admin_routes = match (config.herald.as_ref(), herald_client) {
    (Some(herald_config), Some(herald_sdk)) => {
        admin_routes.layer(axum::middleware::from_fn_with_state(
            HeraldAuthState { herald_sdk, client_id: herald_config.client_id.clone().into() },
            herald_auth_middleware,
        ))
    }
    (_, _) => admin_routes,  // 没配 herald，admin 路由不加认证
};
```

不受影响的接口：

- `/api/thing/*` — 设备 WebHook 回调
- `/api/access/*` — 设备认证和 ACL
- `/api/device/*` — 设备上下线通知
- `/api/health` — 健康检查
- `/api/auth/config` — 认证配置查询（前端用来判断是否开了 Herald）

## 前端流程

### 登录跳转

用户打开管理后台时，`__root.tsx` 的 `beforeLoad` 钩子会检查认证状态：

```typescript
beforeLoad: async ({ location }) => {
    if (location.pathname === '/auth/callback') return
    const authed = await checkAuth()
    if (!authed) {
        handle401()  // 跳转到 Herald 登录页
        throw new Error('unauthenticated')
    }
}
```

`checkAuth()` 先查 `/api/auth/config` 看有没有开 Herald。没开就直接通过。开了就发一个探测请求到 `/api/admin/product?page=1&page_size=1`，根据返回状态判断 session 是否有效。

未登录时跳转到 Herald 的登录页，URL 格式：

```
{herald_base_url}/login?redirect={app_base_url}/auth/callback?redirect={当前页面}
```

### 跨域回调

登录成功后 Herald 重定向到 `/auth/callback?token=xxx&redirect=xxx`。`callback.tsx` 把 token 写入 Cookie 并跳转到原页面：

```typescript
export function completeAuthCallback(search = window.location.search): string {
    const params = new URLSearchParams(search)
    const token = params.get('token')
    const redirect = params.get('redirect') || '/'
    if (token) setAuthToken(token)
    return redirect
}
```

Cookie 格式：`X-Auth={token}; Path=/; SameSite=Lax`

### 会话失效

API 客户端（`api-client.ts`）有 401 拦截器：

```typescript
apiClient.interceptors.response.use(
    response => response,
    error => {
        if (error.response?.status === 401) handle401()
        return Promise.reject(error)
    }
)
```

`handle401()` 重置认证状态缓存，跳转到 Herald 登录页。用 `isRedirecting` 标记防止多个并发 401 触发多次跳转。

### 同域子域名模式

如果 Herald 和 rmqtt-things 在同一个根域下（比如 `auth.example.com` 和 `app.example.com`），Herald 可以把 `X-Auth` Cookie 的 domain 设为 `.example.com`，浏览器会自动在两个子域名之间共享。这种模式下不需要 `/auth/callback` 中转。

## 部署

### 前置准备

部署 rmqtt-things 的 Herald 认证功能之前：

1. Herald 服务已部署并可访问
2. 在 Herald 中创建了 realm，生成了 API Key
3. 在 Herald 中配置了 rmqtt-things 的权限点（上面列的 6 个）
4. 创建了至少一个管理员角色，分配了相应权限

### 配置

生产环境在 `config.toml`（或 `config.production.toml`）中加入：

```toml
[herald]
base_url = "http://herald:3000"       # Docker 网络内用容器名
api_key = "CHANGE_ME_HERALD_API_KEY"
realm_id = "default"
client_id = "rmqtt-things-admin"
```

`base_url` 在 Docker 部署时用容器名或服务名，不要用 `localhost`。

### 部署模式选择

**同域子域名**（推荐）：Herald 和 rmqtt-things 部署在同一根域下，Cookie 自动共享。配置简单，不需要回调中转。前端环境变量设 `VITE_APP_BASE_URL` 为管理后台地址即可。

**跨域**：Herald 和 rmqtt-things 部署在不同域名下。登录走回调页中转。前端需要额外配：
- `VITE_APP_BASE_URL` — 管理后台的完整 URL

### 不开 Herald 的场景

本地开发或内网隔离环境，不配 `[herald]` 段就行。管理端 API 不做认证校验。设备端 HMAC 认证不受任何影响。
