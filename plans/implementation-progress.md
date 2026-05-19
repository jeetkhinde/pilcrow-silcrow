# Simplify Rendering Architecture — Implementation Progress

Branch: `claude/simplify-rendering-architecture-v1zfU`

## Objective

Reduce feature fatigue by collapsing SSG / ISR / SSR Streaming / AsyncValue / AsyncHTML
into a single primitive set: **baked HTML, baked JSON, Redis + LiveProp, FSR**.
The 20 TODOs below capture every decision from the architectural brainstorm.

---

## Slices

### Slice A — Tombstone invalidation  ✅ DONE

**TODO #1**: When a promoted route's entity is deleted, mark it tombstoned so the
next request returns 404 and all Redis + disk artefacts are cleared.

**Files changed:**
| File | Change |
|------|--------|
| `pilcrow/crates/runtime/migrations/0002_pilcrow_fsr_tombstone.sql` | NEW — adds `tombstoned BOOLEAN` column + partial index |
| `pilcrow/crates/runtime/src/fsr/store.rs` | `HitStatus` enum; `tombstone()`, `is_tombstoned()`, updated `increment_hit()` |
| `pilcrow/crates/runtime/src/fsr/cache.rs` | `RedisCache::delete_route_keys()` |
| `pilcrow/crates/runtime/src/fsr/handle.rs` | NEW — `FsrHandle` (mirrors `IsrHandle`/`req.cache`) |
| `pilcrow/crates/runtime/src/fsr/mod.rs` | Re-exports: `FsrHandle`, `HitStatus`, `fsr_store_for_handle` |
| `pilcrow/crates/runtime/src/fsr/extractor.rs` | `fsr_store_for_handle()`; tombstone guard in `extract_live_from_parts` |
| `pilcrow/crates/runtime/src/context.rs` | `pub fsr: FsrHandle` on `Req`; all construction sites updated |
| `pilcrow/registry.toml` | FSR feature entry updated with tombstone source refs and validation rules |

**DX surface (how it looks in a handler):**
```rust
async fn delete_task(req: Req, Path(id): Path<i64>) -> impl IntoResponse {
    db::delete_task(id).await?;
    req.fsr.tombstone(&req.uri().path()).await;
    StatusCode::NO_CONTENT
}
```

**Behaviour:**
1. `tombstone(route)` sets `tombstoned = TRUE`, `promoted = FALSE`, `stale = FALSE` in `pilcrow_fsr`.
2. Calls `RedisCache::delete_route_keys()` — single `DEL` for `pilcrow:html:*`, `pilcrow:slot:*`, `pilcrow:json:*`.
3. Spawns async background task to remove baked disk files (non-fatal).
4. `increment_hit()` now returns `HitStatus` (`Tombstoned | JustPromoted | Normal`).
5. `extract_live_from_parts` checks `HitStatus::Tombstoned` and returns `AppError::NotFound`.

---

### Slice B — PROMOTE_AFTER constant + scheduled invalidation  ✅ DONE

**TODO #2 (partial)**: Route-level `PROMOTE_AFTER` constant and PRERENDER → FSR mapping.
Full collapse of `emit_ssg_handler`/`emit_isr_handler` into FSR path is deferred to Slice C.

**TODO #3**: Timer-based scheduled dep-key invalidation replacing REVALIDATE TTL.

