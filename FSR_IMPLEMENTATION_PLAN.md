# FSR — Field-Selective Rendering: Implementation Plan

> Hand this document to Claude as the complete implementation spec.
> Work phase by phase. Stop at each **STOP** boundary for review before proceeding.

---

## Background

FSR is Pilcrow's original rendering paradigm. It extends the existing
SSR / ISR / SSG / Streaming pipeline with **field-level granularity**:

- Static fields are baked directly into HTML — never tracked
- Watched fields (`LiveProps<T>`) get a shell slot in HTML, a DB cache row, and live SSE updates
- One integer (`promote_after`) controls the full rendering lifecycle

```
promote_after = 0 or absent  → SSG  (bake at startup, surgical patch on dep change)
promote_after = 1            → ISR  (bake after first request, surgical patch on dep change)
promote_after = N            → FSR  (bake after N hits, surgical patch on dep change)

No live.rs file              → pure SSR (existing Pilcrow behaviour, untouched)
```

---

## Crate / file map (existing, do not change)

```
crates/core         — AppError, AppResult, PilcrowConfig
crates/routekit     — build-time pipeline, codegen, page_options
crates/runtime      — Req, Res, ISR, SSE, assets
crates/macros       — #[handler] proc-macro
crates/client       — PilcrowClient
crates/web          — pilcrow_web facade
```

New code lives in:

```
crates/runtime/src/fsr/          — LiveProps, DependencyKey, PilcrowLive trait, watcher
crates/runtime/migrations/       — SQLx migration for pilcrow_fsr table
crates/routekit/src/fsr/         — live.rs discovery, FsrOpts codegen
```

---

## Phase 1 — DB migration

**File:** `crates/runtime/migrations/0001_pilcrow_fsr.sql`

```sql
CREATE TABLE IF NOT EXISTS pilcrow_fsr (
    route           TEXT        NOT NULL,
    slot            TEXT        NOT NULL  DEFAULT '',
    query           TEXT,
    query_params    JSONB,
    depends_on      TEXT[]      NOT NULL  DEFAULT '{}',
    stale           BOOLEAN     NOT NULL  DEFAULT FALSE,
    version         INTEGER     NOT NULL  DEFAULT 0,
    hit_count       INTEGER     NOT NULL  DEFAULT 0,
    promoted        BOOLEAN     NOT NULL  DEFAULT FALSE,
    promote_after   INTEGER,
    debounce_secs   INTEGER,
    html_path       TEXT,
    json_path       TEXT,
    checksum        TEXT,
    last_hit        TIMESTAMPTZ,
    purge_after     INTEGER,
    PRIMARY KEY (route, slot)
);

CREATE INDEX IF NOT EXISTS pilcrow_fsr_stale_idx
    ON pilcrow_fsr (stale)
    WHERE stale = TRUE;

CREATE INDEX IF NOT EXISTS pilcrow_fsr_depends_on_idx
    ON pilcrow_fsr USING GIN (depends_on);
```

**Rules:**
- `slot = ''` is the route-level row (`html_path`, `hit_count`, `promoted` live here)
- Slot rows have `slot = 'field_name'` or `slot = 'list__rowid__field'`
- `query` + `query_params` store the SQL to re-execute when `stale = TRUE`
- `value` is **never stored** — source of truth stays in the real DB tables

**STOP — review schema before Phase 2.**

---

## Phase 2 — `LiveProps<T>` type and `DependencyKey`

**File:** `crates/runtime/src/fsr/live_props.rs`

```rust
/// A field whose value is tracked, cached, and live-patched by Pilcrow.
///
/// `T` must implement `serde::Serialize + serde::de::DeserializeOwned`.
pub struct LiveProps<T> {
    pub value: T,
    pub depends_on: Vec<DependencyKey>,
    /// Override framework default (`pilcrow.toml → [fsr] promote_after_hits`).
    /// 0 or absent = SSG (bake at startup), 1 = ISR, N = FSR.
    /// All modes get surgical patch on dep change.
    pub promote_after: Option<u32>,
    /// Override framework default (`pilcrow.toml → [fsr] patch_debounce_secs`).
    pub patch_debounce: Option<u32>,
}

/// Typed dependency key. Serialises to `"table:column=value"`.
pub struct DependencyKey {
    pub table: &'static str,
    pub column: &'static str,
    pub value: String,
}

impl std::fmt::Display for DependencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}={}", self.table, self.column, self.value)
    }
}
```

