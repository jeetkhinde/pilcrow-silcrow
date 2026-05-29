# Cleanup & Consolidation Plan — Unify on the FSR Rendering Model

_Created 2026-05-29. Executable task spec for roadmap item **E1** (+ parts of **E5**) in [`roadmap-2026-05-29.md`](roadmap-2026-05-29.md). Update the roadmap as each slice lands._

> **For the implementing AI (Codex / Gemini / Claude):** Each `S#` slice below is **self-contained** — you can be handed one slice and complete it without the others, in the listed dependency order. Every slice states the **files**, the **exact change**, a **boundary** (what NOT to touch), and a **success check** (commands that must pass). **Locate code by the named symbol, not by line number** — line numbers in this doc are approximate and drift. If a symbol named here does not exist in the file, **stop and report** — do not invent a replacement.

---

## Ground truth (verified 2026-05-29 against the source — trust this over older docs)

- The page constants `REVALIDATE`, `STREAMING`, `CACHE_TAGS`, `MAX_STALE`, `CACHE_VARY` are currently **silently retained** (the `_ => true` arm in `instrument_frontmatter`). A golden test (`streaming_constant_is_silently_stripped`) asserts this. **They are NOT build errors today** — the docs are wrong.
- `PRERENDER` is currently **functional** (aliases to `promote_after`). Docs wrongly call it removed.
- **No `Deferred<T>` / `DeferredHtml` type exists anywhere** — already removed. Only doc references remain. Do not try to delete these types.
- `isr.rs` (`IsrCache`/`IsrHandle`/`IsrCacheState`) is **dead** — `pub mod isr;` in `runtime/src/lib.rs`, zero other references.
- `baked_pages/` is used **only** by `crates/web/examples/baked_*.rs` and the feature-gated `experimental-baked-pages` block in `web/src/lib.rs`. FSR baking (`fsr/store.rs`) is self-contained. `baked_pages` consumes `deferred::{PatchDelay, BakedProp, BakedField}`.
- `deferred.rs` (399 lines) contains: `__live_props_response`, `LiveTarget`, `LiveProp<T>` (legacy push-based — `watch`/`poll`/`stream`/`initial`) **[KEEP until S8]**, plus `PatchDelay`/`BakedProp`/`BakedField` **[remove in S5]**.
- **THREE `LiveProp` definitions exist; the FSR one is the only survivor (user decision 2026-05-29).**
  1. `runtime::fsr::LiveProp` (`web/src/live.rs` → `pilcrow_web::live::LiveProp`) — **KEEP.** Query-backed (`live_query!`, `PilcrowLive::query`), watcher, `s-live`, `pilcrow_fsr` rows, `PROMOTE_AFTER`.
  2. `runtime::deferred::LiveProp` (`runtime/src/lib.rs` `pub use deferred::{…, LiveProp, …}` → crate-root `pilcrow_web::LiveProp`) — legacy push (`watch`/`poll`/`stream`/`initial`). **DELETE.** Used by 6 demos (below).
  3. `runtime::live_props::model::LiveProp` (+ `LiveFieldData`, `LivePropExtract` derive, used by the `#[handler]` macro + `live_props_derive.rs`) — legacy. **DELETE.**
- **The `Vec<T>` list system is SEPARATE and KEPT** (user: "we will improve it later"): `live_props::{ListBroadcast, ListPatchEvent, ListRow, ListChunkCache, InMemoryListChunkCache, list_chunk_key}` in `live_props/list_broadcast.rs`, `list_row.rs`, `list_chunk.rs`. **Verified independent** of the legacy `LiveProp`/`LiveBroadcast`/`LivePageStore` glue — they can stay while the legacy parts of `live_props/` (`model.rs`, `broadcast.rs`, `store.rs`, `baking.rs`) are removed.
- **Legacy demos have NO database** — `demo/pages/live/index.rs`, `demo/pages/demo/fsr/index.rs`, `demo/pages/demo/async-live/index.rs` (+ `islands/activity.rs`), `demo/pages/demo/combined/simple/index.rs`, `combined/complex/index.rs` are in-memory `tokio::watch` counters incremented by a spawned task. FSR is query-backed, so these **cannot migrate 1:1** — each must be re-backed by a DB query or removed (see S7).
- **Two live SSE topologies:** FSR = one hub (`fsr_hub_handler`, `/__pilcrow/fsr`). Legacy = per-route `/__pilcrow/live{pattern}` (emitted in `app_module.rs` via `__live_props_response`, discovered by `data-pilcrow-live` anchors + `initLiveElements` in `silcrow.js`). `silcrow.js` opens exactly **one** `EventSource` today. Deleting the legacy system (S8) leaves the FSR hub as the sole connection — invariant #1 satisfied by construction.

