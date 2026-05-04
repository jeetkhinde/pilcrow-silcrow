# Baked Pages With Patchable Slots

This is a design note, not implemented behavior.

## Thesis

Pilcrow can plausibly unify SSG, ISR, static export, lazy cache fill, and event-driven revalidation under a "Baked Page" model:

1. Render a page into durable HTML once.
2. Mark selected dynamic regions with Pilcrow-owned slot markers.
3. Record a manifest from dependency keys to `(page_key, slot_id)` targets.
4. On data change, recompute only affected slot fragments.
5. Patch the durable baked HTML atomically.
6. Serve future requests by reading the already-updated baked HTML.

This model should not replace SSR or streaming. It is a durable-page cache with optional fine-grained invalidation. The central difference from today's ISR is that invalidation can target a named slot instead of deleting or re-rendering the whole page.

## Current Fit

Current SSG is already implemented as "pre-render into the ISR cache." `pilcrow_start()` calls `start_with_prerender()`, which creates one shared `IsrCache`, runs generated `__pilcrow_prerender_all(&cache)`, and then attaches the cache handle to requests. Static export also runs the same prerender path, then writes every cache entry as `<dir>/<key>/index.html`.

Current ISR stores whole HTML strings keyed by request path/query/vary. The runtime cache has memory and filesystem backends. Filesystem persistence writes JSON entries through a temp file and `rename`, which is already the right atomic-write shape for durable baked HTML.

Current LiveProp and Deferred support prove Pilcrow can own target markers safely:

- `LiveProp<T>` fields are detected in `instrument.rs`; `compiler.rs` rewrites `{{ field }}` to `<span data-pilcrow-live-field="field">{{ field }}</span>`.
- `AsyncValue<T>` fields are detected the same way; template interpolations become `data-pilcrow-async-value` targets.
- `AsyncHtml` shell rendering emits a text marker like `__pilcrow_html_slot_field__`, then generated handler code replaces that marker with `<span data-pilcrow-async-html="field">...</span>`.

So the slot-marker side is aligned with existing patterns. The missing pieces are:

- a public way for developers to declare dependency keys,
- codegen metadata that records which page slots depend on which keys,
- generated per-slot recompute functions,
- a durable baked-page store/index,
- an atomic patcher that updates only Pilcrow-owned slots.

## Relevant Files

- `pilcrow/crates/runtime/src/isr.rs` — current whole-page cache, filesystem persistence, path/tag invalidation, export entries.
- `pilcrow/crates/runtime/src/start.rs` — startup prerender and export flow.
- `pilcrow/crates/web/src/lib.rs` — `pilcrow_start()` and `pilcrow_export()` generated public helpers.
- `pilcrow/crates/routekit/src/templating/page_options.rs` — parsed page constants such as `REVALIDATE`, `PRERENDER`, `STREAMING`.
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — parses page constants, detects `LiveProp`, `AsyncValue`, `AsyncHtml`, rewrites templates.
- `pilcrow/crates/routekit/src/templating/compiler.rs` — current safe target injection for live and deferred scalar fields.
- `pilcrow/crates/routekit/src/templating/codegen/templates.rs` — emits Askama modules and render functions.
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — emits SSR, ISR, SSG, streaming, deferred, and live handlers.
- `pilcrow/crates/runtime/src/deferred.rs` — `AsyncValue`, `AsyncHtml`, `LiveProp`, streaming patch helpers.
- `silcrow/src/silcrow.js` — browser-side SSE and DOM patch infrastructure if slot updates are mirrored to open documents.

## Proposed Developer API

Start explicit and narrow. Avoid trying to infer dependencies from arbitrary Rust.

```rust
use pilcrow_web::{BakedSlot, Dep};

pub const BAKE: pilcrow_web::Bake = pilcrow_web::Bake::lazy();

pub struct Props {
    pub ticket_status: BakedSlot<String>,
    pub total_tickets: BakedSlot<u64>,
}

pub async fn load(page: Page) -> AppResult<Props> {
    let ticket_id = page.params.id;
    Ok(Props {
        ticket_status: BakedSlot::value(load_status(ticket_id).await?)
            .slot("ticket_status")
            .depends_on(Dep::new("TicketStatus").param("ticket_id", ticket_id)),
        total_tickets: BakedSlot::value(load_count(9).await?)
            .slot("total_tickets")
            .depends_on(Dep::new("TicketCount").param("org_id", 9)),
    })
}
```

Template authors would write normal interpolation:

```html
<span>{{ ticket_status }}</span>
```