**`dep!` macro** — in `crates/macros/src/dep.rs`:

```rust
/// dep!(tickets, id, ticket_id)
/// → DependencyKey { table: "tickets", column: "id", value: ticket_id.to_string() }
#[macro_export]
macro_rules! dep {
    ($table:ident, $col:ident, $val:expr) => {
        ::pilcrow_web::fsr::DependencyKey {
            table: stringify!($table),
            column: stringify!($col),
            value: $val.to_string(),
        }
    };
}
```

**Attributes** — read by routekit codegen, stripped before emit (same pattern as `REVALIDATE`):

```rust
#[pilcrow::promote_after(50)]
#[pilcrow::patch_debounce(30)]
#[pilcrow::depends_on(dep!(tickets, id, id))]
pub ticket_status: LiveProps<String>,
```

**STOP — review types before Phase 3.**

---

## Phase 3 — `PilcrowLive` trait and `live_query!`

**File:** `crates/runtime/src/fsr/live_trait.rs`

```rust
/// Implement this on any `Live` struct defined in a route's `live.rs`.
pub trait PilcrowLive: Sized {
    /// Returns the SQL query and bound params used to populate all LiveProps fields.
    fn query(params: &RouteParams) -> LiveQuery;

    /// Extract field values from a query result row.
    /// Generated by routekit codegen — developer does not implement this manually.
    fn from_row(row: &PilcrowRow) -> Self;
}

pub struct LiveQuery {
    pub sql: &'static str,
    pub params: Vec<serde_json::Value>,
}

/// live_query!("SELECT status FROM tickets WHERE id = $1", id)
#[macro_export]
macro_rules! live_query {
    ($sql:expr $(, $param:expr)*) => {
        ::pilcrow_web::fsr::LiveQuery {
            sql: $sql,
            params: vec![$( serde_json::json!($param) ),*],
        }
    };
}
```

**Deduplication rule:** if two `LiveProps` fields share the same `sql` + `params`, the query
executes once and both fields are populated from the single result row.

**STOP — review trait before Phase 4.**

---

## Phase 4 — `live.rs` file convention

routekit discovers `live.rs` alongside `page.rs` in any route directory.

**Directory layout:**

```
pages/
  tickets/
    [id]/
      page.rs       ← existing, unchanged
      live.rs       ← new, optional
      page.html     ← existing, s-live attrs added by dev
```

**`live.rs` developer surface:**

```rust
use pilcrow::live::*;

pub struct Live {
    #[pilcrow::promote_after(50)]
    #[pilcrow::patch_debounce(30)]
    #[pilcrow::depends_on(dep!(tickets, id, id))]
    pub ticket_status: LiveProps<String>,

    #[pilcrow::depends_on(dep!(tickets, id, id))]
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

**routekit codegen tasks for `live.rs`:**

1. Detect presence of `live.rs` in route directory → set `FsrOpts { has_live_file: true }`
2. Parse `Live` struct fields, extract attributes (`promote_after`, `patch_debounce`, `depends_on`)
3. Strip all `#[pilcrow::*]` attributes before emitting — they never reach Rust compiler
4. Generate `from_row()` impl on `Live` (maps SQL columns → struct fields by name)
5. Detect `pub const FSR_JSON: bool = true` in `page.rs` → set `FsrOpts { json: true }`

**`page.rs` load function — dev writes this:**

```rust
pub struct Props {
    pub title: String,     // static — baked into HTML only
    pub live: Live,        // watched — shell slots + pilcrow_fsr rows
}

pub async fn load(
    Path(id): Path<i32>,
    live: Live,            // injected by Pilcrow automatically
) -> AppResult<Props> {
    Ok(Props {
        title: "Ticket".into(),
        live,
    })
}
```

**Codegen injects `Live` extractor** via `FromRequestParts` — same pattern as `PilcrowClient`.
Dev never calls `Live::query()` manually.

**STOP — review convention before Phase 5.**

---

## Phase 5 — HTML baking and `s-live` shell slots

**Template convention in `page.html`:**

