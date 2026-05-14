# FSR — Field-Selective Rendering: Design Decisions & DX Recap

Use this document to resume the FSR design session.

---

## What FSR is

FSR is Pilcrow's original rendering paradigm. No other framework offers it.
It extends SSR/ISR/SSG with **field-level granularity**:

- Static fields baked directly into HTML — never tracked
- Watched fields (`LiveProps<T>`) get a shell slot in HTML, a DB cache row, and live SSE updates
- One integer controls the full rendering lifecycle per field

---

## Rendering lifecycle — single unified model

```
promote_after = 0 or absent  → SSG  (bake at startup, surgical patch on dep change)
promote_after = 1            → ISR  (bake after first request, surgical patch on dep change)
promote_after = N            → FSR  (bake after N hits, surgical patch on dep change)

No live.rs file              → pure SSR (existing Pilcrow behaviour, untouched)
```

All three modes receive surgical patch on dep change. The only difference is when the first bake happens.

---

## Locked decisions

### Data model
- Static fields → baked into HTML, never stored anywhere
- LiveProps fields → shell slot in HTML + `pilcrow_fsr` DB row
- No `value` column in DB — source of truth stays in real DB tables
- `query` + `query_params` stored for re-execution when `stale = TRUE`
- Watcher sees `stale = TRUE` → re-executes query → re-bakes HTML/JSON → clears stale

### DB — single table
One table only: `pilcrow_fsr`. No separate `pilcrow_routes` table.
`slot = ''` row = route-level metadata. `slot = 'field_name'` = slot-level row.

```sql
pilcrow_fsr (
    route           TEXT,
    slot            TEXT,           -- '' for route-level
    query           TEXT,           -- SQL to re-execute when stale
    query_params    JSONB,
    depends_on      TEXT[],
    stale           BOOLEAN,
    version         INT,
    hit_count       INT,
    promoted        BOOLEAN,
    promote_after   INT,            -- NULL treated as 0 (SSG)
    debounce_secs   INT,
    html_path       TEXT,
    json_path       TEXT,           -- NULL if JSON not opted in
    checksum        TEXT,
    last_hit        TIMESTAMPTZ,
    purge_after     INT,
    PRIMARY KEY (route, slot)
)
```

### Invalidation
- Synchronous — no event log, no queue
- `dep!(table, col, val)` macro produces typed `DependencyKey`
- Serialises to `"table:column=value"` e.g. `"tickets:id=123"`
- `pilcrow::invalidate!(dep!(tickets, id, 123))` → `UPDATE pilcrow_fsr SET stale=TRUE WHERE depends_on @> ARRAY[...]`
- Route-level: `pilcrow::invalidate!(route = "/tickets/123")`

### Watcher process
- Embedded (default) — Tokio task inside Pilcrow
- External — opt-in via `pilcrow.toml`, Pilcrow exposes `pilcrow_fsr_watcher_tick(pool)`
- Poll interval configurable in `pilcrow.toml`

### Promotion and debounce
- Promotion threshold declared per field via `#[pilcrow::promote_after(N)]`
- Framework default in `pilcrow.toml → [fsr] promote_after_hits`
- Debounce declared per field via `#[pilcrow::patch_debounce(N)]`
- Framework default in `pilcrow.toml → [fsr] patch_debounce_secs`
- Both co-located with the field declaration — not in toml per route

### HTML shell attribute
- `s-live="slot_name"` — consistent with Silcrow's `s-` prefix convention
- Same name end to end: `live.rs` field name = `s-live` attr = `pilcrow_fsr` slot = SSE payload key

### JSON baking
- Opt-in at route level: `pub const FSR_JSON: bool = true` in `page.rs`
- Only `LiveProps` fields included — static fields never appear in baked JSON
- Baked flat: `{ "ticket_status": "In Progress", "ticket_priority": "High" }`

### Silcrow.js
- Auto-injected by Pilcrow when any route has `live.rs` — dev never references it
- Owns SSE hub — one persistent connection per app lifetime
- On navigation → sends current route + active slot names to server
- Server pushes only slots relevant to current route
- DOM patching via `querySelectorAll('[s-live="slot_name"]')` → `textContent`
- No changes needed to existing Silcrow.js patch path

### List rows
- Slot naming: `list_field__row_id__field_name` e.g. `ticket_list__42__status`
- Only watched columns get shell slots — static columns baked directly
- Same patcher, no special handling needed

### Dependency key derivation
- SQLx is used as-is — Pilcrow does not wrap it
- Developer declares `depends_on` explicitly via `dep!` macro on each `LiveProps` field
- `dep!(tickets, id, params.id)` → args are: table, column, runtime value — typed, refactor-safe, no raw strings

### Query deduplication
- Same SQL + same params across multiple `LiveProps` fields → executes once
- All fields populated from single result row

---

## File convention

```
pages/
  tickets/
    [id]/
      page.rs       — handler, Props struct, static fields
      live.rs       — LiveProps fields, query, depends_on, policies
      page.html     — template with s-live shell slots
```

---

## Developer surface — complete, nothing else needed