Codegen would rewrite it to a Pilcrow-owned slot:

```html
<span data-pilcrow-slot="ticket_status">Open</span>
```

For HTML fragments:

```rust
pub struct Props {
    pub ticket_summary: BakedHtml,
}
```

`BakedHtml` should use a container marker and replace children, following `AsyncHtml`, not regex over arbitrary HTML.

## Dependency Keys

Dependency keys need to be stable strings, but the API should avoid hand-concatenation. Internally they can normalize to:

```text
TicketStatus:ticket_id=123
TicketCount:org_id=9
```

Public invalidation could live beside today's ISR handle:

```rust
req.cache.emit("TicketStatus:ticket_id=123").await;
req.cache.emit_dep(Dep::new("TicketStatus").param("ticket_id", 123)).await;
```

Naming note: `emit` implies event-driven patching. `revalidate_dep` may fit better if it can fall back to whole-page revalidation.

## Manifest Shape

At bake time Pilcrow should persist two linked records:

```text
page_key: /tickets/123
route_module: page_tickets_id
bake_policy: lazy
html_path/cache_key: ...
slots:
  ticket_status:
    kind: text
    depends_on:
      - TicketStatus:ticket_id=123
    selector:
      attr: data-pilcrow-slot
      value: ticket_status
```

And an inverted index:

```text
TicketStatus:ticket_id=123 -> [(/tickets/123, ticket_status)]
TicketCount:org_id=9 -> [(/dashboard, total_tickets)]
```

For a first version, this can live in memory plus filesystem JSON next to baked HTML. Later, SQLite is likely a better default for multi-process-safe lookup and transactional updates.

## Slot Recompute

The hard part is recomputing only a slot without re-running the full page render. There are three possible tiers:

1. **Minimum tier: field-value slots.** `BakedSlot<T>` stores a recompute closure/factory from `load()` output. On dependency invalidation, generated code re-runs `load()` for the affected page key but extracts only the slot value before patching the HTML. This still pays data-load cost, but proves durable slot patching and manifest/indexing.
2. **Fragment tier: explicit slot loaders.** Developers define a slot function:
   ```rust
   pub async fn ticket_status_slot(page: Page) -> AppResult<BakedSlot<String>> { ... }
   ```
   Codegen can call this directly on invalidation. This is the cleanest route to true partial recompute.
3. **Template fragment tier: typed fragments.** Reuse configured `[[fragments]]` or co-located fragment modules as slot renderers. A page slot points to a fragment render function, and dependency invalidation re-renders that fragment only.

The minimum proof should use tier 2 for at least one text slot. It avoids pretending Rust closures captured during `load()` can be serialized across process restarts.

## Safe HTML Patching

This should not use generic regex. Use one of:

- `lol_html` streaming HTML rewriter for server-side mutation,
- `kuchiki` / `html5ever` DOM parse and serialize,
- a custom marker-pair format that makes replacement byte-range safe.

Recommended marker format for durable patching:

```html
<!--pilcrow-slot:start ticket_status kind=text hash=...-->
<span data-pilcrow-slot="ticket_status">Open</span>
<!--pilcrow-slot:end ticket_status-->
```

The public visible marker remains `data-pilcrow-slot`; the comments give the disk patcher unambiguous boundaries. For text slots, set escaped text content. For HTML slots, replace children inside the owned container, not the container itself, so attributes remain stable.

Atomic write path:

1. Read baked HTML.
2. Parse or locate Pilcrow-owned marker boundaries.
3. Validate exactly one matching slot boundary.
4. Patch into a new string.
5. Write `page.tmp.<nonce>`.
6. `fsync` file if needed for production durability.
7. `rename` over the old file.
8. Update in-memory cache entry after the durable write succeeds.

Concurrency needs per-page locking, not only per-slot locking, because multiple slot patches rewrite the same file. Coalesce dependency events into one page update when possible.

## Bake Timing As Policy

The Baked Page model can make timing a policy instead of a separate rendering concept:

| Timing | Use case | Source of page keys |
|--------|----------|---------------------|
| Build-time bake/export | marketing, docs, known public routes | static routes, `entries()`, configured routes |
| Startup bake | app-hosted public pages needing warm cache | `BAKE = Bake::startup()` or `PRERENDER` compatibility |
| Background warm bake | expensive but predictable routes | generated warmer task from `entries()` or app-provided warmer |
| Lazy first-request bake | huge ID spaces, tenant dashboards, entity detail pages | actual request path/query/vary |
| Event-driven patch | data changes after a page is baked | dependency index lookup |

