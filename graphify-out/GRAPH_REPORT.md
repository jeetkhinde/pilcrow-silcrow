# Graph Report - pilcrow-silcrow  (2026-05-12)

## Corpus Check
- 31 files · ~17,237 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 719 nodes · 1432 edges · 61 communities (39 shown, 22 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 51 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c14929b8`
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
- [[_COMMUNITY_invalidate! Macro|invalidate! Macro]]
- [[_COMMUNITY_Codegen Build Scripts|Codegen Build Scripts]]
- [[_COMMUNITY_App Shell & Navigation|App Shell & Navigation]]
- [[_COMMUNITY_SolidJS Store|SolidJS Store]]
- [[_COMMUNITY_Path Param Matching|Path Param Matching]]
- [[_COMMUNITY_App Hooks|App Hooks]]
- [[_COMMUNITY_App Entry & Macro|App Entry & Macro]]
- [[_COMMUNITY_Counter Data Layer|Counter Data Layer]]
- [[_COMMUNITY_Live Props Module Root|Live Props Module Root]]
- [[_COMMUNITY_Sandbox Params|Sandbox Params]]
- [[_COMMUNITY_Sandbox Entry Point|Sandbox Entry Point]]
- [[_COMMUNITY_Card UI Component|Card UI Component]]
- [[_COMMUNITY_About Page|About Page]]
- [[_COMMUNITY_User Card Widget|User Card Widget]]
- [[_COMMUNITY_Demos Index Page|Demos Index Page]]
- [[_COMMUNITY_Mod Root|Mod Root]]
- [[_COMMUNITY_Handler Live Attribute|Handler Live Attribute]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 55|Community 55]]
- [[_COMMUNITY_Community 56|Community 56]]
- [[_COMMUNITY_Community 57|Community 57]]
- [[_COMMUNITY_Community 58|Community 58]]
- [[_COMMUNITY_Community 60|Community 60]]

## God Nodes (most connected - your core abstractions)
1. `BakedPageStore` - 47 edges
2. `html_path()` - 22 edges
3. `rebake_page()` - 17 edges
4. `text_slot()` - 17 edges
5. `build_time_fragment_composed_prebakes_body_and_serves_composed_hit()` - 17 edges
6. `CLAUDE.md — pilcrow-silcrow workspace` - 17 edges
7. `FSR — Field-Selective Rendering: Implementation Plan` - 16 edges
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

## Communities (61 total, 22 thin omitted)

### Community 0 - "Baked Pages Store Engine"
Cohesion: 0.06
Nodes (72): add_index_slot(), add_reverse_index_entry(), atomic_temp_file_is_not_served(), baked_html_response(), baked_root(), BakedArtifactMode, BakedFragment, BakedLayout (+64 more)

### Community 1 - "Baked Pages Request Handling"
Cohesion: 0.06
Nodes (47): LiveConfig, PilcrowConfig, escape_html(), escapes_html_in_value(), find_live_field_content_range(), inject_live_slots(), missing_slot_leaves_content_unchanged(), null_value_clears_slot_content() (+39 more)

### Community 2 - "Config & Cache Settings"
Cohesion: 0.1
Nodes (45): CartAtom, CartBadge(), CartClearButton(), cartFormSchema, CartFormValues, Counter(), CreateState, DirectAddToCartForm() (+37 more)

### Community 3 - "Live Props Core & HTML Injection"
Cohesion: 0.06
Nodes (33): BackendConfig, CacheConfig, CacheProvider, ClientRuntimeConfig, default_backend_host(), default_backend_port(), default_backend_url(), default_image_cache_dir() (+25 more)

### Community 4 - "Sandbox E-commerce UI"
Cohesion: 0.11
Nodes (19): BakedPatchRegistry, BakedRouteDeclaration, BakedSlot, build_time_declaration_prebakes_before_request(), build_time_fragment_composed_prebakes_body_and_serves_composed_hit(), build_time_prebaked_first_get_is_hit_and_skips_ssr_load(), declaration_can_select_fragment_composed_layout(), declaration_defaults_to_full_page_and_can_make_mode_explicit() (+11 more)

### Community 5 - "Agent Context Docs"
Cohesion: 0.04
Nodes (47): Background, code:block1 (promote_after = 0 or absent  → SSG  (bake at startup, surgic), code:rust (use pilcrow::live::*;), code:rust (pub struct Props {), code:html (<!-- Static field — baked directly, no slot -->), code:block13 (ticket_list__42__status), code:html (<span s-live="ticket_list__42__status">Open</span>), code:json ({) (+39 more)

### Community 6 - "Rendering Mode Demos"
Cohesion: 0.05
Nodes (39): code:bash (git mv sandbox demo), code:bash (mkdir -p demo/pages/demo/fsr), code:rust (use std::sync::OnceLock;), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1 | grep -E "), code:html (---), code:bash (cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5), code:bash (cargo run --manifest-path demo/Cargo.toml &), code:bash (git add demo/pages/demo/fsr/) (+31 more)

### Community 7 - "Live & React Island Demos"
Cohesion: 0.08
Nodes (35): Agent Instructions, CLAUDE.md — pilcrow-silcrow workspace, First rule, graphify, Graphify first, OpenCode / Agent Instructions, Pilcrow / Silcrow relationship, Read Claude.md at workspace root. (+27 more)

### Community 8 - "Live Props DB Store"
Cohesion: 0.13
Nodes (22): AsyncValue/AsyncHtml deferred field pattern, SSG Demo Page, SSR Demo Page, Streaming SSR Demo Page, ISR REVALIDATE constant pattern, data::counter::increment (atomic render counter), pilcrow-demos pages/deferred/index.rs, pilcrow-demos pages/isr/index.rs (+14 more)

### Community 9 - "ISR Page Handlers"
Cohesion: 0.14
Nodes (16): Counter React Component (all hook demos), FragmentGrid React Component, Live Page Handler (SSE counter), LiveProp Pattern — tokio watch channel for live SSE props, Product Grid Widget Handler, <react> custom element for mounting React islands with strategy attribute, React Islands Example README, code:toml ([routing]) (+8 more)

### Community 10 - "ISR/SSG Pages"
Cohesion: 0.14
Nodes (13): Added, App Identity, Deleted (from pilcrow-demos, not migrated), Demo App Consolidation Design, `/demo/fsr` Page Design, `/demo/react-island` Page Design, File Operations Summary, Kept (from sandbox, unchanged) (+5 more)

### Community 11 - "Handler Macro Logic"
Cohesion: 0.23
Nodes (8): bust(), load(), now_utc(), Props, load(), Props, load(), Props

### Community 12 - "API Route Handlers"
Cohesion: 0.18
Nodes (10): current(), increment(), Counter(), Props, load(), now_utc(), Props, load() (+2 more)

### Community 13 - "Proc-macro Entry Points"
Cohesion: 0.24
Nodes (4): body_uses_client(), ClientVisitor, expand(), is_live_attr()

### Community 14 - "Counter Feature"
Cohesion: 0.44
Nodes (7): ApiResponse, escape(), get(), Params, Product, render_cards(), router()

### Community 15 - "Baked Pages Architecture Docs"
Cohesion: 0.39
Nodes (7): ApiResponse, Category, CategoryInfo, load(), Product, ProductView, Props

### Community 17 - "Page Templates"
Cohesion: 0.57
Nodes (6): init(), load(), Props, resolve_1(), resolve_2(), resolve_3()

### Community 18 - "App Entry Points"
Cohesion: 0.25
Nodes (6): Artifact Layout, Baked Pages Sandbox Model, Core Model, Mutation Lifecycle, Request Lifecycle, Safety Rules

### Community 19 - "SSG Page Handlers"
Cohesion: 0.53
Nodes (4): load(), Props, refresh(), Row

### Community 21 - "SolidJS Integration"
Cohesion: 0.33
Nodes (6): Deferred Fields Demo Page Template, Demos Index Page Template, ISR Demo Page Template, ISR+SSG Demo Page Template, Demos Root Layout Template, Demo Nav UI Component

### Community 22 - "PilcrowProps Derive Macro"
Cohesion: 0.6
Nodes (4): load(), now_utc(), Props, tick_tx()

### Community 23 - "App Lifecycle Hooks A"
Cohesion: 0.5
Nodes (3): load(), now_utc(), Props

### Community 24 - "App Lifecycle Hooks B"
Cohesion: 0.6
Nodes (4): load(), now_utc(), Props, render_posts()

### Community 25 - "Counter Page Handler"
Cohesion: 0.7
Nodes (3): counter_tx(), load(), Props

### Community 26 - "Timestamp Page Handler"
Cohesion: 0.4
Nodes (5): createSilcrowStore — SolidJS store bound to Silcrow atom scope, sandbox solid/Counter.tsx, sandbox solid/silcrow-solid.ts, Silcrow.snapshot API, Silcrow.subscribe API

### Community 27 - "SSG Page Handler"
Cohesion: 0.7
Nodes (4): expand(), extract_live_fields(), find_u32_attr(), is_live_props_type()

### Community 31 - "Codegen Build Scripts"
Cohesion: 0.67
Nodes (3): chrono_now(), load(), Props

### Community 39 - "Counter Data Layer"
Cohesion: 1.0
Nodes (3): pilcrow-demos build.rs, routekit::compile_current_crate_sources, sandbox build.rs

### Community 40 - "Live Props Module Root"
Cohesion: 0.67
Nodes (3): Sandbox Root Layout, Nav UI Component, s-boost Silcrow directive for SPA navigation

### Community 41 - "Sandbox Params"
Cohesion: 0.67
Nodes (3): Error Page, Loading Skeleton Page, 404 Not Found Page

## Ambiguous Edges - Review These
- `SSG PRERENDER constant pattern` → `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.html · relation: conceptually_related_to

## Knowledge Gaps
- **143 isolated node(s):** `Props`, `Props`, `Architecture`, `Integration`, `Build Rule` (+138 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `SSG PRERENDER constant pattern` and `Route groups with parentheses strip URL prefix but apply nested layout`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `Props` connect `Handler Macro Logic` to `Baked Pages Store Engine`, `SSG Page Handlers`, `Config & Cache Settings`, `App Shell & Navigation`?**
  _High betweenness centrality (0.063) - this node is a cross-community bridge._
- **Why does `BakedPageStore` connect `Baked Pages Store Engine` to `Sandbox E-commerce UI`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **Why does `get()` connect `Counter Feature` to `Baked Pages Store Engine`, `Handler Macro Logic`, `Baked Pages Architecture Docs`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._
- **What connects `Props`, `Props`, `Architecture` to the rest of the system?**
  _143 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Baked Pages Store Engine` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Baked Pages Request Handling` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._