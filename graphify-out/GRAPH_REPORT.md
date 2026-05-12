# Graph Report - .  (2026-05-11)

## Corpus Check
- 75 files · ~100,000 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 567 nodes · 1123 edges · 53 communities (35 shown, 18 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 51 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

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
- [[_COMMUNITY_Counter Page Handler|Counter Page Handler]]
- [[_COMMUNITY_Timestamp Page Handler|Timestamp Page Handler]]
- [[_COMMUNITY_SSG Page Handler|SSG Page Handler]]
- [[_COMMUNITY_invalidate! Macro|invalidate! Macro]]
- [[_COMMUNITY_Static Page Handler|Static Page Handler]]
- [[_COMMUNITY_Codegen Build Scripts|Codegen Build Scripts]]
- [[_COMMUNITY_App Shell & Navigation|App Shell & Navigation]]
- [[_COMMUNITY_Error & Fallback Pages|Error & Fallback Pages]]
- [[_COMMUNITY_App Hooks|App Hooks]]
- [[_COMMUNITY_App Entry & Macro|App Entry & Macro]]
- [[_COMMUNITY_Counter Data Layer|Counter Data Layer]]
- [[_COMMUNITY_Sandbox Params|Sandbox Params]]
- [[_COMMUNITY_About Page Handler|About Page Handler]]
- [[_COMMUNITY_Greeting React Island|Greeting React Island]]
- [[_COMMUNITY_User Card Handler|User Card Handler]]
- [[_COMMUNITY_Sandbox Entry Point|Sandbox Entry Point]]
- [[_COMMUNITY_Card UI Component|Card UI Component]]
- [[_COMMUNITY_About Page|About Page]]
- [[_COMMUNITY_Live Props Demo Page|Live Props Demo Page]]
- [[_COMMUNITY_User Card Widget|User Card Widget]]
- [[_COMMUNITY_Demos Index Page|Demos Index Page]]
- [[_COMMUNITY_Handler Live Attribute|Handler Live Attribute]]

## God Nodes (most connected - your core abstractions)
1. `BakedPageStore` - 46 edges
2. `html_path()` - 21 edges
3. `CLAUDE.md — pilcrow-silcrow workspace` - 17 edges
4. `rebake_page()` - 16 edges
5. `text_slot()` - 16 edges
6. `build_time_fragment_composed_prebakes_body_and_serves_composed_hit()` - 16 edges
7. `CLAUDE.md — pilcrow-silcrow workspace` - 16 edges
8. `BakedRouteDeclaration` - 14 edges
9. `patch_slot()` - 14 edges
10. `reverse_index()` - 13 edges

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
  pilcrow-demos/pages/isr/index.rs → sandbox/api/products.rs

## Hyperedges (group relationships)
- **Invalidation pipeline: dep! → invalidate! → LivePageStore → LiveBroadcast → SSE clients** — live_props_dep_dep_macro, macros_invalidate_macro_expand, live_props_store_invalidate_dep_key, live_props_broadcast_livebroadcast, live_props_broadcast_invalidationevent [INFERRED 0.95]
- **Handler live write pipeline: #[handler(live)] → LivePropsExtract → LivePageStore::write + increment_hit** — macros_handler_expand, live_props_model_livepropsextract, live_props_store_write_live_fields, live_props_store_increment_hit [INFERRED 0.95]
- **#[derive(PilcrowProps)] → LivePropsExtract impl → LiveFieldData extraction** — macros_lib_derive_pilcrow_props, macros_live_props_derive_expand, live_props_model_livepropsextract, live_props_model_livefielddata [EXTRACTED 1.00]
- **DB schema: pilcrow_cache + pilcrow_routes tables + GIN + stale indexes** — migration_001_pilcrow_cache, migration_001_pilcrow_routes, migration_002_gin_index, migration_002_stale_index [EXTRACTED 1.00]
- **Startup registration: start_with_adapter registers LiveBroadcast + LivePageStore as Axum extensions** — runtime_start_start_with_adapter, runtime_start_live_props_registration, live_props_broadcast_livebroadcast, live_props_store_livepagestore [EXTRACTED 1.00]

## Communities (53 total, 18 thin omitted)

### Community 0 - "Baked Pages Store Engine"
Cohesion: 0.07
Nodes (37): add_reverse_index_entry(), baked_root(), BakedFragment, BakedLayout, BakedPage, BakedPageStore, BakedPatchRegistry, BakedRouteDeclaration (+29 more)

### Community 1 - "Baked Pages Request Handling"
Cohesion: 0.06
Nodes (51): add_index_slot(), atomic_temp_file_is_not_served(), baked_html_response(), BakedArtifactMode, BakedSlot, BakedSlotKind, BakeEligibility, dependency_key() (+43 more)

### Community 2 - "Config & Cache Settings"
Cohesion: 0.06
Nodes (33): BackendConfig, CacheConfig, CacheProvider, ClientRuntimeConfig, default_backend_host(), default_backend_port(), default_backend_url(), default_image_cache_dir() (+25 more)

### Community 3 - "Live Props Core & HTML Injection"
Cohesion: 0.07
Nodes (40): LiveConfig, PilcrowConfig, escape_html(), escapes_html_in_value(), find_live_field_content_range(), inject_live_slots(), missing_slot_leaves_content_unchanged(), null_value_clears_slot_content() (+32 more)

### Community 4 - "Sandbox E-commerce UI"
Cohesion: 0.07
Nodes (41): CartAtom, CartBadge(), CartClearButton(), cartFormSchema, CartFormValues, CreateState, DirectAddToCartForm(), HookFormAddToCartForm() (+33 more)

### Community 5 - "Agent Context Docs"
Cohesion: 0.1
Nodes (28): CLAUDE.md — pilcrow-silcrow workspace, graphify, Read Claude.md at workspace root., Active plans, Attribute namespace table, Build commands (from workspace root), CLAUDE.md — pilcrow-silcrow workspace, code:text (pilcrow-silcrow/) (+20 more)

### Community 6 - "Rendering Mode Demos"
Cohesion: 0.13
Nodes (22): AsyncValue/AsyncHtml deferred field pattern, SSG Demo Page, SSR Demo Page, Streaming SSR Demo Page, ISR REVALIDATE constant pattern, data::counter::increment (atomic render counter), pilcrow-demos pages/deferred/index.rs, pilcrow-demos pages/isr/index.rs (+14 more)

### Community 7 - "Live & React Island Demos"
Cohesion: 0.15
Nodes (16): Counter React Component (all hook demos), FragmentGrid React Component, Live Page Handler (SSE counter), LiveProp Pattern — tokio watch channel for live SSE props, Product Grid Widget Handler, <react> custom element for mounting React islands with strategy attribute, React Islands Example README, code:toml ([routing]) (+8 more)

### Community 8 - "Live Props DB Store"
Cohesion: 0.26
Nodes (7): increment_hit_promotes_at_threshold(), invalidate_dep_key_marks_rows_stale(), LivePageStore, test_pool(), write_and_read_live_fields(), live-props feature gate: broadcast + store registration, start_with_adapter

### Community 9 - "ISR Page Handlers"
Cohesion: 0.22
Nodes (6): bust(), load(), now_utc(), Props, Props, Props

### Community 10 - "ISR/SSG Pages"
Cohesion: 0.24
Nodes (8): load(), Props, ApiResponse, Category, CategoryInfo, Product, ProductView, Props

### Community 11 - "Handler Macro Logic"
Cohesion: 0.24
Nodes (4): body_uses_client(), ClientVisitor, expand(), is_live_attr()

### Community 12 - "API Route Handlers"
Cohesion: 0.28
Nodes (7): ApiResponse, get(), Params, Product, render_cards(), router(), load()

### Community 14 - "Counter Feature"
Cohesion: 0.32
Nodes (6): current(), increment(), Counter(), load(), now_utc(), Props

### Community 15 - "Baked Pages Architecture Docs"
Cohesion: 0.29
Nodes (6): Artifact Layout, Baked Pages Sandbox Model, Core Model, Mutation Lifecycle, Request Lifecycle, Safety Rules

### Community 16 - "Product Grid Page"
Cohesion: 0.33
Nodes (3): Row, Props, Row

### Community 17 - "Page Templates"
Cohesion: 0.33
Nodes (6): Deferred Fields Demo Page Template, Demos Index Page Template, ISR Demo Page Template, ISR+SSG Demo Page Template, Demos Root Layout Template, Demo Nav UI Component

### Community 19 - "SSG Page Handlers"
Cohesion: 0.5
Nodes (3): load(), now_utc(), Props

### Community 20 - "Blog/Posts Page"
Cohesion: 0.6
Nodes (4): load(), now_utc(), Props, render_posts()

### Community 21 - "SolidJS Integration"
Cohesion: 0.4
Nodes (5): createSilcrowStore — SolidJS store bound to Silcrow atom scope, sandbox solid/Counter.tsx, sandbox solid/silcrow-solid.ts, Silcrow.snapshot API, Silcrow.subscribe API

### Community 22 - "PilcrowProps Derive Macro"
Cohesion: 0.7
Nodes (4): expand(), extract_live_fields(), find_u32_attr(), is_live_props_type()

### Community 25 - "Counter Page Handler"
Cohesion: 0.67
Nodes (3): counter_tx(), load(), Props

### Community 26 - "Timestamp Page Handler"
Cohesion: 0.67
Nodes (3): chrono_now(), load(), Props

### Community 27 - "SSG Page Handler"
Cohesion: 0.67
Nodes (3): load(), now_utc(), Props

### Community 31 - "Codegen Build Scripts"
Cohesion: 1.0
Nodes (3): pilcrow-demos build.rs, routekit::compile_current_crate_sources, sandbox build.rs

### Community 32 - "App Shell & Navigation"
Cohesion: 0.67
Nodes (3): Sandbox Root Layout, Nav UI Component, s-boost Silcrow directive for SPA navigation

### Community 33 - "Error & Fallback Pages"
Cohesion: 0.67
Nodes (3): Error Page, Loading Skeleton Page, 404 Not Found Page

## Ambiguous Edges - Review These
- `SSG PRERENDER constant pattern` → `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.html · relation: conceptually_related_to

## Knowledge Gaps
- **93 isolated node(s):** `Props`, `Props`, `Props`, `Props`, `Props` (+88 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **18 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `SSG PRERENDER constant pattern` and `Route groups with parentheses strip URL prefix but apply nested layout`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `Props` connect `ISR Page Handlers` to `Baked Pages Store Engine`, `Product Grid Page`, `ISR/SSG Pages`, `Sandbox E-commerce UI`?**
  _High betweenness centrality (0.090) - this node is a cross-community bridge._
- **Why does `BakedPageStore` connect `Baked Pages Store Engine` to `Baked Pages Request Handling`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **Why does `get()` connect `API Route Handlers` to `ISR Page Handlers`, `Baked Pages Request Handling`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._
- **What connects `Props`, `Props`, `Props` to the rest of the system?**
  _93 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Baked Pages Store Engine` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._
- **Should `Baked Pages Request Handling` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._