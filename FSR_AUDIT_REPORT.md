# FSR Implementation Audit Report

**Date:** 2026-05-13
**Scope:** Real code in `pilcrow/` against `FSR_DESIGN_DECISIONS.md` + `FSR_IMPLEMENTATION_PLAN.md`
**Status legend:** ✅ Matches spec · ⚠️ Deviation (non-breaking) · ❌ Gap / not implemented · 🐛 Bug

---

## Phase 1 — DB Migration

| Item | Spec | Real | Status |
|---|---|---|---|
| File path | `crates/runtime/migrations/0001_pilcrow_fsr.sql` | `migrations/003_pilcrow_fsr.sql` (root-level) | ⚠️ Wrong location/name |
| Schema columns | All 16 columns including `checksum`, `purge_after` | Exact match | ✅ |
| `slot = ''` convention | Route-level row | Implemented consistently | ✅ |
| `pilcrow_fsr_stale_idx` (partial) | Required | Present | ✅ |
| `pilcrow_fsr_depends_on_idx` (GIN) | Required | Present | ✅ |

---

## Phase 2 — `LiveProp<T>`, `DependencyKey`, `dep!`

| Item | Spec | Real | Status |
|---|---|---|---|
| `LiveProp<T>` struct | `depends_on: Vec<DependencyKey>` | `depends_on: Vec<String>` (pre-serialized) | ⚠️ Type changed — caller can't inspect dep keys after construction |
| `LiveProp<T>` other fields | `value`, `promote_after`, `patch_debounce` | Exact match | ✅ |
| `DependencyKey` struct | `table: &'static str, column: &'static str, value: String` | Exact match | ✅ |
| `Display` impl | `"table:column=value"` | ✅ + extra `as_dep_string()` helper | ✅ |
| `dep!` macro for developer | `pilcrow::dep!(tickets, id, val)` via `use pilcrow::live::*` | `fsr_dep!` in runtime, re-exported as `dep` in `web/src/live.rs` | ✅ (alias works) |
| `dep_macro.rs` in `crates/macros` | — | Exists but generates `baked_pages::DependencyKey::new()` and is **not exported** from `macros/src/lib.rs` | 🐛 Dead code, wrong type |

---

## Phase 3 — `PilcrowLive` trait + `live_query!`

| Item | Spec | Real | Status |
|---|---|---|---|
| `LiveQuery` struct | `sql: &'static str, params: Vec<Value>` | Exact match | ✅ |
| `PilcrowLive::query` signature | `fn query(params: &RouteParams) -> LiveQuery` | `fn query(params: &serde_json::Map<String, Value>) -> LiveQuery` | ⚠️ `RouteParams` type never created; raw JSON map used instead |
| `PilcrowLive::from_row` signature | `fn from_row(row: &PilcrowRow) -> Self` | `fn from_row(row: &HashMap<String, Value>, params: &serde_json::Map<String, Value>) -> Self` | ⚠️ `params` added (needed for dep string generation); `PilcrowRow` never created |
| `live_query!` macro | `live_query!("SQL", param)` | Exact match in `fsr/macros.rs` | ✅ |
| Query deduplication | Same SQL+params → execute once | Not implemented | ❌ |

---

## Phase 4 — `live.rs` discovery + routekit codegen

| Item | Spec | Real | Status |
|---|---|---|---|
| `live.rs` detection + `FsrOpts.has_live_file` | Required | Implemented in `crates/routekit/src/fsr.rs` | ✅ |
| `FSR_JSON` constant detection → `FsrOpts.json` | Required | In `page_options.rs` | ✅ |
| `#[pilcrow::live(promote_after, patch_debounce)]` struct-level defaults | Required | Implemented | ✅ |
| `#[pilcrow::depends_on_route(table, col)]` shorthand | Required | Implemented with route-param validation | ✅ |
| `#[pilcrow::promote_after(N)]` field-level | Required | Implemented, overrides struct default | ✅ |
| `#[pilcrow::patch_debounce(N)]` field-level | Required | Implemented | ✅ |
| `#[pilcrow::depends_on(dep!(...))]` field-level | Required | Implemented | ✅ |
| Attribute stripping before emit | Required | Implemented | ✅ |
| `from_row()` codegen | Required | Generated into processed source | ✅ |
| `#[pilcrow::slot("custom_name")]` — custom slot name | Phase 5 spec | **Not implemented**; instead `#[pilcrow::column("name")]` for DB column remapping exists | ❌ / ⚠️ |
| `#[pilcrow::allow_unused]` field attribute | Not in spec | Added as build-time escape hatch | ⚠️ Extra (useful) |
| Template ↔ `live.rs` slot validation at codegen time | Not in spec | Added (`validate_live_template_slots`) | ⚠️ Extra (good) |

