# Pilcrow Rendering Models

## Quick reference

| Mode | Trigger | Cache | Streaming | Node required |
|------|---------|-------|-----------|---------------|
| SSR (default) | — | none | no | no |
| FSR | `PROMOTE_AFTER: u32 = N` | yes, Redis (hit-count promoted) | no | no |
| FSR (bake on first hit) | `PROMOTE_AFTER: u32 = 0` | yes, Redis (forever after first hit) | no | no |
| Live Props | `LiveProp<T>` field in Props | none (always live) | SSE persistent | no |
| Pilcrow Islands | `<island>` tag | none (per-request fragment fetch) | no | no |
| React Islands (CSR) | `<react strategy="load/idle/visible">` | browser SW | no | no |
| React Islands (SSR) | `<react strategy="shell/ssr">` | browser SW | no | yes (ssr only) |

> **Removed modes**: ISR (`REVALIDATE`), SSR Streaming (`STREAMING`), Deferred fields (`Deferred<T>` / `DeferredHtml`), Static Export (`pilcrow_export`), `PRERENDER: bool`. Using any of these now produces a **build error**.

---

## 1. SSR — Server-Side Rendering (default)

`load()` runs on every request. No special constant needed.
A page with no `.rs` file renders the HTML template directly with no data loading.

```rust
// code-behind — no special constants; this is the default
pub async fn load(req: Req) -> AppResult<Props> { … }
```

**Key files**
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — standard handler emit
- `pilcrow/crates/routekit/src/templating/page_options.rs` — `PageOptions` struct

---

## 2. FSR — Field-Selective Rendering

Hit-count-based route promotion to a baked state. On first N misses, `load()` runs normally. After the threshold is crossed, Pilcrow bakes the HTML + JSON to Redis and disk; subsequent requests skip `load()` entirely. Live fields (`LiveProp<T>`) remain live on top of the baked shell via SSE.

```rust
// code-behind — promote after 50 hits
pub const PROMOTE_AFTER: u32 = 50;

// Bake on first hit
pub const PROMOTE_AFTER: u32 = 0;
```

### Tombstone invalidation

When an entity is deleted, tombstone the route. This marks the route deleted in the DB, clears Redis keys, and removes disk artifacts. The next request returns `404`.

```rust
async fn delete_task(req: Req, Path(id): Path<i64>) -> impl IntoResponse {
    db::delete_task(id).await?;
    req.fsr.tombstone(&req.uri().path()).await;
    StatusCode::NO_CONTENT
}
```

### Windowed prebaking

After serving page N of a paginated list, kick off a background GET to pre-warm page N+1:

```rust
pub async fn load(req: Req) -> AppResult<Props> {
    let tickets = db::tickets_page(&cursor).await?;
    if let Some(next) = tickets.next_cursor {
        req.fsr.prebake_next(&format!("/tickets?cursor={next}"));
    }
    Ok(Props { tickets })
}
```

### Scheduled invalidation (replaces REVALIDATE TTL)

**Preferred — field-level `revalidate = N` on Props (zero manual wiring):**

```rust
// page.rs
pub struct Props {
    #[pilcrow::live(revalidate = 60)]   // re-bake every 60 s automatically
    pub status: LiveProp<String>,

    #[pilcrow::live(revalidate = 300)]
    pub price: LiveProp<f64>,
}
```

Codegen auto-derives dep key `"{module_name}::{field_name}"` (e.g. `"page_tickets::status"`),
injects it as `depends_on` in `live.rs`'s `from_row()` when no explicit `depends_on` is set,
and registers a `ScheduledInvalidation` in `__pilcrow_init()`. No `WatcherConfig`, no
`hooks.rs`, no manual dep keys needed.

**Manual — explicit `WatcherConfig` in `hooks.rs` (shared dep keys across routes):**

```rust
spawn_embedded_watcher(store, WatcherConfig {
    scheduled_invalidations: vec![
        ScheduledInvalidation::new("exchange_rates", Duration::from_secs(60)),
        ScheduledInvalidation::new("nav_counts",     Duration::from_secs(300)),
    ],
    ..WatcherConfig::new()
}, event_tx);
```

Use the manual approach when multiple routes share one dep key, or when invalidation is
triggered by an external event (e.g. `req.fsr.invalidate_dep_key("products").await`).

### Redis key structure