**Files changed:**
| File | Change |
|------|--------|
| `pilcrow/crates/routekit/src/templating/page_options.rs` | Added `promote_after: Option<u32>` to `FsrOpts`; updated `PageOptions` doc |
| `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` | Parse `PROMOTE_AFTER` const; `PRERENDER = true` sets `fsr.promote_after = Some(0)` |
| `pilcrow/crates/routekit/src/fsr.rs` | `process_live_rs` now accepts `route_promote_after: Option<u32>`; `generate_from_row_impl` emits `route_promote_after()` method when Some |
| `pilcrow/crates/routekit/src/templating/codegen/templates.rs` | Passes `page_options.fsr.promote_after` to `process_live_rs` |
| `pilcrow/crates/runtime/src/fsr/live_trait.rs` | Added `fn route_promote_after() -> Option<u32>` to `PilcrowLive` trait (default: None) |
| `pilcrow/crates/runtime/src/fsr/extractor.rs` | `ensure_route_row` prefers `T::route_promote_after()` over per-field value |
| `pilcrow/crates/runtime/src/fsr/watcher.rs` | `ScheduledInvalidation` struct; `WatcherConfig::scheduled_invalidations`; both spawn functions now spawn per-key timer tasks |
| `pilcrow/crates/runtime/src/fsr/mod.rs` | Exports `ScheduledInvalidation` |
| `pilcrow/crates/runtime/src/start.rs` | `WatcherConfig` literal updated with `scheduled_invalidations: Vec::new()` |

**DX surface (page.rs):**
```rust
// PRERENDER = true on a FSR route → promote_after = 0 (bake on first hit)
pub const PRERENDER: bool = true;

// OR set a custom threshold
pub const PROMOTE_AFTER: u32 = 50;
```

**DX surface (app init, replaces REVALIDATE TTL):**
```rust
spawn_embedded_watcher(store, WatcherConfig {
    scheduled_invalidations: vec![
        ScheduledInvalidation::new("exchange_rates", Duration::from_secs(60)),
        ScheduledInvalidation::new("nav_counts",     Duration::from_secs(300)),
    ],
    ..WatcherConfig::new()
}, event_tx);
```

---

### Slice C — Remove deprecated rendering paths  ✅ DONE

**TODO #4**: Remove STREAMING, REVALIDATE, MAX_STALE, CACHE_TAGS, CACHE_VARY, `Deferred<T>` / `DeferredHtml` / `AsyncHtml` / `AsyncValue`, ISR cache inspect endpoint, filesystem ISR persistence, combined PRERENDER+REVALIDATE, Static Export (`export()`).

**Files changed:**
| File | Change |
|------|--------|
| `pilcrow/crates/routekit/src/templating/page_options.rs` | Removed `IsrOpts`, `streaming` field from `PageOptions`; kept `SsgOpts`, `FsrOpts` |
| `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` | REVALIDATE/MAX_STALE/CACHE_TAGS/CACHE_VARY/STREAMING now produce build errors; removed `parse_str_slice_const` |
| `pilcrow/crates/routekit/src/templating/codegen/types.rs` | Removed `deferred_fields_map`, `deferred_html_fields_map`, `isr_config_map` from `GeneratedTemplatesModule`; removed `deferred_fields`, `deferred_html_fields` from `InstrumentedFrontmatter` |
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | Removed `AppCodegenMaps` fields for ISR/deferred; removed `emit_isr_handler`, `emit_isr_revalidation_body`, `emit_streaming_handler`; hardcoded SSG TTL to `u64::MAX` |
| `pilcrow/crates/routekit/src/templating/codegen/templates.rs` | Removed ISR/deferred map population and struct fields |
| `pilcrow/crates/routekit/src/templating/codegen/mod.rs` | Removed `IsrOpts` from imports/exports |
| `pilcrow/crates/routekit/src/templating/pipeline.rs` | Removed removed fields from `AppCodegenMaps` construction |
| `pilcrow/crates/routekit/src/templating/compiler.rs` | Removed `inject_async_value_text_spans` (dead code) |
| `pilcrow/crates/runtime/src/deferred.rs` | Removed `AsyncHtml`, `AsyncValue<T>`, streaming helpers; kept `LiveProp<T>`, `LiveTarget`, `__live_props_response`, `BakedProp<T>`, `PatchDelay` |
| `pilcrow/crates/runtime/src/isr.rs` | Removed filesystem persistence, `with_persistence`, `invalidate_tag`, snapshot types; kept in-memory `IsrCache` for SSG only |
| `pilcrow/crates/runtime/src/start.rs` | Removed `export()`, `isr_inspect_handler()`, filesystem/SQLite/Redis cache init; `export` import removed from pub exports |
| `pilcrow/crates/runtime/src/lib.rs` | Re-exports trimmed to `__live_props_response, LiveProp, LiveTarget`; removed `export` |
| `pilcrow/crates/web/src/lib.rs` | Removed streaming/async/export re-exports |
| `pilcrow/crates/runtime/tests/response.rs` | Removed async_value / async_response_combined tests |
| `pilcrow/crates/routekit/src/templating/codegen/tests.rs` | Updated ISR/streaming tests to expect build errors; removed streaming handler tests |
| `pilcrow/registry.toml` | Marked `deferred-streams`, `incremental-ssr`, `ssr-streaming` as removed |

