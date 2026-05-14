# Graph Report - pilcrow-silcrow  (2026-05-14)

## Corpus Check
- 42 files · ~27,670 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 922 nodes · 1630 edges · 81 communities (58 shown, 23 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 54 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `7630ab75`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Baked Pages Store Engine|Baked Pages Store Engine]]
- [[_COMMUNITY_Baked Pages Request Handling|Baked Pages Request Handling]]
- [[_COMMUNITY_Config & Cache Settings|Config & Cache Settings]]
- [[_COMMUNITY_Live Props Core & HTML Injection|Live Props Core & HTML Injection]]
- [[_COMMUNITY_Sandbox E-commerce UI|Sandbox E-commerce UI]]
- [[_COMMUNITY_Agent Context Docs|Agent Context Docs]]
- [[_COMMUNITY_Rendering Mode Demos|Rendering Mode Demos]]
- [[_COMMUNITY_Live & React Island Demos|Live & React Island Demos]]
- [[_COMMUNITY_Live Props DB Store|Live Props DB Store]]
- [[_COMMUNITY_ISR Page Handlers|ISR Page Handlers]]
- [[_COMMUNITY_ISRSSG Pages|ISR/SSG Pages]]
- [[_COMMUNITY_Handler Macro Logic|Handler Macro Logic]]
- [[_COMMUNITY_API Route Handlers|API Route Handlers]]
- [[_COMMUNITY_Proc-macro Entry Points|Proc-macro Entry Points]]
- [[_COMMUNITY_Counter Feature|Counter Feature]]
- [[_COMMUNITY_Baked Pages Architecture Docs|Baked Pages Architecture Docs]]
- [[_COMMUNITY_Product Grid Page|Product Grid Page]]
- [[_COMMUNITY_Page Templates|Page Templates]]
- [[_COMMUNITY_App Entry Points|App Entry Points]]
- [[_COMMUNITY_SSG Page Handlers|SSG Page Handlers]]
- [[_COMMUNITY_BlogPosts Page|Blog/Posts Page]]
- [[_COMMUNITY_SolidJS Integration|SolidJS Integration]]
- [[_COMMUNITY_PilcrowProps Derive Macro|PilcrowProps Derive Macro]]
- [[_COMMUNITY_App Lifecycle Hooks A|App Lifecycle Hooks A]]
- [[_COMMUNITY_App Lifecycle Hooks B|App Lifecycle Hooks B]]
- [[_COMMUNITY_Counter Page Handler|Counter Page Handler]]
- [[_COMMUNITY_Timestamp Page Handler|Timestamp Page Handler]]
- [[_COMMUNITY_SSG Page Handler|SSG Page Handler]]
- [[_COMMUNITY_dep! Macro|dep! Macro]]
- [[_COMMUNITY_invalidate! Macro|invalidate! Macro]]
- [[_COMMUNITY_Static Page Handler|Static Page Handler]]
- [[_COMMUNITY_Codegen Build Scripts|Codegen Build Scripts]]
- [[_COMMUNITY_App Shell & Navigation|App Shell & Navigation]]
- [[_COMMUNITY_Error & Fallback Pages|Error & Fallback Pages]]
- [[_COMMUNITY_Sandbox Main|Sandbox Main]]
- [[_COMMUNITY_SolidJS Store|SolidJS Store]]
- [[_COMMUNITY_Path Param Matching|Path Param Matching]]
- [[_COMMUNITY_App Hooks|App Hooks]]
- [[_COMMUNITY_App Entry & Macro|App Entry & Macro]]
- [[_COMMUNITY_Counter Data Layer|Counter Data Layer]]
- [[_COMMUNITY_Live Props Module Root|Live Props Module Root]]
- [[_COMMUNITY_About Page Handler|About Page Handler]]
- [[_COMMUNITY_Greeting React Island|Greeting React Island]]
- [[_COMMUNITY_User Card Handler|User Card Handler]]
- [[_COMMUNITY_Sandbox Entry Point|Sandbox Entry Point]]
- [[_COMMUNITY_Card UI Component|Card UI Component]]
- [[_COMMUNITY_Demos Index Page|Demos Index Page]]
- [[_COMMUNITY_Mod Root|Mod Root]]
- [[_COMMUNITY_Handler Live Attribute|Handler Live Attribute]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 55|Community 55]]
- [[_COMMUNITY_Community 56|Community 56]]
- [[_COMMUNITY_Community 57|Community 57]]
- [[_COMMUNITY_Community 58|Community 58]]
- [[_COMMUNITY_Community 59|Community 59]]
- [[_COMMUNITY_Community 60|Community 60]]
- [[_COMMUNITY_Community 63|Community 63]]
- [[_COMMUNITY_Community 65|Community 65]]
- [[_COMMUNITY_Community 66|Community 66]]
- [[_COMMUNITY_Community 67|Community 67]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 71|Community 71]]
- [[_COMMUNITY_Community 72|Community 72]]
- [[_COMMUNITY_Community 73|Community 73]]
- [[_COMMUNITY_Community 74|Community 74]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 76|Community 76]]
- [[_COMMUNITY_Community 77|Community 77]]
- [[_COMMUNITY_Community 78|Community 78]]
- [[_COMMUNITY_Community 80|Community 80]]

