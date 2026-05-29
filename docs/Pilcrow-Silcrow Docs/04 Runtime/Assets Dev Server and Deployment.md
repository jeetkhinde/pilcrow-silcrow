# Assets Dev Server and Deployment

## Runtime Asset

Silcrow is served from the Pilcrow runtime namespace:

```text
/__pilcrow/runtime/silcrow.{hash}.js
```

Use the framework helper for script tags instead of hardcoding this path. That allows `inline_runtime` to work.

## Inline Runtime

When `[client] inline_runtime = true` is set in `Pilcrow.toml`, the asset helper emits an inline script instead of a script `src`.

This is process-wide startup configuration, not a per-request switch.

## Dev Server

`pilcrow dev` runs the app in dev mode.

Stable dev features:

- live reload
- CSS hot swap
- build status banner

CSS hot swap requires stylesheet loading through a `<link rel="stylesheet">` element.

## Deployment Adapters

Pilcrow supports a pluggable adapter trait. Built-in adapters include default Tokio and platform adapters such as Lambda, Fly, Railway, Cloud Run, Render, Vercel, and bare metal.

Lambda requires the `lambda` feature on `pilcrow-web`.