---

## Hard invariants — every slice must hold these

1. **One live connection per client.** Preserve the multiplexed hub (`fsr_hub_handler`) and WS hub. **Never delete SSE/WS multiplexing** (`createSseHub`, `connectSseHub`, WS hub). Consolidation must move legacy per-route connections *onto* the hub — connection count per client goes to one, never up.
2. **Server is the source of truth** — live values are never the canonical store.
3. **Docs change in the same commit as the code** — never leave a doc claiming the opposite of the code.
4. **Four-layer rule:** code + `registry.toml` + MCP knowledge/validation + a test, together. After any `registry.toml`/`validation.rs` change run `cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml` (golden tests pass silently under `cargo check` — must use `test`).
5. **No app-runtime verification here** (apps need Postgres + Redis). Build + unit/golden tests are the gate; runtime checks are flagged as out-of-band for the human.

## Build/test commands (canonical, from workspace root)

```bash
cargo build --manifest-path pilcrow/Cargo.toml
cargo build --manifest-path demo/Cargo.toml
cargo build --manifest-path address-book/Cargo.toml
cargo test  --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit
cargo test  --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime
cargo test  --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml
node silcrow/build.js   # only if silcrow/src/silcrow.js changed
```

## Slice dependency order

```
S0 (baseline)
 ├─ S1 → S2 → S3        (removed page consts)
 ├─ S4                  (delete isr.rs)            — independent
 ├─ S5 → S6             (remove baked_pages + doc scrub) — independent
 └─ S7 → S8 → S9        (migrate demos to FSR, then delete legacy live infra)
```
S1–S6 are safe and can ship first. S7–S8 is the large consolidation: **decided — keep only FSR `LiveProp<T>`; delete the legacy push `LiveProp` entirely.**

---

## S0 — Baseline & test-gate sanity (do first, no product change)

**Goal:** a known-green starting point; confirm the test gate works.
**Files:** none (investigation only). Work in a git worktree.
**Steps:**
1. Run all build/test commands above; record pass/fail.
2. Verify whether `routekit/.../codegen/tests.rs` compiles. (`pilcrow/AGENTS.md` "Known Bug #1" claims tests at ~544/625 fail to compile via `render_generated_app_module` arg mismatch.) If it fails, fix the arg list or confirm the claim is stale — **the routekit test gate must compile** before S1.
**Success:** every command above either passes or its failure is documented as pre-existing; `cargo test -p pilcrow-routekit` compiles.
**Boundary:** change no product code.

---

## S1 — Reject removed page constants (routekit codegen)

**Goal:** `REVALIDATE`, `STREAMING`, `CACHE_TAGS`, `MAX_STALE`, `CACHE_VARY`, `PRERENDER` each produce a build error with a migration hint. (User decision: `PRERENDER` removed completely — no alias.)

**Files & changes:**
1. `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — in `fn instrument_frontmatter` (~L8), inside the `file.items.retain(|item| { … match ident.as_str() { … } })` block, **add match arms before the `_ => true` arm** for the six idents. The retain closure **cannot `return Err`** — follow the existing `promote_after_err` pattern: declare `let mut removed_const_err: Option<io::Error> = None;` next to `promote_after_err`, set it inside the new arms (and `return false` to strip the const), then **after** the retain block, mirror the existing `if let Some(err) = promote_after_err { return Err(err); }` with `if let Some(err) = removed_const_err { return Err(err); }`. Error messages must name the const + migration, e.g.:
   - `REVALIDATE` → "`REVALIDATE` was removed — use `#[revalidate(N)]` on a `LiveProp` field"
   - `PRERENDER` → "`PRERENDER` was removed — use `pub const PROMOTE_AFTER: u32 = 0`"
   - `STREAMING` / `CACHE_TAGS` / `MAX_STALE` / `CACHE_VARY` → "`<NAME>` was removed and is no longer supported"
