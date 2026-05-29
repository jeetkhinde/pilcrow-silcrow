# FSR SSE Hub

FSR uses one browser `EventSource` per route. That connection carries patch events for every live slot the current page renders.

## SSE Reconnect Lifecycle

The injected FSR client script splits its reconnect work across two Silcrow navigation events:

| Event | When it fires | What the FSR script does |
|---|---|---|
| `silcrow:navigate` | Before fetch, before DOM swap, before `history.pushState` | Close old SSE connection. Capture destination URL from `e.detail.url`. |
| `silcrow:load` | After DOM swap + `pushState` | Reopen SSE with `window.location.pathname` (now the new URL) and `__fsr_slots()` scanning the updated DOM. |

**Why this matters:** `silcrow:navigate` fires before the page content has changed. Calling `__fsr_slots()` at that point returns the old page's `s-live` elements. If you were on an index page with `s-live="total_contacts"` and navigated to a contact detail page, the FSR connection would subscribe to `total_contacts` on the contact route — wrong slots, no patches received.

The `silcrow:load` event fires after Silcrow has applied the PS fragment or full-page swap and after `pushState` has run. At that point `window.location.pathname` is the new route and `querySelectorAll('[s-live]')` returns the new page's live elements. The FSR connection is opened with correct route and correct slots every time.

The guard `if(slots) __fsr_connect()` prevents a wasted SSE connection when navigating to a non-FSR page (no `s-live` elements → empty string → falsy → no connection).

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

**Scalar limitation:** `s-live` patches only `textContent`. It does not update attributes, CSS classes, or `src`. If a live field controls both displayed text and a CSS class (e.g. a badge that changes colour on status change), use the object path instead. Routekit only auto-inserts `s-live` for text-node uses of `{{ live.field.value }}`; attribute expressions like `class="badge-{{ live.field.value }}"` are rendered at SSR time and are never patched.

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

**When to use object vs scalar:**

| Need | Use |
|---|---|
| Patch a text node only | Scalar `LiveProp<T>` — auto `s-live` via routekit |
| Patch both text and CSS class | Object `LiveProp<Badge>` with `text` + `class` fields + `s-use="fsr.slot"` |
| Patch multiple attributes at once | Object + `s-use` |
| Drive a Silcrow atom (any shape) | Object + `s-use` |

The demo's `StatusBadge { text, class }` and `PriorityBadge { text, class, raw }` are canonical examples of the object pattern.

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