## God Nodes (most connected - your core abstractions)
1. `BakedPageStore` - 47 edges
2. `html_path()` - 22 edges
3. `rebake_page()` - 17 edges
4. `text_slot()` - 17 edges
5. `build_time_fragment_composed_prebakes_body_and_serves_composed_hit()` - 17 edges
6. `FSR — Field-Selective Rendering: Implementation Plan` - 17 edges
7. `CLAUDE.md — pilcrow-silcrow workspace` - 17 edges
8. `CLAUDE.md — pilcrow-silcrow workspace` - 16 edges
9. `BakedRouteDeclaration` - 15 edges
10. `patch_slot()` - 15 edges

## Surprising Connections (you probably didn't know these)
- `SSG PRERENDER constant pattern` --semantically_similar_to--> `SSR Streaming: shell-first with deferred patch via window.__ps`  [INFERRED] [semantically similar]
  pilcrow-demos/pages/ssg/index.rs → pilcrow-demos/pages/streaming/index.html
- `SSG PRERENDER constant pattern` --conceptually_related_to--> `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.rs → sandbox/pages/(admin)/_layout.html
- `pilcrow-demos build.rs` --semantically_similar_to--> `sandbox build.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/build.rs → sandbox/build.rs
- `pilcrow-demos hooks.rs` --semantically_similar_to--> `sandbox hooks.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/hooks.rs → sandbox/hooks.rs
- `load()` --calls--> `get()`  [INFERRED]
  pilcrow-demos/pages/isr/index.rs → demo/api/products.rs

## Hyperedges (group relationships)
- **Invalidation pipeline: dep! → invalidate! → LivePageStore → LiveBroadcast → SSE clients** — live_props_dep_dep_macro, macros_invalidate_macro_expand, live_props_store_invalidate_dep_key, live_props_broadcast_livebroadcast, live_props_broadcast_invalidationevent [INFERRED 0.95]
- **Handler live write pipeline: #[handler(live)] → LivePropsExtract → LivePageStore::write + increment_hit** — macros_handler_expand, live_props_model_livepropsextract, live_props_store_write_live_fields, live_props_store_increment_hit [INFERRED 0.95]
- **#[derive(PilcrowProps)] → LivePropsExtract impl → LiveFieldData extraction** — macros_lib_derive_pilcrow_props, macros_live_props_derive_expand, live_props_model_livepropsextract, live_props_model_livefielddata [EXTRACTED 1.00]
- **DB schema: pilcrow_cache + pilcrow_routes tables + GIN + stale indexes** — migration_001_pilcrow_cache, migration_001_pilcrow_routes, migration_002_gin_index, migration_002_stale_index [EXTRACTED 1.00]
- **Startup registration: start_with_adapter registers LiveBroadcast + LivePageStore as Axum extensions** — runtime_start_start_with_adapter, runtime_start_live_props_registration, live_props_broadcast_livebroadcast, live_props_store_livepagestore [EXTRACTED 1.00]

## Communities (81 total, 23 thin omitted)

### Community 0 - "Baked Pages Store Engine"
Cohesion: 0.05
Nodes (91): add_index_slot(), add_reverse_index_entry(), atomic_temp_file_is_not_served(), baked_html_response(), baked_root(), BakedArtifactMode, BakedFragment, BakedLayout (+83 more)

### Community 1 - "Baked Pages Request Handling"
Cohesion: 0.06
Nodes (38): ApiResponse, escape(), get(), Params, Product, render_cards(), router(), load() (+30 more)

### Community 2 - "Config & Cache Settings"
Cohesion: 0.07
Nodes (39): LiveConfig, PilcrowConfig, clone_shares_same_channel(), InvalidationEvent, LiveBroadcast, multiple_subscribers_all_receive_event(), no_subscribers_send_does_not_panic(), subscriber_receives_sent_event() (+31 more)

### Community 3 - "Live Props Core & HTML Injection"
Cohesion: 0.1
Nodes (45): CartAtom, CartBadge(), CartClearButton(), cartFormSchema, CartFormValues, Counter(), CreateState, DirectAddToCartForm() (+37 more)

### Community 4 - "Sandbox E-commerce UI"
Cohesion: 0.06
Nodes (33): BackendConfig, CacheConfig, CacheProvider, ClientRuntimeConfig, default_backend_host(), default_backend_port(), default_backend_url(), default_image_cache_dir() (+25 more)

### Community 5 - "Agent Context Docs"
Cohesion: 0.05
Nodes (41): App startup (promote_after = 0 or absent), code:block1 (promote_after = 0 or absent  → SSG  (bake at startup, surgic), code:block10 (Request arrives), code:block11 (pilcrow::invalidate!(dep!(tickets, id, 123))), code:block12 (pilcrow_start()), code:block13 (SSG              ★★★★★  Pilcrow matches — promoted routes ar), code:block3 (pages/), code:rust (use pilcrow::live::*;) (+33 more)

### Community 6 - "Rendering Mode Demos"
Cohesion: 0.05
Nodes (39): code:bash (git mv sandbox demo), code:bash (mkdir -p demo/pages/demo/fsr), code:rust (use std::sync::OnceLock;), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1 | grep -E "), code:html (---), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5), code:bash (cargo run --manifest-path demo/Cargo.toml &), code:bash (git add demo/pages/demo/fsr/) (+31 more)

### Community 7 - "Live & React Island Demos"
Cohesion: 0.08
Nodes (35): Agent Instructions, CLAUDE.md — pilcrow-silcrow workspace, First rule, graphify, Graphify first, OpenCode / Agent Instructions, Pilcrow / Silcrow relationship, Read Claude.md at workspace root. (+27 more)

### Community 8 - "Live Props DB Store"
Cohesion: 0.09
Nodes (22): code:javascript (Object.keys(d).forEach(function(k) {), code:rust (// live.rs), code:html (<span s-live="ticket_status">{{ live.ticket_status.value }}<), code:rust (// live.rs), code:html (<!-- Initial render: SSR-baked from Live struct -->), code:rust (impl Live {), code:rust (ticket_status: LiveProp {), code:rust (ticket_badge: LiveProp {) (+14 more)

### Community 9 - "ISR Page Handlers"
Cohesion: 0.13
Nodes (22): AsyncValue/AsyncHtml deferred field pattern, SSG Demo Page, SSR Demo Page, Streaming SSR Demo Page, ISR REVALIDATE constant pattern, data::counter::increment (atomic render counter), pilcrow-demos pages/deferred/index.rs, pilcrow-demos pages/isr/index.rs (+14 more)

### Community 10 - "ISR/SSG Pages"
Cohesion: 0.16
Nodes (13): TicketUpdate, update(), load(), Props, TicketRow, pool(), init(), load() (+5 more)

### Community 11 - "Handler Macro Logic"
Cohesion: 0.13
Nodes (13): current(), increment(), load(), now_utc(), Props, Counter(), Props, load() (+5 more)

### Community 12 - "API Route Handlers"
Cohesion: 0.14
Nodes (16): Counter React Component (all hook demos), FragmentGrid React Component, Live Page Handler (SSE counter), LiveProp Pattern — tokio watch channel for live SSE props, Product Grid Widget Handler, <react> custom element for mounting React islands with strategy attribute, React Islands Example README, code:toml ([routing]) (+8 more)

### Community 13 - "Proc-macro Entry Points"
Cohesion: 0.12
Nodes (16): 1. DB schema — `pilcrow_fsr` table, 2. `StaleSlot` and `upsert_slot`, 3. Watcher — use column_name for value extraction, 4. Codegen — wire column_name into slot registration, 5. Tests for renamed SQL columns, code:rust (// pages/tickets/[id]/live.rs), code:sql (ALTER TABLE pilcrow_fsr ADD COLUMN column_name TEXT;), code:sql (INSERT INTO pilcrow_fsr (route, slot, column_name, query, ..) (+8 more)

### Community 14 - "Counter Feature"
Cohesion: 0.13
Nodes (14): FSR Implementation Audit Report, Phase 10 — hit_count + promotion, Phase 11 — Re-exports + developer surface, Phase 1 — DB Migration, Phase 2 — `LiveProp<T>`, `DependencyKey`, `dep!`, Phase 3 — `PilcrowLive` trait + `live_query!`, Phase 4 — `live.rs` discovery + routekit codegen, Phase 5 — HTML baking + `s-live` shell slots (+6 more)

### Community 15 - "Baked Pages Architecture Docs"
Cohesion: 0.13
Nodes (14): code:rust (#[test]), code:bash (git add pilcrow/crates/runtime/src/fsr/live_props.rs), code:rust (/// Returns the FSR inline patch script. Exposed for tests o), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-rou), code:rust (let fsr_script = "(function(){var __fsr_route=window.locatio), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-rou), code:bash (git add pilcrow/crates/routekit/src/templating/codegen/app_m), code:rust (/// A field whose value is tracked, cached, and live-patched) (+6 more)

### Community 16 - "Product Grid Page"
Cohesion: 0.14
Nodes (13): Added, App Identity, Deleted (from pilcrow-demos, not migrated), Demo App Consolidation Design, `/demo/fsr` Page Design, `/demo/react-island` Page Design, File Operations Summary, Kept (from sandbox, unchanged) (+5 more)

### Community 17 - "Page Templates"
Cohesion: 0.15
Nodes (11): Background, code:block1 (promote_after = 0 or absent  → SSG  (bake at startup, surgic), code:block2 (crates/core         — AppError, AppResult, PilcrowConfig), code:block3 (crates/runtime/src/fsr/          — LiveProp, DependencyKey, ), code:block31 (live.rs), code:block32 (Phase 1   DB migration), Complete developer surface (nothing else needed), Crate / file map (existing, do not change) (+3 more)

### Community 18 - "App Entry Points"
Cohesion: 0.24
Nodes (4): body_uses_client(), ClientVisitor, expand(), is_live_attr()

### Community 19 - "SSG Page Handlers"
Cohesion: 0.22
Nodes (8): code:rust (pub use hub::{), code:bash (cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-ru), code:bash (git add pilcrow/crates/runtime/src/fsr/mod.rs), code:toml ([fsr]), File Map, FSR SSE Best Practices Implementation Plan, Pilcrow.toml reference (for app developers), Task 8: Update `mod.rs` exports

### Community 20 - "Blog/Posts Page"
Cohesion: 0.42
Nodes (8): escape_html(), escapes_html_in_value(), find_live_field_content_range(), inject_live_slots(), missing_slot_leaves_content_unchanged(), null_value_clears_slot_content(), replaces_multiple_slots_in_one_pass(), replaces_single_slot()

### Community 22 - "PilcrowProps Derive Macro"
Cohesion: 0.25
Nodes (6): Artifact Layout, Baked Pages Sandbox Model, Core Model, Mutation Lifecycle, Request Lifecycle, Safety Rules

### Community 23 - "App Lifecycle Hooks A"
Cohesion: 0.25
Nodes (7): code:rust (#[tokio::test]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:rust (/// Dev-only handler for `GET /__pilcrow/fsr/inspect`.), code:rust (// Change every occurrence of:), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:bash (git add pilcrow/crates/runtime/src/fsr/inspect.rs), Task 7: Show live connection count in the dev inspect page

### Community 24 - "App Lifecycle Hooks B"
Cohesion: 0.29
Nodes (7): code:rust (#[cfg(test)]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-cor), code:rust (/// Maximum concurrent SSE connections before returning 503.), code:rust (max_sse_connections: 1000,), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-cor), code:bash (git add pilcrow/crates/core/src/config/config.rs), Task 1: Add config fields to `FsrConfig`

### Community 25 - "Counter Page Handler"
Cohesion: 0.33
Nodes (3): Live, PriorityBadge, TicketStatus

### Community 26 - "Timestamp Page Handler"
Cohesion: 0.33
Nodes (6): code:rust (#[cfg(test)]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:rust (use axum::{), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:bash (git add pilcrow/crates/runtime/src/fsr/hub.rs), Task 3: New types in `hub.rs` — `FsrHubConfig`, `FsrConnectionCounter`, `ConnectionGuard`, `GuardedStream`

### Community 27 - "SSG Page Handler"
Cohesion: 0.33
Nodes (6): code:rust (#[cfg(test)]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:rust (/// Fetch all slot rows for a route for use by the snapshot ), code:bash (cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-ru), code:bash (git add pilcrow/crates/runtime/src/fsr/store.rs), Task 4: Add `fetch_slots_for_snapshot` to `FsrStore`

### Community 28 - "dep! Macro"
Cohesion: 0.33
Nodes (6): code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-rou), code:bash (cargo test --manifest-path pilcrow/tools/mcp/pilcrow-mcp/Car), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1), code:bash (git add -p), Task 11: Full test suite verification

### Community 29 - "invalidate! Macro"
Cohesion: 0.33
Nodes (6): code:rust (use axum::Router;), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:rust (#[derive(Debug, Deserialize)]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:bash (git add pilcrow/crates/runtime/src/fsr/hub.rs), Task 6: Update `fsr_hub_handler` with limit, TTL, lag resync, configurable keepalive

### Community 30 - "Static Page Handler"
Cohesion: 0.33
Nodes (6): code:rust (#[tokio::test]), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:rust (/// Handler for `GET /__pilcrow/fsr/snapshot?route=...&slots), code:bash (cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-run), code:bash (git add pilcrow/crates/runtime/src/fsr/hub.rs), Task 5: Add `fsr_snapshot_handler` to `hub.rs`

