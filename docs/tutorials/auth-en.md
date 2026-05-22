# Authentication and Permissions

The system has two independent authentication mechanisms -- one for devices and one for the admin backend. They don't interact with each other.

| | Device Side | Admin Side |
|--|-------------|------------|
| Auth method | HMAC-SHA1 | Herald SSO |
| Protected scope | MQTT connections | Admin HTTP API (`/api/admin/*`) |
| Credential storage | Device-generated password | Herald manages sessions |
| Code location | `auth_handlers.rs` | `middleware/mod.rs` |

For details on device-side HMAC authentication, see the "Device Authentication and ACL Flow" section in the [architecture doc](architecture.md). This document covers only the admin-side Herald SSO integration.

## What is Herald

Herald is a standalone authentication service that handles user registration, login, session management, and permission definitions. rmqtt-things does not store user information -- it only verifies session tokens and permissions through the Herald SDK.

The relationship between the two services:

```
Browser -> Caddy -> rmqtt-things App -> Herald SDK -> Herald Service
                   (backend API)                      (verify token + check permissions)
```

rmqtt-things never connects directly to Herald's database. All authentication and permission queries go through Herald's HTTP API (`POST /api/ext/permission/check`). The SDK maintains a roughly 5-minute cache internally, so not every request hits Herald.

## Backend Integration

### Configuration

Add a `[herald]` section to `config.toml`:

```toml
[herald]
base_url = "http://127.0.0.1:3000"  # Herald service address
api_key = "your-api-key"            # API key for Herald ext API calls
realm_id = "rmqtt"                # realm that rmqtt-things belongs to
client_id = "admin-web-console"    # client identifier
```

Without this section, the admin API runs without authentication. This is fine for local development but must be configured in production.

During startup, `main.rs` initializes the Herald SDK client based on this config:

```rust
let herald_client = config.herald.as_ref().map(|herald| {
    Arc::new(herald_sdk::Client::new(
        herald.base_url.clone(),
        herald.api_key.clone(),
        None,
    ))
});
```

### Middleware

The `herald_auth_middleware` in `middleware/mod.rs` is an Axum middleware function that runs on all Admin API requests. Its flow:

```
Request arrives
  -> Extract X-Auth token from Cookie
    -> No token -> 401 Unauthorized
  -> Generate permission rule from request path and HTTP method (resource + action)
    -> Path not in protected scope -> 403 Forbidden
  -> Call Herald SDK to check permission (with caching)
    -> allowed=true  -> Inject user_id into request extensions, pass through
    -> allowed=false -> 403 Forbidden
    -> Herald unavailable -> 503 Service Unavailable
```

When Herald is unavailable, the middleware returns 503 rather than letting the request through. It is better to have the admin backend temporarily unavailable than to allow unauthenticated requests.

### Permission Model

Permissions have two dimensions: resource and action.

**Resource mapping** (request path to resource identifier):

| Path prefix | Resource | Admin interfaces covered |
|-------------|----------|--------------------------|
| `/admin/product*`, `/admin/valid*`, `/admin/file*` | `product` | Products, validation templates, file uploads |
| `/admin/device*`, `/admin/property*`, `/admin/event*` | `device` | Device status, properties, events, commands |
| `/admin/ca*`, `/admin/ota*` | `cert` | Certificate issuance/revocation, OTA versions |

**Action mapping** (HTTP method to action identifier):

| HTTP method | Action |
|-------------|--------|
| GET | `read` |
| POST, PUT, PATCH, DELETE | `write` |

**The complete set of 6 permission points:**

| Permission | Description |
|------------|-------------|
| `product:read` | View products and validation templates |
| `product:write` | Create/edit products and templates, upload files |
| `device:read` | View device status, properties, events, commands |
| `device:write` | Send and delete property commands |
| `cert:read` | View certificates and OTA versions |
| `cert:write` | Issue/revoke certificates, manage OTA versions |

These permission points must be configured in Herald ahead of time as roles and rules. rmqtt-things does not manage role definitions itself -- it only asks Herald "can this user perform this action?"