2. `pilcrow/crates/routekit/src/fsr.rs` — remove any code path that maps `PRERENDER = true` to `route_promote_after`, and fix the stale comment near the `route_promote_after()` emission (currently reads "PROMOTE_AFTER / PRERENDER = true"). Find it: `grep -n PRERENDER pilcrow/crates/routekit/src/fsr.rs`. After S1, the only `PRERENDER` mentions in `routekit/src` should be the S1 error string + tests.
3. `pilcrow/crates/routekit/src/templating/codegen/tests.rs` — **flip the golden test** `streaming_constant_is_silently_stripped`: rename to `streaming_constant_is_build_error`, change the trailing `.expect("STREAMING should be ignored, not a build error")` to `.expect_err("STREAMING must be a build error")`, and assert `err.kind() == io::ErrorKind::InvalidData` and `err.to_string().contains("STREAMING")`. Add three sibling tests (`revalidate_constant_is_build_error`, `prerender_constant_is_build_error`, `cache_tags_constant_is_build_error`) by copying the same `TemplateCodegenInput { … }` literal and swapping the const in `rust_frontmatter`.

**Success:**
- `cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit` passes (flipped + 3 new tests green).
- `cargo build --manifest-path demo/Cargo.toml` and `--manifest-path address-book/Cargo.toml` stay green (no source page declares these consts).
- `grep -rn "PRERENDER" pilcrow/crates/routekit/src` shows only the error string and test idents.
**Boundary:** do not touch the `TRAILING_SLASH`/`LAYOUT`/`PROMOTE_AFTER`/`FSR_JSON` arms; do not change FSR baking.

---

## S2 — Registry + MCP validation for removed consts (four-layer)

**Goal:** MCP/registry reject the removed consts and suggest the migration.
**Files & changes:**
1. `pilcrow/registry.toml` — in the `page-options` feature (`id = "page-options"`): extend `constraints`, add `invalid_examples` for each removed const, and add `validation_rules` describing rejection + migration.
2. `pilcrow/tools/pilcrow-mcp/src/validation.rs` — in `fn validate_rust`, add checks mirroring the existing `findings.push(finding(...))` / `finding_with_line(...)` pattern: when `code.contains("const REVALIDATE")` (and each other name), push a `Finding` with the migration hint as the suggestion.
**Success:** `cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml` passes; a `validate_implementation` call on code containing `pub const REVALIDATE` returns a finding naming `#[revalidate(N)]`.
**Boundary:** do not change `docs.rs` (corpus is fixed `.md` docs).

---

## S3 — Reconcile docs to S1 (no code)

**Goal:** every doc agrees the consts are build errors.
**Files & changes:**
1. `pilcrow/AGENTS.md` — rewrite the ISR section, the "Per-page Options" block listing `PRERENDER`/`REVALIDATE`, and any "ISR (Incremental Static Regeneration)" prose to say these are **removed → build error**; point to `#[revalidate(N)]` / `PROMOTE_AFTER`.
2. Verify (and only fix if wrong): root `CLAUDE.md` "Removed modes" line and `.claude/rendering-models.md` (the "Removed modes" callout + the build-error table). They already claim build-error — after S1 that is finally true.
**Success:** `grep -rn "REVALIDATE\|PRERENDER\|STREAMING" pilcrow/AGENTS.md .claude/rendering-models.md CLAUDE.md` shows only "removed/build error" framing — no text presenting them as usable features.
**Boundary:** docs only.

---

## S4 — Delete dead `isr.rs`