### `live.rs`
```rust
use pilcrow::live::*;

pub struct Live {
    #[pilcrow::promote_after(50)]
    #[pilcrow::patch_debounce(30)]
    #[pilcrow::depends_on(dep!(tickets, id, params.id))]
    pub ticket_status: LiveProps<String>,

    #[pilcrow::depends_on(dep!(tickets, id, params.id))]
    pub ticket_priority: LiveProps<String>,
}

impl PilcrowLive for Live {
    fn query(params: &RouteParams) -> LiveQuery {
        live_query!(
            "SELECT status, priority FROM tickets WHERE id = $1",
            params.id
        )
    }
}
```

### `page.rs`
```rust
// Optional JSON opt-in — only LiveProps fields baked into JSON
pub const FSR_JSON: bool = true;

pub struct Props {
    pub title: String,   // static — baked into HTML only
    pub live: Live,      // watched — shell slots + pilcrow_fsr rows
}

pub async fn load(
    Path(id): Path<i32>,
    live: Live,          // injected by Pilcrow automatically
) -> AppResult<Props> {
    Ok(Props {
        title: "Ticket".into(),
        live,
    })
}
```

### `page.html`
```html
<!-- Static field — baked directly -->
<h1>{{ title }}</h1>

<!-- Watched field — shell slot -->
<span s-live="ticket_status">{{ live.ticket_status.value }}</span>
<span s-live="ticket_priority">{{ live.ticket_priority.value }}</span>

<!-- List row watched field -->
<span s-live="ticket_list__42__status">Open</span>
```

### `pilcrow.toml`
```toml
[fsr]
watcher              = "embedded"
poll_interval_ms     = 500
promote_after_hits   = 100
patch_debounce_secs  = 30
purge_after_seconds  = 2592000
```

### Macros
```rust
// dep!(table, column, runtime_value)
// Args: table name, column name, the runtime value to match (e.g. a route param).
dep!(tickets, id, params.id)             // typed DependencyKey
live_query!("SELECT ...", param)         // LiveQuery with bound params
pilcrow::invalidate!(dep!(tickets, id, 123))   // targeted invalidation
pilcrow::invalidate!(route = "/tickets/123")   // route-level invalidation
```

---

## Runtime flow

### SSR request (not yet promoted)
```
Request arrives
→ Pilcrow injects Live via FromRequestParts
→ Executes Live::query(params) via SQLx pool
→ Populates LiveProps fields from result row
→ Writes slot rows to pilcrow_fsr (insert or update)
→ Increments hit_count on route row
→ Checks hit_count >= promote_after → sets promoted = TRUE if threshold reached
→ Injects current LiveProps values into s-live shell slots
→ Serves HTML fresh
```

### Promoted route request
```
Request arrives
→ Serve html_path file directly
→ s-live slots already contain latest value (patched by watcher)
→ No DB read needed
```

### Dep change → surgical patch
```
pilcrow::invalidate!(dep!(tickets, id, 123))
→ UPDATE pilcrow_fsr SET stale=TRUE WHERE depends_on @> ARRAY['tickets:id=123']
→ Watcher polls, sees stale=TRUE
→ Re-executes stored query with stored params
→ Re-bakes s-live slots in HTML
→ Re-bakes JSON file (if json_path set)
→ SET stale=FALSE, version=version+1
→ Debounce → patches html_path file on disk (promoted routes)
→ Pushes SSE: { "ticket_status": "In Progress" } to connected clients on this route
→ Silcrow.js patches DOM via querySelectorAll('[s-live="ticket_status"]')
```

### App startup (promote_after = 0 or absent)
```
pilcrow_start()
→ SELECT all pilcrow_fsr rows WHERE promote_after = 0 OR promote_after IS NULL
→ Execute query for each slot
→ Bake HTML and JSON files
→ Mark promoted = TRUE
→ Server begins accepting traffic
```

---

## Comparison with other rendering models

```
SSG              ★★★★★  Pilcrow matches — promoted routes are static file serves
ISR (Next.js)    ★★★★☆  Pilcrow is better — dep-based not time-based, field-level not page-level
SSR              ★★★☆☆  Pilcrow matches for unpromoted routes, better cache story
CSR              ★★☆☆☆  Pilcrow better — 95% baked, only watched fields are shells
Streaming SSR    ★★★★☆  Pilcrow has this too (AsyncValue, AsyncHTML, STREAMING=true)
```

Pilcrow's unique advantages over all:
- Field-level granularity — no other framework does this at HTML baking level
- Dep-based invalidation — not time-based, not manual
- Automatic promotion based on traffic
- Zero client JS required for live fields — SSE + server patch, no hydration
- One integer (`promote_after`) unifies SSG/ISR/FSR/SSR into a single continuum

---

## What is NOT in scope for FSR

- Wrapping SQLx — developer uses SQLx directly, declares deps explicitly
- Shared DTOs — frontend and backend define independent structs
- Value storage in DB — pilcrow_fsr never stores field values
- Per-route toml config — all config is co-located on the field declaration
- File-based cache fallback — DB is required if app uses LiveProps