---

## Design Decisions (locked)

### TODO #6 — Layout-aware navigation: route-segment diffing

**Decision date:** 2026-05-19  
**Status:** Locked — do not re-open without strong reason.

#### What is built

When Pilcrow's file-based router processes a request, it already knows the full layout stack for every route:

```
routes/
  _layout.html          ← root shell   (layout level "/")
  tickets/
    _layout.html        ← tickets shell (layout level "/tickets")
    [id]/
      page.html         ← only this changes on /tickets/1 → /tickets/2
```

The codegen auto-injects two attributes at layout boundary points — no developer annotation required:

- `data-ps-layout="<pattern>"` on the root element of each layout's rendered output, identifying which layout level owns that shell.
- `data-ps-slot="<child-pattern>"` on the element inside the layout where its child route renders (the page insertion point).

Example rendered HTML for `/tickets/1`:

```html
<body>
  <div data-ps-layout="/">
    <nav>…</nav>
    <div data-ps-layout="/tickets">
      <aside>…</aside>
      <main data-ps-slot="/tickets/:id">
        <!-- page.html content -->
      </main>
    </div>
  </div>
</body>
```

#### Navigation flow

On a Silcrow-intercepted link to `/tickets/2`:

1. Silcrow reads all `data-ps-layout` values already in the DOM → `["/" , "/tickets"]`
2. Sends `X-PS-Present: /,/tickets` with the navigation request
3. Server walks the route tree: root layout shared ✓, tickets layout shared ✓, page differs ✗
4. Server renders and returns **only** the `page.html` fragment (no layout wrappers, no `<html>`/`<body>`)
5. Silcrow swaps the element matching `[data-ps-slot="/tickets/:id"]` with the fragment

Full-page fallback: if `X-PS-Present` is absent (first load, non-JS, direct URL) the server renders the complete page normally. No special case needed — the attributes are just ignored.

#### What was rejected

| Idea | Why rejected |
|------|-------------|
| Auto-inject `data-ps-slot` on every component boundary (`<Products/>` → `data-ps-slot="Products"`) | Component names are not unique per page; position-based fallback IDs (`Products-0`) break on template changes |
| `data-ps-stable` compiler-inferred attribute | "Layout stable" (same `_layout.html` file) is deterministic. "Content stable" (LeftTicketPanel with ticket-specific data) is not — only the developer knows |
| Inject a `<script>` to annotate the DOM at page load | The server knows the structure at render time; re-deriving it client-side adds execution overhead and a new failure mode |
| Centralized `<script id="__ps_route__" type="application/json">` block | Functionally identical to attributes but with worse co-location; Silcrow would need to find it by ID instead of reading the attribute on the element it is about to swap |

#### Implementation layers

| Layer | Work |
|-------|------|
| `_layout.html` template compiler (`pilcrow-routekit`) | Emit `data-ps-layout="<pattern>"` on layout root; emit `data-ps-slot="<child-pattern>"` on the child insertion point |
| Request handler (`pilcrow-runtime`) | Read `X-PS-Present` header; walk route tree to find deepest shared prefix; render only the delta fragment; set `Content-Type: text/html; x-ps-fragment=1` |
| Silcrow.js | On link intercept: collect `data-ps-layout` values; send `X-PS-Present`; receive fragment; swap `[data-ps-slot="<pattern>"]` |