**Goal:** remove the unwired ISR module.
**Files & changes:**
1. `pilcrow/crates/runtime/src/lib.rs` — remove the `pub mod isr;` line.
2. Delete `pilcrow/crates/runtime/src/isr.rs`.
3. Docs: remove the `isr.rs` mention in root `CLAUDE.md` ("…`isr.rs` (stripped to in-memory SSG cache only)…") and the ISR/`isr.rs` references + "Known Bug #6" row in `pilcrow/AGENTS.md`.
**Success:** `grep -rn "mod isr\b\|isr::\|IsrCache\|IsrHandle\|IsrCacheState" pilcrow/crates` returns nothing; `cargo build --manifest-path pilcrow/Cargo.toml` + `cargo test -p pilcrow-runtime` green.
**Boundary:** do not touch `fsr/` baking or the SSG bake path.

---

## S5 — Remove `baked_pages` + its `deferred.rs` support

**Goal:** delete the experimental baked-pages module (subsumed by `PROMOTE_AFTER`) and the `deferred.rs` types only it used.
**Files & changes:**
1. Delete directory `pilcrow/crates/runtime/src/baked_pages/`; remove its module declaration in `pilcrow/crates/runtime/src/lib.rs` (`grep -n baked_pages` there).
2. `pilcrow/crates/web/src/lib.rs` — remove the `#[cfg(feature = "experimental-baked-pages")]` block (the `experimental`/`baked_pages` re-export module, `BakedRoute`, `register_baked_fields`) and its `#[cfg(all(test, feature = "experimental-baked-pages"))]` test module.
3. `pilcrow/crates/web/Cargo.toml` — remove the `experimental-baked-pages` feature.
4. Delete `pilcrow/crates/web/examples/baked_prebake.rs` and `baked_ticket.rs`.
5. `pilcrow/crates/runtime/src/deferred.rs` — remove `PatchDelay` (~L230), `BakedProp<T>` (~L282), `BakedField` (~L394) and their impls. **KEEP** `__live_props_response`, `LiveTarget`, `LiveProp<T>` and everything above ~L228.
6. `pilcrow/crates/runtime/src/lib.rs` / re-exports — remove any `BakedField`/`BakedProp`/`PatchDelay` re-export.
7. `pilcrow/registry.toml` — delete the `experimental-baked-pages` feature entry; remove `deferred.rs` from the `live-props` feature's `source_refs`.
8. Delete `pilcrow/docs/experimental-baked-pages.md` if it exists.
**Success:** `grep -rn "baked_pages\|BakedPageStore\|BakedRoute\|BakedProp\|BakedField\|PatchDelay\|experimental-baked-pages" pilcrow/crates pilcrow/registry.toml` returns nothing; `cargo build --manifest-path pilcrow/Cargo.toml` (default features) green; `cargo test -p pilcrow-runtime` + `cargo test -p pilcrow-mcp` green.
**Boundary:** **KEEP** legacy `LiveProp`/`LiveTarget`/`__live_props_response` in `deferred.rs` (handled in S7/S8). Do not touch `fsr/`.

---

## S6 — Doc scrub for already-removed Deferred / Static-Export (no code)

**Goal:** stop docs implying `Deferred<T>`/`DeferredHtml`/Static-Export/baked-pages are touchable.
**Files & changes:** root `CLAUDE.md` (remove "do not open … `deferred.rs`" guidance now that baked types are gone; keep the "removed modes" list accurate), `pilcrow/AGENTS.md`, `.claude/rendering-models.md` — ensure these appear only as "removed." Remove any "open `baked_pages`/`deferred.rs`" instructions.
**Success:** no doc instructs opening deleted files; `Deferred<T>`/`DeferredHtml` appear only in "removed" lists.
**Boundary:** docs only.

---

## S7 — Migrate the legacy-`LiveProp` demos onto FSR (then they no longer need the legacy system)

**Decision (2026-05-29):** keep **only** the FSR `LiveProp<T>`. FSR is query-backed, but these demos are DB-less `tokio::watch` counters, so each must be **re-backed by a DB query** or **removed**. Do this BEFORE S8 so deleting the legacy system breaks nothing.