### Route Protection Scope

In `api/mod.rs`, the `create_router()` function applies the middleware only when Herald is configured:

```rust
let admin_routes = match (config.herald.as_ref(), herald_client) {
    (Some(herald_config), Some(herald_sdk)) => {
        admin_routes.layer(axum::middleware::from_fn_with_state(
            HeraldAuthState { herald_sdk, client_id: herald_config.client_id.clone().into() },
            herald_auth_middleware,
        ))
    }
    (_, _) => admin_routes,  // No herald config, admin routes run without auth
};
```

Routes that are not affected:

- `/api/thing/*` -- device WebHook callbacks
- `/api/access/*` -- device authentication and ACL
- `/api/device/*` -- device online/offline notifications
- `/api/health` -- health check
- `/api/auth/config` -- auth configuration query (the frontend uses this to detect whether Herald is enabled)

## Frontend Flow

### Login Redirect

When a user opens the admin backend, the `beforeLoad` hook in `__root.tsx` checks authentication status:

```typescript
beforeLoad: async () => {
    const authed = await checkAuth()
    if (!authed) {
        handle401()  // Redirect to Herald login page
        throw new Error('unauthenticated')
    }
}
```

`checkAuth()` first queries `/api/auth/config` to see if Herald is enabled. If not, it passes through immediately. If Herald is enabled, it sends a probe request to `/api/admin/product?page=1&page_size=1` and checks the response status to determine whether the session is valid.

When unauthenticated, the browser redirects directly to Herald's login page. The backend `GET /api/auth/config` returns the complete login URL:

```json
{"enabled": true, "login_url": "http://herald.example.com/default/auth/login"}
```

The URL is assembled by the backend from `herald.base_url` and `herald.realm_id` in the format `{base_url}/{realm_id}/auth/login`. The frontend does no URL construction.

### Authentication Mechanism

When Herald and rmqtt-things are deployed on the same host (or same root domain), browser cookies are shared across ports/subdomains. After the user logs in at Herald, the `X-Auth` cookie set by Herald is automatically included in requests to rmqtt-things. The user simply returns to the rmqtt-things page after logging in -- no callback intermediary is needed.

### Session Expiry

The API client (`api-client.ts`) has a 401 interceptor:

```typescript
apiClient.interceptors.response.use(
    response => response,
    error => {
        if (error.response?.status === 401) handle401()
        return Promise.reject(error)
    }
)
```

`handle401()` resets the auth state cache and redirects to the Herald login page. An `isRedirecting` flag prevents multiple concurrent 401 responses from triggering duplicate redirects.

## Deployment

### Prerequisites

Before deploying rmqtt-things with Herald authentication:

1. Herald service is deployed and accessible
2. A realm has been created in Herald and an API Key generated
3. rmqtt-things permission points (the 6 listed above) have been configured in Herald
4. At least one admin role has been created with the appropriate permissions assigned

### Configuration

For production, add the following to `config.toml` (or `config.production.toml`):

```toml
[herald]
base_url = "http://herald:3000"       # Use container name within Docker network
api_key = "CHANGE_ME_HERALD_API_KEY"
realm_id = "rmqtt"
client_id = "admin-web-console"
```

When deploying with Docker, use the container name or service name for `base_url`, not `localhost`.

### Deployment Requirements

Herald and rmqtt-things must be deployed on the same host or under the same root domain so that browsers share cookies. Typical deployment patterns:

- Same host, different ports (e.g., `127.0.0.1:13000` and `127.0.0.1:3000`)
- Reverse proxy with a unified entry point (e.g., Caddy/Nginx routing `/auth` to Herald and `/` to rmqtt-things)
- Same root domain subdomains (e.g., `auth.example.com` and `app.example.com`)

### Running Without Herald

For local development or isolated intranet environments, simply omit the `[herald]` section. The admin API will not perform authentication checks. Device-side HMAC authentication is not affected in any scenario.