---

## Phase 5 — HTML baking + `s-live` shell slots

| Item | Spec | Real | Status |
|---|---|---|---|
| `inject_fsr_slots()` — replace slot content | Required | `crates/runtime/src/fsr/baking.rs` with HTML escaping | ✅ |
| `find_s_live_slots()` — scan HTML for slots | Required | Present | ✅ |
| Serving promoted routes from `html_path` directly | Required | `get_promoted_paths()` on store exists; actual request-handler path that skips DB is **not wired** | ❌ |
| JSON opt-in baking (`FSR_JSON = true`) | Opt-in | `FsrOpts.json` tracked; actual JSON file write at promotion not implemented | ❌ |
| SSR request slot injection (fresh values injected per request) | Required | `extractor.rs` is a **stub** returning an empty row | ❌ Stub |

---

## Phase 6 — Invalidation

| Item | Spec | Real | Status |
|---|---|---|---|
| `pilcrow::invalidate!(dep!(...))` — no explicit store arg | `pilcrow::invalidate!(dep!(tickets, id, 123))` | `fsr_invalidate!(store_ref, dep!(...))` — **requires explicit store argument** | ⚠️ API shape differs |
| `pilcrow::invalidate!(route = "...")` | Same | `fsr_invalidate!(store_ref, route = "...")` | ⚠️ Same deviation |
| SQL for dep-key invalidation | `WHERE depends_on @> ARRAY[...]` + `version++` | Exact match | ✅ |
| SQL for route-level invalidation | `WHERE route = $1` | `WHERE route = $1 AND slot != ''` (excludes route row) | ⚠️ Minor but intentional |
| Synchronous, no queue | Required | Direct UPDATE, synchronous | ✅ |

---

## Phase 7 — Watcher process

| Item | Spec | Real | Status |
|---|---|---|---|
| `spawn_embedded_watcher()` | Required | Present in `watcher.rs` | ✅ |
| `pilcrow_fsr_watcher_tick(pool)` external API | Required | Present | ✅ |
| Fetch stale → re-execute query → patch HTML → mark fresh | Required | Full loop implemented | ✅ |
| SSE push after patch | Required | Broadcasts `SlotPatch` | ✅ |
| `WatcherConfig` defaults match `pilcrow.toml` spec | poll=500ms, promote=100, debounce=30, purge=30d | Exact match | ✅ |
| Debounce before patching promoted HTML on disk | Required (`debounce_secs`) | **Not implemented** — patches immediately | ❌ |
| Purge logic (`purge_after`) | Required | Field tracked in DB/config, purge not executed | ❌ |
| External mode wiring in `start.rs` | Config `watcher = "external"` → skip spawn | `start.rs` checks `fsr_config.watcher == "embedded"` before spawning | ✅ |

---

## Phase 8 — SSE hub + Silcrow.js

| Item | Spec | Real | Status |
|---|---|---|---|
| SSE endpoint URL | `/__pilcrow/live` | `/__pilcrow/fsr` | ⚠️ URL renamed |
| Route + slots filter | `route=...&slots=...` query params | Exact match | ✅ |
| SSE event name | `"fsr"` (per recent fix commit) | `Event::default().event("fsr")` | ✅ |
| SSE payload shape | `{ "slot_name": value }` | `json!({ &patch.slot: patch.value })` | ✅ |
| Hub registered in `start.rs` | Required | `/__pilcrow/fsr` route + broadcast channel wired | ✅ |
| Silcrow.js auto-injection when any route has `live.rs` | Required | Not visible in routekit codegen files | ❌ Not verified |
| One persistent connection per app, nav sends route+slots | Silcrow.js behaviour | Silcrow.js side not audited | — |