**Per-demo plan (confirm fate with the user where noted):**
- `demo/pages/demo/fsr/index.rs` (+ `.html`) — **rewrite as genuine FSR** (it is mislabeled today). Add `demo/pages/demo/fsr/live.rs` with a `Live` struct (`pub live_count: LiveProp<i64>`) and `Live::query` against a real counter table; a background job / action does the DB write + `invalidate!`. Template uses `s-live="live_count"`. This becomes the canonical FSR counter demo.
- `demo/pages/live/index.rs`, `demo/pages/demo/async-live/index.rs` (+ `islands/activity.rs`), `demo/pages/demo/combined/simple/index.rs`, `combined/complex/index.rs` — these only exist to showcase the **removed** push primitive. **Recommended:** convert `live/index.rs` to a DB-backed FSR counter (mirrors the `fsr/index.rs` pattern) as the single "live counter" showcase, and **remove** the now-redundant `async-live`/`combined` push demos (their FSR equivalents are already covered by `tickets/` + the rewritten `fsr/index`). **Confirm with the user before deleting any page.**
- For any page kept: drop `use`/imports of `pilcrow_web::LiveProp` (legacy) and `LiveProp::watch/initial`; switch to `use pilcrow_web::live::*` + the FSR `Live`/`live.rs` convention.

**Files:** the 6 demo `.rs` + `.html` listed above; new `live.rs` files; a small migration (counter table) if the executor chooses DB-backed counters; update the Demo App tutorial (`docs/Pilcrow-Silcrow Docs/Tutorials/Demo App/`) for any page that changes or is removed.
**Success:** `grep -rn "pilcrow_web::LiveProp\b\|LiveProp::watch\|LiveProp::initial\|LiveProp::poll\|LiveProp::stream" demo address-book` returns nothing (no source uses the legacy prop); `cargo build --manifest-path demo/Cargo.toml` green; kept demos render via FSR `s-live`.
**Boundary:** do not touch the `Vec<T>` list system or the FSR hub. Do not delete a demo page without user confirmation.

---

## S8 — Delete the legacy live infrastructure (one `LiveProp`, one hub)

**Goal:** with S7 done (no source uses the legacy `LiveProp`), remove the entire legacy live-props machinery. The FSR `LiveProp` + FSR hub are the only survivors → invariant #1 satisfied by construction. **KEEP the `Vec<T>` list system.**

**Delete:**
- `pilcrow/crates/runtime/src/deferred.rs` — remove `LiveProp<T>`, `LiveTarget`, `__live_props_response` (after S5 this file has nothing else; if empty, delete the file and its `pub mod deferred;` in `runtime/src/lib.rs`).
- `pilcrow/crates/runtime/src/live_props/` — delete the **legacy** files `model.rs` (legacy `LiveProp`/`LiveFieldData`/`LivePropExtract`), `broadcast.rs` (`LiveBroadcast`/`InvalidationEvent`), `store.rs` (`LivePageStore`), and `baking.rs` (`inject_live_slots`) **only if** a grep confirms FSR does not use them (FSR has its own baking). **KEEP** `list_broadcast.rs`, `list_row.rs`, `list_chunk.rs`. Update `live_props/mod.rs` to drop the deleted re-exports and keep the `List*` ones. Verify `dep.rs` ownership (FSR has its own `DependencyKey` in `fsr/`; the `live_props/dep.rs` one is legacy → delete if unused).
- `pilcrow/crates/runtime/src/lib.rs` — remove `pub use deferred::{__live_props_response, LiveProp, LiveTarget};` and any `live_props::{LiveProp, LiveFieldData, LivePropExtract, LiveBroadcast, LivePageStore, InvalidationEvent}` re-exports. **Keep** the `live_props::{ListBroadcast, ListChunkCache, InMemoryListChunkCache, ListPatchEvent, ListRow, list_chunk_key}` re-export line.
- `pilcrow/crates/web/src/lib.rs` — remove `pub use runtime::{__live_props_response, LiveProp, LiveTarget};` so the only `LiveProp` is `pilcrow_web::live::LiveProp` (FSR).
- `pilcrow/crates/macros/src/live_props_derive.rs` — delete the derive macro; remove its registration in `macros/src/lib.rs`.
- `pilcrow/crates/macros/src/handler.rs` — remove the `LivePropExtract`/`LivePageStore`/`LiveBroadcast` injection (search `live_props`); leave the rest of `#[handler]` intact.
- `pilcrow/crates/macros/src/invalidate_macro.rs` — if it targets `live_props::InvalidationEvent` (legacy), retarget it to FSR invalidation or delete if FSR's `invalidate!` supersedes it (FSR has its own `dep!`/`invalidate!`). Verify against `web/src/live.rs` re-exports.
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — remove the `__live_props_response` emission and the `data-pilcrow-live` anchor injection (search both); the `/__pilcrow/live{pattern}` route generation goes with it.
- `pilcrow/crates/runtime/src/start.rs` — remove the `/__pilcrow/live` route registration + `LiveBroadcast`/`LivePageStore` wiring (search those symbols). **Keep** the FSR hub registration (`/__pilcrow/fsr`).
- `pilcrow/crates/runtime/assets/silcrow.js` — remove `initLiveElements` + `data-pilcrow-live` discovery (legacy per-route SSE). **KEEP** the single FSR hub `EventSource`, `createSseHub`/`connectSseHub`, the WS hub, and the `silcrow:navigate`/`silcrow:load` reconnect lifecycle. Run `node silcrow/build.js` after.
- `pilcrow/registry.toml` — remove the `live-props` feature entry (superseded by `fsr`); scrub any remaining `deferred.rs`/`live_props` legacy `source_refs`.

