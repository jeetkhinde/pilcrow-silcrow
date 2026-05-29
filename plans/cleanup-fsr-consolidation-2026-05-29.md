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
- **Two `LiveProp` types collide:** `pilcrow_web::LiveProp` (legacy, from `runtime/src/lib.rs` `pub use deferred::{…, LiveProp, …}`) vs `pilcrow_web::live::LiveProp` (FSR, from `web/src/live.rs` `pub use runtime::fsr::{…, LiveProp, …}`). Legacy is used by demo pages: `demo/pages/live/index.rs`, `demo/pages/demo/async-live/index.rs`, `demo/pages/demo/async-live/islands/activity.rs`, `demo/pages/demo/combined/simple/index.rs`, `demo/pages/demo/combined/complex/index.rs`, `demo/pages/demo/fsr/index.rs`.
- **Two live SSE topologies:** FSR = one hub (`fsr_hub_handler`, `/__pilcrow/fsr`). Legacy = per-route `/__pilcrow/live{pattern}` (emitted in `app_module.rs` via `__live_props_response`, discovered by `data-pilcrow-live` anchors + `initLiveElements` in `silcrow.js`). `silcrow.js` opens exactly **one** `EventSource` today.

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
 └─ S7 → S8 → S9        (live consolidation; S8 needs the Fold/Rename decision)
```
S1–S6 are safe and can ship first. S7+ is the large consolidation.

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

## S7 — Transport: converge legacy live SSE onto the single hub (invariant #1; LARGE)

**Goal:** legacy live pages stop opening per-route `/__pilcrow/live{pattern}` connections and ride the one multiplexed hub. One `EventSource` per client.
**Files (locate by symbol):**
- `pilcrow/crates/runtime/src/fsr/hub.rs` — `fsr_hub_handler` (`/__pilcrow/fsr`); the target hub. Understand its subscribe/route+slots protocol first.
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — the `data-pilcrow-live` anchor injection (search `data-pilcrow-live`) and `__live_props_response` emission (search `__live_props_response`). Replace per-route live wiring with hub subscription.
- `pilcrow/crates/runtime/assets/silcrow.js` — `initLiveElements` + `data-pilcrow-live` discovery; route through the single existing `EventSource`/`connectSseHub`. After editing, run `node silcrow/build.js`.
- `pilcrow/crates/runtime/src/start.rs` — the `/__pilcrow/live` route registration + `LiveBroadcast`/`LivePageStore` (search those symbols); retire the per-route endpoint once the hub serves legacy slots.
**Success:** `cargo build` (pilcrow + demo + address-book) green; `grep -rn "/__pilcrow/live{" pilcrow/crates` and `data-pilcrow-live` per-route wiring removed; **one** `EventSource` in `silcrow.js`. **Out-of-band (human, needs PG+Redis):** load a legacy live demo page, confirm exactly one live connection in devtools and that updates still patch.
**Boundary:** **DO NOT** remove `createSseHub`/`connectSseHub`/WS hub or reduce multiplexing. Keep the `silcrow:navigate` close / `silcrow:load` reopen reconnect lifecycle.

---

## S8 — Type unification: one `LiveProp` (DECISION REQUIRED — Fold vs Rename)

**Goal:** end the `pilcrow_web::LiveProp` vs `pilcrow_web::live::LiveProp` collision. **Blocked on the user's choice:**
- **Option F (Fold):** one `LiveProp<T>` with both producers — `::watch(rx)` (push) and the FSR DB-query path — all flowing through the S7 hub. Migrate the 6 demo pages, delete `deferred.rs`'s `LiveProp`/`LiveTarget`.
- **Option R (Rename):** keep both capabilities; rename the legacy type (e.g. `PushProp`/`StreamProp`) so names stop colliding; both ride the S7 hub.
**Files (either option):** `pilcrow/crates/runtime/src/lib.rs` (the `pub use deferred::{… LiveProp …}`), `pilcrow/crates/web/src/lib.rs` (`pub use runtime::{… LiveProp …}` re-export) and `web/src/live.rs`; `#[handler]` macro (`pilcrow/crates/macros/src/handler.rs`), `pilcrow/crates/macros/src/live_props_derive.rs`, `invalidate_macro.rs`; the 6 demo pages listed in Ground truth; `pilcrow/registry.toml` (`live-props` ↔ `fsr`).
**Success:** exactly one `LiveProp` path resolvable; `cargo build` + `cargo test` (routekit, runtime, mcp) green; demo pages compile against the unified surface; `node silcrow/build.js` if JS touched.
**Boundary:** preserve the push capability (broadcast-channel updates) regardless of option; keep one hub.

---

## S9 — Final reconciliation & ledger

**Files & changes:** `FSR_IMPLEMENTATION_PLAN-2.md` (correct Phase 9/10 to "`PRERENDER` removed, not aliased"; reconcile `LiveProps`→`LiveProp` and `#[pilcrow::promote_after]`→`pub const PROMOTE_AFTER`); `roadmap-2026-05-29.md` (mark E1 + closed items done); memory `project_framework_drift.md` (drift resolved) + `project_architecture_decisions.md` (one LiveProp, one hub); run `graphify update .` (then restore `graphify-out/.graphify_root`), code-review-graph incremental update, and `cargo test -p pilcrow-mcp`.
**Success:** roadmap E1 checked off; no doc/source drift remains; graphs current.

---

## Open decision before S8

**Fold vs Rename** the legacy push-based `LiveProp`. Everything through **S7** is unblocked and preserves the one-connection-per-client guarantee regardless of the choice.
