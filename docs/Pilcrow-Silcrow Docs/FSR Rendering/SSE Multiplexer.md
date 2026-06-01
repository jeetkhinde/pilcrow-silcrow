# SSE Multiplexer

FSR uses a single Server-Sent Events connection per page. All live field updates for a route flow over one `EventSource` — no matter how many `LiveProp` fields the page has.

## Connection URL

The client opens:

```
/__pilcrow/fsr?route=/tickets&slots=status,priority,volume
```

- `route` — the current page path (must match the route registered in `__pilcrow_init`)
- `slots` — comma-separated list of `LiveProp` field names the page cares about

The Pilcrow JS runtime builds this URL at page load and manages the `EventSource` lifetime automatically.

## How slot filtering works

`fsr_hub_handler` (in `crates/runtime/src/fsr/hub.rs`) holds a `broadcast::Receiver<SlotPatch>`. On each broadcast it:

1. Checks `patch.route == subscribed_route` — drops the patch if the route does not match.
2. Checks `patch.slot ∈ subscribed_slots` — drops the patch if the slot is not in the query string.
3. Serialises the surviving patch and writes it as an SSE event.

This means the broadcast channel is global (all routes share it), but each connected client only receives events for its own route and the slots it declared.

## SSE event shapes

### Normal field update

```
event: fsr
data: {"status": "Closed"}
```

Each `fsr` event carries **one or more** slot → value pairs as a JSON object. The client applies them in one DOM pass.

**Scalar slots** (strings, numbers, booleans) are patched directly into `textContent` of elements carrying `[s-live="slot_name"]`.

**Object/array slots** are published as Silcrow atoms:

```js
window.Silcrow.publish('fsr.slot_name', value)
```

Components using `s-use` bindings pick up the new value reactively.

### Resync on buffer overflow

When the broadcast ring-buffer overflows (a slow client falls too far behind):

```
event: fsr-resync
data: {}
```

The client re-fetches a full snapshot from `/__pilcrow/fsr/snapshot` and re-renders the page from scratch.

## Connection lifecycle

- The server closes the connection after `connection_ttl_secs` (default 3600 s).
- The browser `EventSource` auto-reconnects with standard exponential back-off.
- Keepalive comments (`: ping`) are written periodically so proxies do not time out idle connections.

## Data flow

```
DB row changes
      │
      ▼
pilcrow_fsr table updated
      │
      ▼
watcher tick → dep key invalidated
      │
      ▼
broadcast::send(SlotPatch { route, slot, value })
      │
      ▼
fsr_hub_handler
  filter: patch.route == subscribed_route
  filter: patch.slot ∈ subscribed_slots
      │
      ▼
SSE event → client JS
  scalar  → patch textContent of [s-live="slot"]
  object  → Silcrow.publish('fsr.slot', value)
      │
      ▼
DOM updated
```

## End-to-end example

### `live.rs`

```rust
use pilcrow_web::prelude::*;

#[derive(Debug, Default)]
pub struct Live {
    pub status: LiveProp<String>,
    #[revalidate(60)]
    pub price: LiveProp<f64>,
}

pub async fn live_fields(ctx: &Page) -> Live {
    let ticket_id = ctx.param("id").unwrap_or_default();
    let mut live = Live::default();

    live.status = ctx
        .db()
        .query_scalar("SELECT status FROM tickets WHERE id = $1", &[&ticket_id])
        .await
        .unwrap_or_default()
        .into();

    live.price = fetch_market_price(&ticket_id).await.into();
    live
}
```

### HTML template

```html
<span s-live="status">{{ live.status }}</span>
<span s-live="price">{{ live.price }}</span>
```

The client subscribes to `slots=status,price` over one SSE connection. When `status` changes, only the `status` slot event is sent; the `price` element is untouched.

## Relevant source files

| File | What it contains |
|------|-----------------|
| `crates/runtime/src/fsr/hub.rs` | `fsr_hub_handler` — SSE loop, slot filtering, resync logic |
| `crates/runtime/src/fsr/watcher.rs` | Dep-key watcher; broadcasts `SlotPatch` on invalidation |
| `crates/runtime/src/fsr/baking.rs` | Snapshot baking for initial render and resync |
