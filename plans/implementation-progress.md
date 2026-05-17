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

## TODO Backlog

| # | Title | Status |
|---|-------|--------|
| 1 | Tombstone invalidation | ✅ Done (Slice A) |
| 2 | Unify `PRERENDER = true` → `promote_after = 0, prebake = true`; collapse `emit_ssg_handler` / `emit_isr_handler` into FSR path | ⬜ Pending |
| 3 | Timer-based watcher — fires `invalidate_dep_key` on a schedule (replaces REVALIDATE TTL) | ⬜ Pending |
| 4 | Deprecate and remove: STREAMING, REVALIDATE, MAX_STALE, `Deferred<T>`, `DeferredHtml`, ISR cache inspect endpoint, filesystem ISR cache, combined PRERENDER+REVALIDATE, Static Export | ⬜ Pending |
| 5 | s-boost opt-out by default — auto-skip external origin, download, `mailto:`, hash-only, `s-boost="false"` | ⬜ Pending |
| 6 | Layout-aware navigation — three-mode system (JSON / fragment / full) driven by `X-Pilcrow-Layout` header; `data-ps-slot` markers emitted by codegen | ⬜ Pending |
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