```html
<!-- Static field — baked directly, no slot -->
<h1>{{ title }}</h1>

<!-- Watched field — shell slot -->
<span s-live="ticket_status">{{ live.ticket_status.value }}</span>
<span s-live="ticket_priority">{{ live.ticket_priority.value }}</span>
```

**List row slot naming:**

```
ticket_list__42__status
ticket_list__42__priority
```

```html
<span s-live="ticket_list__42__status">Open</span>
```

**Slot name derivation:** `field_name` by default. For list rows: `list_field__row_id__field_name`.
Dev can override with `#[pilcrow::slot("custom_name")]`.

**At serve time (SSR routes):**

1. Read `pilcrow_fsr` rows for this route where `slot != ''`
2. For each stale slot — re-execute stored query, get fresh value
3. Inject fresh value into `s-live` shell in HTML before serving
4. HTML served is always fresh at request time

**Promoted routes:**

- Serve baked HTML file from `html_path` directly
- `s-live` slots already contain latest value (patched by watcher)

**JSON opt-in (`FSR_JSON = true`):**

Baked JSON at `json_path` contains only `LiveProps` fields:

```json
{
  "ticket_status": "In Progress",
  "ticket_priority": "High"
}
```

Static fields (`title`) never appear in baked JSON.

**STOP — review baking before Phase 6.**

---

## Phase 6 — Invalidation

**`pilcrow::invalidate!` macro:**

```rust
// Targeted — by dependency key
pilcrow::invalidate!(dep!(tickets, id, 123));

// Route-level — all slots on a route
pilcrow::invalidate!(route = "/tickets/123");
```

**SQL executed (targeted):**

```sql
UPDATE pilcrow_fsr
SET    stale   = TRUE,
       version = version + 1
WHERE  depends_on @> ARRAY['tickets:id=123']
```

**SQL executed (route-level):**

```sql
UPDATE pilcrow_fsr
SET    stale   = TRUE,
       version = version + 1
WHERE  route = '/tickets/123'
```

**Rules:**
- Synchronous — no event log, no queue
- Direct DB update only
- Watcher picks up `stale = TRUE` on next poll

**STOP — review invalidation before Phase 7.**

---

## Phase 7 — Watcher process

**Configuration in `pilcrow.toml`:**

```toml
[fsr]
watcher              = "embedded"   # "embedded" | "external"
poll_interval_ms     = 500
promote_after_hits   = 100          # framework default
patch_debounce_secs  = 30           # framework default
purge_after_seconds  = 2592000      # 30 days
```

**Watcher loop (embedded mode — runs as Tokio task inside Pilcrow):**

```
LOOP every poll_interval_ms:
  SELECT route, slot, query, query_params, depends_on, promoted, debounce_secs, html_path, json_path
  FROM pilcrow_fsr
  WHERE stale = TRUE

  FOR each stale row:
    re-execute stored query with stored params via SQLx pool
    extract field value from result
    update s-live slot in HTML (always)
    update json_path file (if set)
    SET stale = FALSE, version = version + 1
    push SSE to connected clients on this route

  FOR promoted routes after debounce:
    patch baked HTML file on disk at html_path
    patch baked JSON file on disk at json_path (if set)
```

**External mode:** Pilcrow exposes `pilcrow_fsr_watcher_tick(pool)` as a public async fn.
External process calls it on its own schedule.

**STOP — review watcher before Phase 8.**

---

## Phase 8 — SSE push via Silcrow.js

**Silcrow.js is auto-injected** when `live.rs` is present in any route.
Dev never references Silcrow.js explicitly — it works like a tiny script.

**Detection in routekit:** if any route has `FsrOpts { has_live_file: true }` →
inject Silcrow.js `<script>` tag in `<head>` of every page (same as dev-reload injection).

**Hub model — one persistent SSE connection per app:**

```
App loads
→ Silcrow opens ONE SSE connection to /__pilcrow/live

On each navigation
→ Silcrow sends current route + active slot names to server:
  { "route": "/tickets/123", "slots": ["ticket_status", "ticket_priority"] }

Server updates slot subscription for this client connection.
Server pushes only slots relevant to current route.
```

**SSE push payload from watcher:**

```json
{ "ticket_status": "In Progress" }
```

For list rows:
```json
{ "ticket_list__42__status": "Closed" }
```

