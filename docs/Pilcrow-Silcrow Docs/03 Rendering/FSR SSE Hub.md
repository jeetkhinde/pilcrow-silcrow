# FSR SSE Hub

FSR uses one browser `EventSource` per route. That connection carries patch events for every live slot the current page renders.

## Connection Shape

When a page contains `s-live` slots, routekit injects a small client script. On page load it scans the DOM for slot names and opens:

```text
/__pilcrow/fsr?route=/tickets&slots=total_count,open_count,critical_count
```

The route is the current browser path. The slots are the `s-live` names found in the rendered HTML. If Silcrow navigates to another page, the script reconnects with the new route and slot set.

## Server Filtering

The embedded watcher sends `SlotPatch` values to a shared broadcast channel:

```rust
pub struct SlotPatch {
    pub route: String,
    pub slot: String,
    pub value: serde_json::Value,
}
```

`fsr_hub_handler` subscribes to that broadcast channel for each connected browser. It drops every patch where:

- `patch.route` does not match the subscribed route.
- `patch.slot` is not in the subscribed slot list.

Each accepted patch is sent as an `fsr` SSE event. A single patch event carries one slot value:

```text
event: fsr
data: {"open_count": 12}
```

Snapshot responses can contain multiple slots because the client asks for all visible slots during resync.

## Client Patching

Scalar values patch direct text slots:

```html
<strong>{{ live.open_count.value }}</strong>
```

Routekit rewrites that text-node use into an `s-live` slot:

```html
<span s-live="open_count">{{ live.open_count.value }}</span>
```

When an `fsr` event arrives, scalar values update `textContent` for matching `[s-live="open_count"]` elements.

Object values are published as Silcrow atoms instead:

```rust
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct PriorityBadge {
    pub text: String,
    pub class: String,
}

pub struct Live {
    #[pilcrow::allow_unused]
    pub priority: LiveProp<PriorityBadge>,
}
```

```html
<span class="{{ live.priority.value.class }}" s-use="fsr.priority">
  {{ live.priority.value.text }}
</span>
```

The client publishes object patches to `Silcrow.publish("fsr.priority", value)`, and normal Silcrow bindings update the element.

## Resync and Connection Lifetime

The FSR hub uses a broadcast ring buffer. If a browser falls behind and misses events, the server sends:

```text
event: fsr-resync
data: lagged
```

The client responds by fetching:

```text
/__pilcrow/fsr/snapshot?route=/tickets&slots=total_count,open_count,critical_count
```

The snapshot handler re-runs the stored slot queries and returns the current values as JSON.

Connections close after `[fsr].connection_ttl_secs` seconds. The default is `3600`. Browser `EventSource` reconnects automatically. Keepalive comments are sent every `[fsr].keepalive_secs` seconds; the default is `30`.

## End-to-End Example

Page code-behind:

```rust
use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub struct Live {
    pub open_count: LiveProp<i64>,
    pub critical_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT \
             COUNT(*) FILTER (WHERE status = 'open')::bigint AS open_count, \
             COUNT(*) FILTER (WHERE priority = 'critical')::bigint AS critical_count \
             FROM tickets"
        )
    }
}

pub async fn load(live: Live) -> AppResult<Props> {
    Ok(Props { live })
}
```

Template:

```html
<section>
  <p>Open: <strong>{{ live.open_count.value }}</strong></p>
  <p>Critical: <strong>{{ live.critical_count.value }}</strong></p>
</section>
```

Data flow:

```text
DB row changes
  -> pilcrow_fsr slot marked stale
  -> watcher tick re-runs LiveQuery
  -> watcher sends SlotPatch { route, slot, value }
  -> fsr_hub_handler filters by route and subscribed slots
  -> event: fsr
  -> client updates [s-live] or publishes fsr.<slot>
```

## Code References

- `pilcrow/crates/runtime/src/fsr/hub.rs`
- `pilcrow/crates/runtime/src/fsr/watcher.rs`
- `pilcrow/crates/runtime/src/fsr/baking.rs`
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs`