```
pilcrow:html:<route>     → full baked HTML string
pilcrow:slot:<route>:<name> → individual slot value
pilcrow:json:<route>     → baked JSON data
```

### Constraints

- `PROMOTE_AFTER = 0` means bake on first hit; all subsequent requests skip `load()`
- Dynamic routes with `PROMOTE_AFTER = 0` must provide `entries()` for startup prebaking
- `#[pilcrow::live(revalidate = N)]` on a Props `LiveProp<T>` field auto-wires a timer and dep key — no manual `depends_on` in `live.rs` needed unless you want to share the dep key
- `revalidate = N` and an explicit `depends_on` in `live.rs` coexist: explicit wins for `depends_on`; the timer fires regardless
- `REVALIDATE`, `MAX_STALE`, `CACHE_TAGS`, `CACHE_VARY`, `STREAMING`, `PRERENDER` are **build errors**

**Key files**
- `pilcrow/crates/runtime/src/fsr/` — `store.rs`, `handle.rs`, `extractor.rs`, `watcher.rs`, `cache.rs`
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — parses `PROMOTE_AFTER` and `#[pilcrow::live(...)]` Props field attrs
- `pilcrow/crates/routekit/src/templating/page_options.rs` — `FsrOpts`, `LiveFieldAttr`
- `pilcrow/crates/routekit/src/fsr.rs` — `process_live_rs`, auto dep key injection
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — emits `__register_codegen_scheduled_invalidations` in `__pilcrow_init()`

---

## 3. Live Props (`LiveProp<T>`)

`LiveProp<T>` is an update mode orthogonal to the render mode (SSR/FSR/SSG). Fields typed as `LiveProp<T>` render their initial value in the SSR shell and auto-subscribe the client to a server-pushed SSE channel for live updates.

### DX — developer experience

```rust
use std::sync::OnceLock;
use tokio::sync::watch;
use pilcrow_web::{AppResult, LiveProp, Req};

static COUNTER: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn counter_tx() -> &'static watch::Sender<u64> {
    COUNTER.get_or_init(|| {
        let (tx, _rx) = watch::channel(0u64);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                COUNTER.get().unwrap().send_modify(|v| *v += 1);
            }
        });
        tx
    })
}

pub struct Props {
    pub label: String,
    pub count: LiveProp<u64>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        label: "Live counter".to_string(),
        count: LiveProp::watch(counter_tx().subscribe()),
    })
}
```

Template — write `{{ count }}` exactly as any other field:

```html
<p>Count: {{ count }}</p>
```

Pilcrow rewrites this at build time into:

```html
<p>Count: <span data-pilcrow-live-field="count">{{ count }}</span></p>
```

### Producers

```rust
LiveProp::watch(rx)                   // tokio watch::Receiver<T>; initial = *rx.borrow()
LiveProp::poll(initial, dur, || fut)  // interval-based async poll
LiveProp::stream(initial, stream)     // arbitrary Stream<Item = T>
LiveProp::initial(v)                  // static — no producer, use as placeholder in load()
```

### `LiveTarget` — field-level delivery mode

```rust
.target(LiveTarget::Dom)          // default: patch [data-pilcrow-live-field] text only
.target(LiveTarget::DomAndStore)  // patch DOM + publish to Silcrow stream: atom
.target(LiveTarget::Store)        // atom only — no span generated in template
```

### Pattern B — separate `live()` when `load()` is expensive

```rust
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        label: expensive_db_call().await?,
        count: LiveProp::initial(0),
    })
}

pub struct LiveProps { pub count: LiveProp<u64> }

pub async fn live(_req: Req) -> AppResult<LiveProps> {
    Ok(LiveProps { count: LiveProp::watch(counter_tx().subscribe()) })
}
```

### Under the hood — full pipeline

#### Build time (codegen)

1. **`instrument.rs`** — detects `LiveProp<_>` fields, collects names into `live_fields`.
2. **`compiler.rs` `inject_live_text_spans`** — rewrites `{{ field }}` for each `Dom`/`DomAndStore` live field to `<span data-pilcrow-live-field="field">{{ field }}</span>`.
3. **`app_module.rs`** — injects before `</body>`: `<div data-pilcrow-live="/__pilcrow/live{path}" style="display:none"></div>`
4. **`app_module.rs` `emit_live_handler`** — registers `GET /__pilcrow/live/{page_params}` as an Axum SSE route.

#### Runtime (server)

