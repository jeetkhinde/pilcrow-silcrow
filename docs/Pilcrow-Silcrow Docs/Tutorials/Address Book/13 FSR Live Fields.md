# 13 — FSR Live Fields

This step explains the FSR system that makes `total_contacts`, `favorite_mark`, and `updated_label` update in the browser without a page reload.

## What FSR is

FSR (Field-Selective Rendering) is Pilcrow's live-update system. It tracks specific database-backed fields per route, re-executes stored SQL queries when those fields go stale, and pushes patches to connected browsers via SSE.

The generated page handler always runs `load()`. FSR adds a parallel layer on top — slot values update in the DOM asynchronously without re-rendering the page.

## The three surfaces

### 1. `s-live="slot_name"` in the template

The DOM element where SSE patches land.

```html
<span s-live="total_contacts">{{ live.total_contacts.value }}</span>
```

`{{ live.total_contacts.value }}` renders the initial value at SSR time. `s-live` is the patch target for all subsequent SSE updates.

**Important:** `s-live` patches only `textContent`. It does not update CSS classes or attributes. If a live field controls both text and a class (e.g. a badge that changes colour), use the object path — see [[../../03 Rendering/FSR SSE Hub]].

### 2. `req.fsr.invalidate_route(path)` in actions

Marks all slots for a route as stale. The watcher detects this and re-runs their stored SQL queries.

```rust
req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
```

Always pass the concrete URL path including the param value, not the route pattern.

### 3. `Live::query()` in the code-behind

The SQL that the watcher re-executes when a slot is stale. The query params are stored in `pilcrow_fsr` when the route first receives a request. The watcher needs no additional configuration.

```rust
impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("contact_id").and_then(|v| v.as_str()).unwrap_or("");
        live_query!(
            "SELECT
               CASE WHEN favorite THEN '*' ELSE '☆' END AS favorite_mark,
               'Updated ' || ... AS updated_label
             FROM contacts WHERE id = $1",
            id
        )
    }
}
```

## Full flow

```
User submits favourite form
  → action: set_favorite() in DB
  → req.fsr.invalidate_route("/contacts/kent-c-dodds")
  → pilcrow_fsr: favorite_mark and updated_label marked stale = TRUE
  → FSR watcher (Redis pub/sub on pilcrow:invalidate; falls back to
     500ms polling without Redis — WatcherConfig::poll_interval_ms default):
      re-runs Live::query("contact_id" = "kent-c-dodds")
      → new values: favorite_mark = "*", updated_label = "Updated 2026-05-29..."
      → publishes SlotPatch events to broadcast channel
      → marks slots stale = FALSE
  → fsr_hub_handler (SSE):
      filters SlotPatch for /contacts/kent-c-dodds
      → sends event: fsr
         data: {"favorite_mark": "*"}
      → sends event: fsr
         data: {"updated_label": "Updated 2026-05-29..."}
  → Silcrow in the browser:
      patches [s-live="favorite_mark"] textContent = "*"
      patches [s-live="updated_label"] textContent = "Updated 2026-05-29..."
```

## FSR shape for this app

| Route | Live slots | Invalidated by |
|---|---|---|
| `/` | `total_contacts` | `create`, `destroy` |
| `/contacts/[id]` | `favorite_mark`, `updated_label` | `favorite`, `save`, `destroy` |
| `/contacts/[id]/edit` | `updated_label` | `save`, `favorite` |

## `depends_on_route`

```rust
#[pilcrow::depends_on_route(contacts, contact_id)]
pub struct Live { ... }
```

This wires each slot's `depends_on` to `contacts:contact_id=<value>`. It means you can also invalidate by dep key if needed:

```rust
fsr_store.invalidate_dep_key("contacts:contact_id=kent-c-dodds").await.ok();
```

For the address-book `invalidate_route` is simpler and sufficient.

## Reset FSR state

If you change slot definitions, truncate the FSR table and let it repopulate:

```sql
TRUNCATE pilcrow_fsr;
```

---

Next: [[14 Live Timestamps on the Edit Page]]