Slot IDs are route **patterns** (`/tickets/:id`), not resolved paths (`/tickets/42`) — they are stable across navigations to different records on the same route.

---

## TODO Backlog

| # | Title | Status |
|---|-------|--------|
| 1 | Tombstone invalidation | ✅ Done (Slice A) |
| 2 | Unify `PRERENDER = true` → `promote_after = 0, prebake = true`; collapse `emit_ssg_handler` / `emit_isr_handler` into FSR path | ✅ Done (Slice B — route-level PROMOTE_AFTER constant + PRERENDER→FSR mapping; full emit_ssg collapse deferred to Slice C) |
| 3 | Timer-based watcher — fires `invalidate_dep_key` on a schedule (replaces REVALIDATE TTL) | ✅ Done (Slice B — `ScheduledInvalidation` + `WatcherConfig::scheduled_invalidations`) |
| 4 | Deprecate and remove: STREAMING, REVALIDATE, MAX_STALE, `Deferred<T>`, `DeferredHtml`, ISR cache inspect endpoint, filesystem ISR cache, combined PRERENDER+REVALIDATE, Static Export | ✅ Done (Slice C) |
| 5 | s-boost opt-out by default — auto-skip external origin, download, `mailto:`, hash-only, `s-boost="false"` | ⬜ Pending |
| 6 | Layout-aware navigation — route-segment diffing; `data-ps-layout` / `data-ps-slot` auto-injected by codegen at layout boundaries; `X-PS-Present` header drives delta-only server renders | ⬜ Pending |
| 7 | Scroll behaviour per mode — full→top, fragment→main-top, JSON→preserve | ⬜ Pending |
| 8 | `<pilcrow:head>` always runs on fragment / JSON nav | ⬜ Pending |
| 9 | History state stores layout hash | ⬜ Pending |
| 10 | View Transitions API wraps all three nav modes | ⬜ Pending |
| 11 | One SSE per page enforced — one `data-pilcrow-live` anchor per page; all producers merge via `select_all` | ⬜ Pending |
| 12 | Keyed list patch wire format — `{ list, key, ...changed_fields }` SSE message; client targets `data-pilcrow-key` rows | ⬜ Pending |
| 13 | `#[pilcrow::key]` and `#[pilcrow::live]` field attributes — bare fields bake static HTML | ⬜ Pending |
| 14 | List broadcast producer — one server-side producer per live list | ⬜ Pending |
| 15 | Silcrow splits into cacheable same-origin modules at `/__pilcrow/runtime/` | ⬜ Pending |
| 16 | `inline_runtime = true` config option in `Pilcrow.toml` | ⬜ Pending |
| 17 | Scroll-aware windowed record baking — cursor requests trigger background Redis pre-bake of next window | ⬜ Pending |
| 18 | HTML chunk baking for lists — pre-baked HTML chunks in Redis including live field markers | ⬜ Pending |
| 19 | `Vec<T>` with `#[pilcrow::key]` + `#[pilcrow::live]` drives all list behaviour (no `LiveList` / `AppendList`) | ⬜ Pending |
| 20 | Create and maintain this progress tracking record | ✅ Done |

---

## Slice Plan (dependency order)

```
Slice A  (done)   — #1, #20   Tombstone + progress file
Slice B           — #2, #3    Collapse PRERENDER into FSR + timer watcher
Slice C           — #4        Remove deprecated rendering paths
Slice D           — #5–#10    Layout-aware navigation (s-boost, three-mode, scroll, head, history, view-transitions)
Slice E           — #11, #12  One SSE per page + keyed list wire format
Slice F           — #13–#14   #[pilcrow::key] / #[pilcrow::live] macros + list producer
Slice G           — #15–#16   Silcrow module split + inline_runtime config
Slice H           — #17–#19   Scroll-aware windowed baking + HTML chunk baking + Vec<T> list DX
```

Slices B–H each depend on the slice before them except D (navigation) which is independent of B/C.