1. `live()` is called if present; otherwise `load()` is re-called. Only `LiveProp<T>` fields are used.
2. Each field's producer is converted to a `Stream<Item = (&'static str, String)>`.
3. All field streams are merged via `select_all`.
4. On each item, emits an SSE frame: `event: live` / `data: {"count":15}`

#### Runtime (client)

1. Silcrow's `initLiveElements()` scans for `[data-pilcrow-live]` on page load and calls `connectSseHub(url)`, opening a single `EventSource` per URL (hub-pooled).
2. `connectSseHub()` adds a `live` event listener that calls `applyLivePatchPayload(data, fallbackTarget)`.
3. `applyLivePatchPayload` does `querySelectorAll('[data-pilcrow-live-field="key"]')` and sets `textContent`. No diffing, no VDOM — direct string write.

#### Layout-aware navigation

When the user navigates via boost:
- The `MutationObserver` detects removed `[data-pilcrow-live]` elements and closes their SSE subscriptions.
- `finalizeNavigation` scans the new `targetEl` for `[data-pilcrow-live]` elements and calls `initLiveElements` on them, opening fresh SSE connections.

### Constraints

- `T` must implement `Display` + `Clone` + `Serialize` + `Send + 'static`
- Pattern B validation: every `LiveProps` field must exist in `Props` with matching type — build error on mismatch
- One SSE connection per page URL per tab; all live fields share it

**Key files:**
- `pilcrow/crates/runtime/src/deferred.rs` — `LiveProp<T>`, `LiveProducer<T>`, `LiveTarget`, `__live_props_response`
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — detection of `LiveProp` fields
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — `data-pilcrow-live` anchor injection, `emit_live_handler`
- `pilcrow/crates/routekit/src/templating/compiler.rs` — `inject_live_text_spans`
- `silcrow/src/silcrow.js` — `initLiveElements`, `connectSseHub`, `applyLivePatchPayload`

---

## 4. Keyed list live updates

For live-updating lists, use `#[derive(PilcrowListRow)]` with `#[pilcrow(key)]` and `#[pilcrow(live)]` field attributes, plus `ListBroadcast` for server-side fan-out.

`Vec<T>` drives all list behavior — no special wrapper types needed. Fields with `#[pilcrow(live)]` get patched via SSE; bare fields are baked static. A `Vec<T>` with no `#[pilcrow(live)]` fields is effectively append-only.

```rust
#[derive(PilcrowListRow)]
pub struct TicketRow {
    #[pilcrow(key)]
    pub id: i64,
    #[pilcrow(live)]
    pub status: String,
    pub title: String,   // static — baked into HTML, never patched via SSE
}

// Mutation handler — fans out to all SSE subscribers
lb.send_row("tickets", &updated_ticket);

// SSE route handler
let mut rx = lb.subscribe();
match rx.recv().await {
    Ok(ev) => emit.send(SilcrowEvent::list_patch(ev.list_name, ev.key, ev.fields)).await?,
    Err(RecvError::Lagged(_)) => continue,
    Err(RecvError::Closed) => break Ok(()),
}
```

Template:

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

SSE wire format (`list-patch` event):

```
event: list-patch
data: {"list":"tickets","key":"42","status":"Closed"}
```

### HTML chunk cache (`ListChunkCache`)

For high-traffic lists, pre-bake individual row HTML in memory (or Redis) to skip per-request rendering:

```rust
// Register at startup:
let cache = Arc::new(InMemoryListChunkCache::new());
app.layer(Extension(cache.clone() as Arc<dyn ListChunkCache>));

// In a page handler — serve from cache or render + cache:
if let Some(html) = cache.get("tickets", &id.to_string()) {
    // serve pre-baked html
} else {
    let html = render_ticket_row(&ticket);
    cache.set("tickets", &id.to_string(), html.clone());
}

// After a mutation — invalidate the stale chunk:
lb.send_row("tickets", &updated);
cache.invalidate("tickets", &updated.id.to_string());
```

Redis key scheme (for future Redis-backed impl): `pilcrow:chunk:{list_name}:{row_key}` — use `list_chunk_key(list, key)` to generate collision-safe keys.

`InMemoryListChunkCache` is suitable for single-process deployments; Redis-backed implementation is future work.