### Community 32 - "App Shell & Navigation"
Cohesion: 0.33
Nodes (6): Deferred Fields Demo Page Template, Demos Index Page Template, ISR Demo Page Template, ISR+SSG Demo Page Template, Demos Root Layout Template, Demo Nav UI Component

### Community 33 - "Error & Fallback Pages"
Cohesion: 0.6
Nodes (4): load(), now_utc(), Props, tick_tx()

### Community 34 - "Sandbox Main"
Cohesion: 0.4
Nodes (5): code:html (<!-- Static field — baked directly, no slot -->), code:block13 (ticket_list__42__status), code:html (<span s-live="ticket_list__42__status">Open</span>), code:json ({), Phase 5 — HTML baking and `s-live` shell slots

### Community 35 - "SolidJS Store"
Cohesion: 0.6
Nodes (5): code:rust (/// A field whose value is tracked, cached, and live-patched), code:rust (/// dep!(tickets, id, params.id)), code:rust (#[pilcrow::promote_after(50)]), Phase 2 — `LiveProp<T>` type and `DependencyKey`, Phase 2 — `LiveProps<T>` type and `DependencyKey`

### Community 36 - "Path Param Matching"
Cohesion: 0.4
Nodes (5): code:rust (#[cfg(feature = "live-props")]), code:bash (cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-ru), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1), code:bash (git add pilcrow/crates/runtime/src/start.rs), Task 9: Wire everything in `start.rs`

### Community 37 - "App Hooks"
Cohesion: 0.4
Nodes (3): code:bash (git add pilcrow/crates/runtime/src/fsr/watcher.rs), code:bash (cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-ru), Task 2: Expose `execute_with_params` in watcher.rs

### Community 38 - "App Entry & Macro"
Cohesion: 0.4
Nodes (5): code:rust (const FSR_PATCH_SCRIPT: &str = "(function(){window.__fsr_rou), code:bash (cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-ro), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1), code:bash (git add pilcrow/crates/routekit/src/templating/codegen/app_m), Task 10: Update `FSR_PATCH_SCRIPT` in `app_module.rs`

### Community 39 - "Counter Data Layer"
Cohesion: 0.4
Nodes (5): createSilcrowStore — SolidJS store bound to Silcrow atom scope, sandbox solid/Counter.tsx, sandbox solid/silcrow-solid.ts, Silcrow.snapshot API, Silcrow.subscribe API

### Community 40 - "Live Props Module Root"
Cohesion: 0.7
Nodes (4): expand(), extract_live_fields(), find_u32_attr(), is_live_props_type()

### Community 43 - "Greeting React Island"
Cohesion: 0.5
Nodes (4): code:block21 (App loads), code:json ({ "ticket_status": "In Progress" }), code:json ({ "ticket_list__42__status": "Closed" }), Phase 8 — SSE push via Silcrow.js

### Community 44 - "User Card Handler"
Cohesion: 0.5
Nodes (4): code:rust (#[derive(Debug, Clone, PartialEq, Eq, Default)]), code:rust (pub const FSR_JSON: bool = true;   // in page.rs — opt in to), code:block26 (FSR_JSON = true  +  STREAMING = true   → build error (incomp), Phase 9 — `page_options.rs` integration

### Community 45 - "Sandbox Entry Point"
Cohesion: 0.5
Nodes (4): code:rust (use pilcrow::live::*;), code:rust (pub struct Props {), code:block9 (pages/), Phase 4 — `live.rs` file convention

### Community 46 - "Card UI Component"
Cohesion: 0.5
Nodes (4): code:rust (// Targeted — by dependency key), code:sql (UPDATE pilcrow_fsr), code:sql (UPDATE pilcrow_fsr), Phase 6 — Invalidation

### Community 55 - "Community 55"
Cohesion: 0.67
Nodes (3): code:rust (pub use pilcrow_runtime::fsr::{), code:rust (use pilcrow::live::*;), Phase 11 — re-exports and developer surface

### Community 56 - "Community 56"
Cohesion: 0.67
Nodes (3): code:sql (UPDATE pilcrow_fsr), code:sql (UPDATE pilcrow_fsr), Phase 10 — `pilcrow_fsr` DB hit count and promotion

### Community 57 - "Community 57"
Cohesion: 0.67
Nodes (3): code:toml ([fsr]), code:block20 (LOOP every poll_interval_ms:), Phase 7 — Watcher process

### Community 58 - "Community 58"
Cohesion: 1.0
Nodes (3): pilcrow-demos build.rs, routekit::compile_current_crate_sources, sandbox build.rs

### Community 59 - "Community 59"
Cohesion: 0.67
Nodes (3): Sandbox Root Layout, Nav UI Component, s-boost Silcrow directive for SPA navigation

### Community 60 - "Community 60"
Cohesion: 0.67
Nodes (3): Error Page, Loading Skeleton Page, 404 Not Found Page

## Ambiguous Edges - Review These
- `SSG PRERENDER constant pattern` → `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.html · relation: conceptually_related_to

## Knowledge Gaps
- **267 isolated node(s):** `TicketUpdate`, `Props`, `Props`, `Ticket`, `Props` (+262 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **23 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `SSG PRERENDER constant pattern` and `Route groups with parentheses strip URL prefix but apply nested layout`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `Props` connect `Baked Pages Request Handling` to `Baked Pages Store Engine`, `Live Props Core & HTML Injection`?**
  _High betweenness centrality (0.038) - this node is a cross-community bridge._
- **Why does `get()` connect `Baked Pages Request Handling` to `Baked Pages Store Engine`?**
  _High betweenness centrality (0.011) - this node is a cross-community bridge._
- **What connects `TicketUpdate`, `Props`, `Props` to the rest of the system?**
  _267 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Baked Pages Store Engine` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._
- **Should `Baked Pages Request Handling` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Config & Cache Settings` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._