# Rendering Models

## Current Modes

| Mode | Trigger | Notes |
| --- | --- | --- |
| SSR | default | `load()` runs on every request. |
| FSR | `PROMOTE_AFTER: u32 = N` | Promotes a route after N hits. |
| FSR bake on first hit | `PROMOTE_AFTER: u32 = 0` | First request bakes the route. |
| Live Props | `LiveProp<T>` field | Initial SSR value plus SSE updates. |
| Pilcrow islands | `<island>` | Server-rendered fragment fetched by strategy. |
| React islands | `<react>` | React view layer mounted by strategy. |

## Removed Modes

These are legacy surfaces and should not appear in new docs or examples:

- `REVALIDATE`
- `STREAMING`
- `Deferred<T>`
- `DeferredHtml`
- `PRERENDER`
- static export

Use FSR scheduled invalidation and Live Props instead of legacy ISR or streaming constants.

## FSR Ownership Boundary

`load()` does only data fetching and returns `Props`. The framework owns the FSR lifecycle — slot registration, hit counting, baking, watcher re-execution. The app's entire FSR write surface is:

- `s-live="slot_name"` on HTML elements (DOM patch targets)
- `req.fsr.invalidate_route(path)` after a mutation changes live data
- `req.fsr.tombstone(path)` after a delete

Never write files to `.pilcrow-baked/`, never call `UPDATE pilcrow_fsr SET promoted = TRUE` directly, and never call bake functions from `load()`. The generated handler always runs `load()` — there is no promoted-route fast path that skips it.

## `PROMOTE_AFTER` and Dynamic Routes

`PROMOTE_AFTER: u32 = N` is only coherent on **static or near-static routes** with a finite, known set of URL instances (e.g. `/`, `/about`).

On a dynamic route like `/contacts/[id]`, every distinct URL gets its own `pilcrow_fsr` row. Setting `PROMOTE_AFTER = 0` marks every contact URL as `promoted = TRUE` on first hit, but `html_path` stays `NULL` because the framework has no way to pre-bake an open ID-space. The watcher logs warnings on every invalidation cycle for every such URL. Do not use `PROMOTE_AFTER` on dynamic routes.

## Default SSR

SSR is the baseline. A page with `load()` runs it per request. A page without code-behind renders as a static template.

```rust
pub async fn load(req: Req) -> AppResult<Props> {
    Ok(Props { /* ... */ })
}
```

No constant is required.