**Key files:**
- `pilcrow/crates/macros/src/list_row_derive.rs` — `PilcrowListRow` derive
- `pilcrow/crates/runtime/src/live_props/list_row.rs` — `ListRow` trait
- `pilcrow/crates/runtime/src/live_props/list_broadcast.rs` — `ListBroadcast`, `ListPatchEvent`
- `pilcrow/crates/runtime/src/live_props/list_chunk.rs` — `ListChunkCache`, `InMemoryListChunkCache`, `list_chunk_key`
- `pilcrow/crates/runtime/src/sse/server_sent_events.rs` — `EventKind::ListPatch`

---

## 5. Layout-aware navigation

Silcrow intercepts same-origin `<a href>` clicks globally (no `s-boost` container needed). On navigation it sends `X-PS-Present: /,/tickets` listing all layout IDs already in the DOM. The server returns only the changed fragment (`Content-Type: text/html; x-ps-fragment=1`) if all layout shells are already present; otherwise a full page.

Auto-skipped without any attribute: external origins, `download`, `mailto:`/`tel:`, hash-only links. For explicit opt-out on same-origin links:

```html
<a href="/logout" no-boost>Log out</a>
```

Codegen auto-injects these attributes — no developer annotation required:
- `data-ps-layout="<pattern>"` on the root element of each layout's rendered output
- `data-ps-slot="<child-pattern>"` on the element where the child route renders

```html
<div data-ps-layout="/">
  <nav>…</nav>
  <div data-ps-layout="/tickets">
    <aside>…</aside>
    <main data-ps-slot="/tickets/:id">
      <!-- page content swapped here on navigation -->
    </main>
  </div>
</div>
```

**Key files:**
- `pilcrow/crates/runtime/src/nav.rs` — `extract_ps_fragment()`
- `pilcrow/crates/routekit/src/templating/pipeline.rs` — `inject_ps_nav_markers`
- `silcrow/src/silcrow.js` — `collectLayoutPatterns`, `applyFragment`, `finalizeNavigation`

---

## 6. Pilcrow Islands (`<island>` tag)

Server-driven HTML islands: Silcrow fetches a fragment from the server and injects it into the page.

```html
<island src="./counter" strategy="visible" initial="0" />
<island src="./user-card" strategy="idle" user-id="{{ props.user_id }}" />
```

| Strategy | When |
|----------|------|
| `load` (default) | Immediately |
| `visible` | `IntersectionObserver` fires |
| `idle` | `requestIdleCallback` |

**Key files**
- `pilcrow/crates/routekit/src/templating/compiler.rs` — `<island>` transpilation

---

## 7. React island strategies

```html
<react src="/react/Counter.tsx" strategy="load" initial-count="{{ props.count }}" />
```

| Strategy | Type | When |
|----------|------|------|
| `load` | CSR | Bundle available |
| `idle` | CSR | `requestIdleCallback` |
| `visible` | CSR | `IntersectionObserver` |
| `shell` | SSR | Build-time shell, hydrates with real props |
| `ssr` | SSR | Request-time render with real props |

`shell` and `ssr` require `[client.react] ssr = true` in `Pilcrow.toml`. `ssr` additionally requires Node.js at runtime.

See [`.claude/react-island.md`](react-island.md) for full usage rules and hook reference.

**Key files**
- `pilcrow/crates/routekit/src/templating/react.rs`
- `pilcrow/crates/runtime/src/island_ssr.js`
- `pilcrow/crates/runtime/assets/react-islands.js`

---

## 8. Service worker caching

Client-side GET caching, orthogonal to server rendering mode.

```toml
[service_worker]
enabled = true
strategy = "network-first"        # network-first | cache-first | stale-while-revalidate
precache = ["/", "/about"]        # silcrow.js always added automatically
exclude = ["/api/"]
offline_fallback = "/offline"
```

**Key files**
- `pilcrow/crates/runtime/src/sw.rs` — `generate_sw_source()`, `sw_inject_layer()`
- `pilcrow/crates/core/src/config/config.rs` — `ServiceWorkerConfig`, `SwStrategy`

---

## Combination rules

| Combination | Allowed? |
|-------------|----------|
| FSR + LiveProp | Yes — baked shell + SSE live layer on top |
| FSR + React/Pilcrow islands | Yes |
| FSR (PROMOTE_AFTER=0) + LiveProp | Yes |
| LiveProp + React/Pilcrow islands | Yes |
| `REVALIDATE` / `STREAMING` / `Deferred<T>` | **Build error** — removed |
