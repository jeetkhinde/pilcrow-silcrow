# Slice H — Windowed baking, HTML chunk cache, Vec<T> canonical list DX

**Date:** 2026-05-21  
**TODOs:** #17, #18, #19  
**Branch:** `claude/slice-h-windowed-baking`

---

## Goal

1. **TODO #17** — `prebake_next(path)`: when a paginated route serves a cursor window,
   the handler can call this to trigger a fire-and-forget background GET to the *next*
   window URL, warming the FSR/HTTP cache before the user scrolls there.

2. **TODO #18** — `ListChunkCache`: a trait + `InMemoryListChunkCache` impl for
   per-row pre-baked HTML storage. Rows are stored under `pilcrow:chunk:{list}:{key}`.
   When a row changes (via `ListBroadcast`), callers invalidate the stale chunk.
   Redis-backed impl deferred to the `live-props-redis` feature.

3. **TODO #19** — Design lock: `Vec<T: ListRow>` is the canonical list DX.
   No `LiveList<T>` or `AppendList<T>` types exist or will be added.
   Documented in registry; the `ListRow` derive + `ListBroadcast` pair is the full surface.

---

## Design

### TODO #17 — `prebake_next`

A global base-URL registry plus a fire-and-forget spawn:

```
start_with_adapter
  └─ prebake::set_local_base("http://127.0.0.1:{port}")

page handler (after returning the current page)
  └─ req.fsr.prebake_next("/tickets?cursor=next_token")
       └─ prebake::trigger(path)
            └─ tokio::spawn(reqwest::get(base + path))
```

The response is silently discarded. If the base URL hasn't been set the call is a
no-op. Tests verify `set_local_base` / `trigger` with a mock server.

### TODO #18 — `ListChunkCache`

```rust
pub trait ListChunkCache: Send + Sync + 'static {
    fn get(&self, list_name: &str, key: &str) -> Option<String>;
    fn set(&self, list_name: &str, key: &str, html: String);
    fn invalidate(&self, list_name: &str, key: &str);
    fn invalidate_list(&self, list_name: &str);
}

pub struct InMemoryListChunkCache { ... }  // Arc<RwLock<HashMap<(String,String), String>>>
```

Key scheme (for future Redis mapping): `pilcrow:chunk:{list_name}:{row_key}`.

Usage:
```rust
// In a page handler or SSE handler:
let cache: &InMemoryListChunkCache = req.extensions().get().unwrap();
if let Some(html) = cache.get("tickets", &id) {
    // serve pre-baked row
} else {
    let rendered = render_ticket(&ticket);
    cache.set("tickets", &id, rendered.clone());
    // use rendered
}

// When a row changes:
lb.send_row("tickets", &row);
cache.invalidate("tickets", &row.id.to_string());
```

### TODO #19 — Design lock

No new types. Registry entry records the decision. The surface for lists is:
- `#[derive(PilcrowListRow)]` → `ListRow` trait
- `ListBroadcast` → push `list-patch` events
- Developer writes `data-pilcrow-list` / `data-pilcrow-key` in HTML templates

---

## Tasks

| # | File | Work |
|---|------|------|
| H1 | `crates/runtime/src/prebake.rs` | NEW — `set_local_base`, `trigger`, `LOCAL_BASE: OnceLock` |
| H2 | `crates/runtime/src/fsr/handle.rs` | Add `prebake_next(path)` forwarding to `prebake::trigger` |
| H3 | `crates/runtime/src/live_props/list_chunk.rs` | NEW — `ListChunkCache` trait + `InMemoryListChunkCache` |
| H4 | `crates/runtime/src/live_props/mod.rs` | Export `ListChunkCache`, `InMemoryListChunkCache` |
| H5 | `crates/runtime/src/lib.rs` | Re-export `prebake::trigger as prebake_next`; `ListChunkCache`, `InMemoryListChunkCache` |
| H6 | `crates/runtime/src/start.rs` | Call `prebake::set_local_base` at startup |
| H7 | `pilcrow/registry.toml` | 3 new entries |
| H8 | `plans/implementation-progress.md` | TODOs #17–#19 marked done |