**Silcrow patches DOM** via `querySelectorAll('[s-live="ticket_status"]')` → sets `textContent`.
No changes to existing Silcrow.js patch path — `s-live` is handled identically to existing
reactive binding attributes.

**STOP — review SSE before Phase 9.**

---

## Phase 9 — `page_options.rs` integration

**Add `FsrOpts` to `PageOptions`** in `crates/routekit/src/templating/page_options.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FsrOpts {
    /// `live.rs` was found alongside this page's `page.rs`.
    pub has_live_file: bool,
    /// `pub const FSR_JSON: bool = true` was declared in `page.rs`.
    pub json: bool,
}
```

**Constants detected and stripped (same pattern as `REVALIDATE`, `PRERENDER`, `STREAMING`):**

```rust
pub const FSR_JSON: bool = true;   // in page.rs — opt in to baked JSON
```

**Compatibility matrix — build errors:**

```
FSR_JSON = true  +  STREAMING = true   → build error (incompatible)
live.rs present  +  STREAMING = true   → build error (incompatible)
live.rs present  +  REVALIDATE         → valid (ISR + field-level live)
live.rs present  +  PRERENDER = true   → valid (treated as promote_after = 0)
promote_after absent on LiveProps      → treated as 0, baked at startup
```

**STOP — review page_options before Phase 10.**

---

## Phase 10 — `pilcrow_fsr` DB hit count and promotion

**On every request to a route with `live.rs`:**

```sql
UPDATE pilcrow_fsr
SET    hit_count = hit_count + 1,
       last_hit  = NOW()
WHERE  route = $1
AND    slot  = ''
```

**Promotion check (after update):**

```sql
UPDATE pilcrow_fsr
SET    promoted = TRUE
WHERE  route         = $1
AND    slot          = ''
AND    hit_count    >= COALESCE(promote_after, 0)
AND    promoted      = FALSE
```

When `promoted` flips to `TRUE`:
- Watcher bakes full HTML to `html_path` on next tick
- Subsequent requests served from `html_path` file directly (no DB read for static fields)

**`promote_after = 0` or absent special case:**

Both treated identically — bake at server startup in `pilcrow_start()`, same path as `PRERENDER = true`.
All routes/slots with `promote_after = 0` or `promote_after = NULL` are baked before server accepts traffic.
All three modes (SSG/ISR/FSR) receive surgical patch on dep change — no exceptions.

---

## Phase 11 — re-exports and developer surface

**Add to `crates/web/src/lib.rs`:**

```rust
pub use pilcrow_runtime::fsr::{
    LiveProps,
    DependencyKey,
    PilcrowLive,
    LiveQuery,
};
pub use pilcrow_macros::{dep, live_query, invalidate};
```

**Developer imports one line:**

```rust
use pilcrow::live::*;
```

---

## Complete developer surface (nothing else needed)

```
live.rs
  pub struct Live { ... }              — LiveProps fields with attribute annotations
  impl PilcrowLive for Live { ... }    — one query, one impl block

page.rs
  pub struct Props { pub live: Live }  — Live embedded in Props
  pub async fn load(..., live: Live)   — Live injected, zero boilerplate
  pub const FSR_JSON: bool = true;     — optional, JSON opt-in

page.html
  s-live="slot_name"                   — on any element whose value is watched

pilcrow.toml
  [fsr] section                        — framework defaults only, all optional

Macros
  dep!(table, col, val)                — typed dependency key
  pilcrow::invalidate!(dep!(...))      — targeted invalidation
  pilcrow::invalidate!(route = "...")  — route-level invalidation
```

---

## Implementation order

```
Phase 1   DB migration
Phase 2   LiveProps<T> + DependencyKey + dep! macro
Phase 3   PilcrowLive trait + live_query! macro
Phase 4   live.rs discovery + routekit codegen
Phase 5   HTML baking + s-live shell slots
Phase 6   pilcrow::invalidate! macro + SQL
Phase 7   Watcher process (embedded Tokio task)
Phase 8   SSE hub + Silcrow.js auto-injection
Phase 9   FsrOpts in page_options.rs + compatibility checks
Phase 10  hit_count + promotion logic
Phase 11  Re-exports + use pilcrow::live::*
```

Stop at each phase boundary for review before proceeding.
