# Slice F — `#[pilcrow::key]` / `#[pilcrow::live]` field attrs + List broadcast producer

**Date:** 2026-05-21  
**TODOs:** #13, #14  
**Branch:** `claude/slice-f-key-live-attrs`

---

## Goal

Two complementary primitives that close the loop on keyed list live-updates:

1. **TODO #13** — `#[pilcrow(key)]` and `#[pilcrow(live)]` helper attributes on item struct
   fields; `#[derive(PilcrowListRow)]` generates a `ListRow` trait impl that the broadcast
   channel and SSE handlers use.
2. **TODO #14** — `ListBroadcast` runtime type: a clone-safe, Axum-Extension-compatible
   broadcast channel that app code calls to fan out `list-patch` SSE events to every
   connected client viewing a page with that list.

Wire format already exists from Slice E: `event: list-patch` / `{ list, key, ...fields }`.
The client-side handler already reads `[data-pilcrow-list]` + `[data-pilcrow-key]` from the DOM.
Slice F provides the server-side plumbing so apps can produce those events ergonomically.

---

## Design

### Attribute convention

Proc-macro helper attributes are registered as bare idents; path syntax (`pilcrow::key`) is
not valid in the `attributes(...)` list. We use the standard serde-style approach:

```rust
#[derive(PilcrowListRow)]
pub struct TicketRow {
    #[pilcrow(key)]
    pub id: i64,
    #[pilcrow(live)]
    pub status: String,
    pub title: String,   // rendered as static HTML; not live
}
```

The derive looks for `#[pilcrow(...)]` attrs and inspects their content for `key` and `live`.

### `ListRow` trait

```rust
pub trait ListRow: Send + Sync + 'static {
    /// Unique key for this row, e.g. "42" or "ticket:42".
    fn pilcrow_key(&self) -> String;
    /// Live-updatable fields as (field_name, json_value) pairs.
    fn pilcrow_live_fields(&self) -> Vec<(&'static str, serde_json::Value)>;
}
```

### `ListBroadcast`

```rust
pub struct ListBroadcast { ... }

impl ListBroadcast {
    pub fn new(capacity: usize) -> Self;
    /// Send a list-patch event derived from T's key + live fields.
    pub fn send_row<T: ListRow>(&self, list_name: &str, row: &T);
    /// Send a raw list-patch event without a typed row.
    pub fn send_patch(&self, list_name: &str, key: impl Into<String>, fields: impl serde::Serialize);
    /// Subscribe to receive ListPatchEvents.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ListPatchEvent>;
}

#[derive(Debug, Clone)]
pub struct ListPatchEvent {
    pub list_name: String,
    pub key: String,
    pub fields: serde_json::Value,
}
```

### Usage (developer-facing)

**Startup (app/main.rs):**
```rust
let list_broadcast = ListBroadcast::new(256);
let app = Router::new()
    .layer(Extension(list_broadcast.clone()));
```

**Mutation handler:**
```rust
async fn update_ticket(req: Req, ...) -> ActionResult {
    let ticket = db::update_ticket(id, status).await?;
    let lb = req.extensions().get::<ListBroadcast>().unwrap();
    lb.send_row("tickets", &ticket);
    ok()
}
```

**SSE handler (hand-written or future-generated):**
```rust
async fn live_tickets(
    Extension(lb): Extension<ListBroadcast>,
) -> impl IntoResponse {
    sse_stream(|emit| async move {
        let mut rx = lb.subscribe();
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    emit.send(SilcrowEvent::list_patch(ev.list_name, ev.key, ev.fields)).await?;
                }
                Err(broadcast::error::RecvError::Closed) => break Ok(()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    })
}
```

**Template (developer writes manually):**
```html
<ul data-pilcrow-list="tickets">
  {% for ticket in tickets %}
  <li data-pilcrow-key="{{ ticket.id }}">
    {{ ticket.title }}
    <span data-pilcrow-live-field="status">{{ ticket.status }}</span>
  </li>
  {% endfor %}
</ul>
```

---

## Tasks

| # | File | Work |
|---|------|------|
| F1 | `pilcrow/crates/runtime/src/live_props/list_row.rs` | NEW — `ListRow` trait |
| F2 | `pilcrow/crates/runtime/src/live_props/list_broadcast.rs` | NEW — `ListBroadcast`, `ListPatchEvent` |
| F3 | `pilcrow/crates/runtime/src/live_props/mod.rs` | Export `ListRow`, `ListBroadcast`, `ListPatchEvent` |
| F4 | `pilcrow/crates/runtime/src/lib.rs` | Re-export under `live-props` feature gate |
| F5 | `pilcrow/crates/macros/src/list_row_derive.rs` | NEW — `PilcrowListRow` derive expansion |
| F6 | `pilcrow/crates/macros/src/lib.rs` | Register `PilcrowListRow` derive + `pilcrow` helper attr |
| F7 | `pilcrow/registry.toml` | Add `key-live-field-attrs` + `list-broadcast-producer` entries |
| F8 | `plans/implementation-progress.md` | Mark #13 + #14 done |

---

## Out of scope for Slice F

- Codegen auto-injection of `data-pilcrow-list` / `data-pilcrow-key` into `{% for %}` loops
  (requires for-loop variable tracking in the template AST — deferred to Slice H)
- Generated SSE route integration (the live-props SSE route auto-subscribing to `ListBroadcast`
  for `Vec<T: ListRow>` fields in Props — deferred to Slice H alongside windowed baking)

Developers write `data-pilcrow-list` and `data-pilcrow-key` manually in templates for now.
The `list-patch-wire-format` registry entry from Slice E already documents this.
