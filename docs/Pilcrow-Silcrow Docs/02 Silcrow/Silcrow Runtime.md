# Silcrow Runtime

Silcrow is the browser runtime bundled by Pilcrow.

It is a zero-dependency IIFE that exposes `window.Silcrow` and boots on `DOMContentLoaded`.

## Subsystems

- Runtime: DOM patching, bindings, spread, keyed loops.
- Atoms: reactive stores with route, stream, and custom scopes.
- Navigator: enhanced links, forms, history, preloading, response caching.
- Live: SSE and WebSocket connection management.
- Optimistic: snapshot, optimistic patch, confirm, rollback.

## Build Pipeline

Edit only:

```text
silcrow/src/silcrow.js
```

Then run:

```bash
node silcrow/build.js
```

The build emits `silcrow/dist/*` and copies the minified runtime to:

```text
pilcrow/crates/runtime/assets/silcrow.js
```

Pilcrow embeds that file at compile time.

## Public API

The public API is documented in `silcrow/docs/silcrow-api.md`. Treat it as authoritative for directives, attributes, headers, events, and `window.Silcrow`.