This suggests replacing or supplementing `PRERENDER` / `REVALIDATE` with a single page option over time:

```rust
pub const BAKE: Bake = Bake::build();
pub const BAKE: Bake = Bake::startup();
pub const BAKE: Bake = Bake::lazy();
pub const BAKE: Bake = Bake::warm();
```

For compatibility, current constants can map into the model:

- `PRERENDER = true` -> startup bake with infinite freshness.
- `REVALIDATE = n` -> lazy bake with TTL-based whole-page revalidation.
- `PRERENDER + REVALIDATE` -> startup bake with TTL-based whole-page revalidation.
- new slot dependencies -> event-driven slot patching when possible, whole-page fallback otherwise.

## Architecture Sketch

Runtime:

- Add `BakedPageStore` next to or inside `IsrCache`.
- Store page HTML and metadata separately from ISR `CacheEntry`, or evolve `CacheEntry` to include optional `manifest`.
- Add `DependencyIndex` mapping dep keys to page slots.
- Add `BakedHandle` / extend `IsrHandle` with dependency invalidation.
- Add per-page locks for patch operations.

Routekit:

- Parse new page constants or types in `instrument.rs`.
- Detect `BakedSlot<T>` / `BakedHtml` fields.
- Rewrite template interpolations to `data-pilcrow-slot`.
- Emit generated slot recompute functions in `templates.rs` or `app_module.rs`.
- Extend `__pilcrow_prerender_all` so proactive bake policies use the same durable store.
- Extend handlers so lazy bake checks durable page first, otherwise renders and records manifest.

Silcrow:

- Optional only. If the server patches baked HTML, future requests are solved without browser code.
- For open tabs, dependency patch events can reuse the same slot identity:
  ```js
  document.querySelectorAll('[data-pilcrow-slot="ticket_status"]')
  ```
  and replace text/children from SSE.
- This can be a later layer; do not block server-side baked-page proof on browser live patching.

## Minimum Proof Of Concept

1. Add a runtime-only `BakedPageStore` with filesystem HTML write/read and atomic full-page store.
2. Add a manual/generated page with one explicit text slot marker:
   ```html
   <!--pilcrow-slot:start ticket_status kind=text-->
   <span data-pilcrow-slot="ticket_status">Open</span>
   <!--pilcrow-slot:end ticket_status-->
   ```
3. Persist a JSON manifest:
   ```json
   { "page": "/tickets/123", "slots": { "ticket_status": ["TicketStatus:ticket_id=123"] } }
   ```
4. Add `emit_dep()` that finds matching slots and calls a hard-coded generated slot recompute function.
5. Patch the HTML file atomically and update the memory copy.
6. Serve the baked file on the next GET.
7. Add one sandbox route that demonstrates:
   - startup/proactive bake for an `entries()` route,
   - lazy bake for `/tickets/[id]`,
   - action handler changes the ticket status and emits the dependency key,
   - next request reads the patched baked HTML without full-page SSR.

This proves the model without solving every cache provider, browser SSE mirroring, or automatic dependency inference.

## Main Risks

- **Request-specific data leakage.** Baked pages must require stable cache keys and explicit vary rules. Do not allow auth/session-derived pages unless `CACHE_VARY` or a tenant/user scope is explicit.
- **Slot recompute context.** Recomputing a slot needs route params, query, locale, maybe tenant context. The manifest must store enough safe context to reconstruct a synthetic `Req` or `Page`.
- **Layout dependencies.** A page slot may depend on layout props or shared chrome. Minimum version should only patch page-owned slots, then later support layout-owned slots.
- **HTML parser cost.** Full parse/serialize for every slot event may be expensive. Marker-boundary patching is faster but needs strict validation.
- **Multi-process deployment.** Filesystem JSON plus process memory is enough for single-node proof. SQLite or Redis-like coordination is needed for many workers.
- **Deferred and streaming interactions.** `AsyncHtml` already owns slots, but it is request-streaming, not durable. Initial implementation should reject or whole-page fallback for deferred/streaming pages.

## Recommendation

Build the first version as an extension of the existing ISR/SSG pipeline rather than a parallel system. Current SSG already warms `IsrCache`; current ISR already has path/tag invalidation, filesystem persistence, synthetic requests, and atomic JSON writes. Add baked-page manifests and slot patching there first.

The most convincing proof is: `entries()` pages bake proactively, large dynamic pages bake lazily, and both can be updated by the same dependency-key event. Whole-page re-render remains the fallback, but text/HTML slots with explicit slot loaders can take the fast path.
