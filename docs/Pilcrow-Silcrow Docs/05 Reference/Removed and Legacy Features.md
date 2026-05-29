# Removed and Legacy Features

Do not use these in new examples.

## Removed Rendering Surfaces

- `REVALIDATE`
- `MAX_STALE`
- `CACHE_TAGS`
- `CACHE_VARY`
- `STREAMING`
- `Deferred<T>`
- `DeferredHtml`
- `PRERENDER`
- static export

These were replaced by the current SSR, FSR, Live Props, and island model.

## Current Replacements

| Legacy surface | Replacement |
| --- | --- |
| `REVALIDATE` | FSR scheduled invalidation with `#[revalidate(N)]` or `[fsr] revalidate_seconds` |
| `STREAMING` | Live Props over SSE |
| `Deferred<T>` / `DeferredHtml` | Live Props or islands, depending on UX |
| `PRERENDER` | FSR `PROMOTE_AFTER = 0` for bake-on-first-hit behavior |
| static export | current runtime serving model |

## Documentation Note

Some older repository guidance still mentions legacy ISR or prerender behavior. Treat the current rendering model and registry status as authoritative, and clean stale references when touching those files.
