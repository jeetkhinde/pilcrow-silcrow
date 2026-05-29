# Feature Status

This page is a practical status index derived from `pilcrow/registry.toml` and the local MCP feature registry.

## Stable Core

- SSR pages
- File routing
- Layouts
- Route groups
- Fragments
- API routes
- Named actions
- Server hooks
- Middleware
- Typed route helpers
- Typed param matchers
- Typed env config
- Loading skeletons
- Page options
- Form parsing and form validation
- Test request builder

## Stable Rendering and Client Integration

- Live Props
- FSR, including scheduled revalidation and promotion thresholds
- Bake on first hit through FSR with `PROMOTE_AFTER = 0`
- Pilcrow islands
- React islands
- Keyed list patch wire format
- `PilcrowListRow`
- `ListBroadcast`
- `ListChunkCache`
- Canonical `Vec<T: ListRow>` list DX
- Silcrow runtime URL unification
- Inline runtime config
- Windowed prebake

## Stable Runtime and Operations

- CSRF protection
- Graceful shutdown
- Request timeout
- Request body limit
- Request tracing
- Response compression
- Service worker
- i18n
- Image optimization
- Head/meta management
- Adapter trait and platform adapters
- Dev server
- CSS hot swap
- Dev build status banner

## Experimental

- [[../05 Reference/Experimental Baked Pages]]

## Removed or Legacy

See [[../05 Reference/Removed and Legacy Features]].

Important removed surfaces include:

- `REVALIDATE`
- `STREAMING`
- `Deferred<T>` and `DeferredHtml`
- `PRERENDER`
- static export

Some older local notes still mention legacy ISR and prerender behavior. Treat the current rendering docs and registry status as authoritative until those stale notes are cleaned up.
