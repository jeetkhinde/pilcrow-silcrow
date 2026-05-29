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

## Default SSR

SSR is the baseline. A page with `load()` runs it per request. A page without code-behind renders as a static template.

```rust
pub async fn load(req: Req) -> AppResult<Props> {
    Ok(Props { /* ... */ })
}
```

No constant is required.
