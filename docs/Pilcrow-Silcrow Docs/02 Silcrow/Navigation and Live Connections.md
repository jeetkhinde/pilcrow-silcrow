# Navigation and Live Connections

## Navigation Attributes

Silcrow enhances links and forms with HTTP verb attributes:

- `s-get`
- `s-post`
- `s-put`
- `s-patch`
- `s-delete`

Navigation modifiers:

- `s-target`
- `s-timeout`
- `s-html`
- `s-skip-history`
- `s-preload`
- `no-boost`

## Request and Response Headers

Enhanced requests include `silcrow-target`.

Servers can drive client behavior with response headers such as:

- `silcrow-patch`
- `silcrow-invalidate`
- `silcrow-navigate`
- `silcrow-sse`
- `silcrow-trigger`
- `silcrow-retarget`
- `silcrow-push`
- `silcrow-cache`
- `silcrow-full-reload`

## Live Connections

SSE uses:

- `s-sse`
- `data-pilcrow-live`

Pilcrow auto-injects `data-pilcrow-live` for pages with `LiveProp<T>` fields. Application templates should not write it manually.

WebSocket attributes exist, but the current Silcrow notes identify the WS path as broken until missing state registration helpers are added. Prefer SSE in examples.