---

## Phase 9 — `FsrOpts` in `page_options.rs`

| Item | Spec | Real | Status |
|---|---|---|---|
| `FsrOpts { has_live_file, json }` struct | Required | Exact match | ✅ |
| `FSR_JSON = true` detection + stripping | Required | Implemented | ✅ |
| `live.rs + STREAMING → build error` | Required | **Not implemented** | ❌ |
| `FSR_JSON + STREAMING → build error` | Required | **Not implemented** | ❌ |

---

## Phase 10 — hit_count + promotion

| Item | Spec | Real | Status |
|---|---|---|---|
| `increment_hit()` per request | Required | `FsrStore::increment_hit()` exists | ✅ (store layer) |
| Promotion flip when `hit_count >= promote_after` | Required | In `increment_hit()`, sets `promoted = TRUE` | ✅ |
| `promote_after = 0` / absent → bake at startup | Required | **Not integrated** into `start.rs` / `pilcrow_start()` | ❌ |
| Promoted routes served from `html_path` (skip DB read) | Required | `get_promoted_paths()` exists on store; handler not wired | ❌ |

---

## Phase 11 — Re-exports + developer surface

| Item | Spec | Real | Status |
|---|---|---|---|
| `use pilcrow::live::*` | Required | `web/src/live.rs` exports all items | ✅ |
| `LiveProp, DependencyKey, PilcrowLive, LiveQuery` | Required | All exported | ✅ |
| `dep!, live_query!, invalidate!` macros | Required | All available under `pilcrow::live::*` | ✅ |
| Feature gating `#[cfg(feature = "live-props")]` | Not in spec | Applied in `live.rs` re-exports and `start.rs` | ⚠️ Extra (good practice) |

---

## Summary

| Phase | Status |
|---|---|
| Phase 1 — DB migration | ✅ Schema correct, path deviated |
| Phase 2 — LiveProp + DependencyKey + dep! | ✅ + 1 dead-code bug in macros crate |
| Phase 3 — PilcrowLive + live_query! | ✅ with type naming deviations |
| Phase 4 — live.rs codegen | ✅ + extra features added |
| Phase 5 — HTML baking | ⚠️ `baking.rs` complete; extractor is a stub; promoted serving not wired |
| Phase 6 — Invalidation | ✅ logic correct; API requires explicit store arg |
| Phase 7 — Watcher | ✅ core loop; debounce and purge not implemented |
| Phase 8 — SSE hub | ✅ core; Silcrow.js injection not verified |
| Phase 9 — FsrOpts | ✅ struct; compatibility build errors absent |
| Phase 10 — hit_count + promotion | ⚠️ store methods exist; SSG startup bake and promoted request path not wired |
| Phase 11 — Re-exports | ✅ |

---

## Top 5 Gaps to Close (priority order)

1. **`extractor.rs` stub** (`crates/runtime/src/fsr/extractor.rs:14`) — `extract_live_from_parts()` always returns an empty row; SSR requests never actually read live values from DB or inject them into templates.

2. **Promoted route serving not wired** — `FsrStore::get_promoted_paths()` exists but no request handler reads it and serves `html_path` directly, bypassing the DB read.

3. **SSG startup bake missing** — `start.rs` never queries `pilcrow_fsr` for rows with `promote_after = 0 / NULL` and bakes them before accepting traffic (the Phase 10 spec requirement).

4. **Watcher debounce not implemented** — `debounce_secs` is stored in `pilcrow_fsr` and in `WatcherConfig` but the watcher patches promoted HTML files on disk immediately rather than coalescing rapid dep changes.

5. **`invalidate!` API shape** — spec is `pilcrow::invalidate!(dep!(...))` (store injected automatically); real API is `fsr_invalidate!(store_ref, dep!(...))` requiring the caller to pass the store explicitly.
