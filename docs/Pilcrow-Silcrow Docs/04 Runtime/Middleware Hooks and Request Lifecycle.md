# Middleware Hooks and Request Lifecycle

## Hooks

Optional lifecycle hooks live in app-root `hooks.rs`.

Hook signatures must match the framework contract exactly. Routekit auto-detects `hooks.rs` and wires supported hooks into generated routes.

## Middleware

Middleware also lives in `hooks.rs`.

Rules:

- The function must be named `middleware`.
- It must be `pub async fn middleware`.
- No other location is auto-detected.

Use middleware for request-wide behavior such as auth context, locals, and common response shaping.

## Request Context

`Req` is the primary request object. It carries:

- params
- query
- form
- cookies
- headers
- path
- enhanced-request flag
- locals
- response modifier handle

`Locals` is a typed per-request store.

`Res` accumulates response side effects such as status, headers, cookies, events, navigation, patches, SSE, and WebSocket upgrades.