**Boundary (critical):** **DO NOT** delete or weaken: the `Vec<T>` list system (`list_broadcast.rs`/`list_row.rs`/`list_chunk.rs`), the FSR hub (`fsr_hub_handler`), the WS hub, or SSE/WS multiplexing. One `EventSource` per client must remain.
**Success:**
- `grep -rn "deferred::LiveProp\|live_props::model\|LivePropExtract\|LiveBroadcast\|LivePageStore\|__live_props_response\|data-pilcrow-live\|/__pilcrow/live\b" pilcrow/crates` returns nothing.
- Exactly one `LiveProp` resolvable: `pilcrow_web::live::LiveProp` (FSR). `grep -rn "pub use .*LiveProp" pilcrow/crates` shows only the `runtime::fsr` path.
- `cargo build --manifest-path pilcrow/Cargo.toml` + `demo` + `address-book` green; `cargo test -p pilcrow-routekit -p pilcrow-runtime` + `cargo test -p pilcrow-mcp` green; `node silcrow/build.js` succeeds.
- `live_props::ListRow`/`ListBroadcast` still compile and are still re-exported.
- **Out-of-band (human, needs PG+Redis):** load demo + address-book; confirm exactly one live connection per client and FSR `s-live` updates still patch.

---

## S9 — Final reconciliation & ledger

**Files & changes:** `FSR_IMPLEMENTATION_PLAN-2.md` (correct Phase 9/10 to "`PRERENDER` removed, not aliased"; reconcile `LiveProps`→`LiveProp` and `#[pilcrow::promote_after]`→`pub const PROMOTE_AFTER`); `roadmap-2026-05-29.md` (mark E1 + closed items done); memory `project_framework_drift.md` (drift resolved) + `project_architecture_decisions.md` (one LiveProp, one hub); run `graphify update .` (then restore `graphify-out/.graphify_root`), code-review-graph incremental update, and `cargo test -p pilcrow-mcp`.
**Success:** roadmap E1 checked off; no doc/source drift remains; graphs current.

---

## Decisions

- **RESOLVED (2026-05-29):** keep **only** the FSR `LiveProp<T>`. The legacy push-based `LiveProp` and its per-route SSE are deleted entirely (S8); demos migrate to query-backed FSR (S7). The `Vec<T>` list system is kept and improved later.
- **Remaining sub-decision (S7):** the fate of the pure push-showcase demos (`live/`, `async-live/`, `combined/*`) — convert each to a DB-backed FSR counter, or remove the redundant ones. Recommendation in S7: rewrite `demo/fsr/index.rs` as real FSR, convert `live/index.rs` to a DB-backed FSR counter, remove the rest. **Confirm before deleting any demo page.**
