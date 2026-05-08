# Graph Report - pilcrow-silcrow  (2026-05-08)

## Corpus Check
- 35 files · ~14,510 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 405 nodes · 712 edges · 44 communities (24 shown, 20 thin omitted)
- Extraction: 94% EXTRACTED · 6% INFERRED · 0% AMBIGUOUS · INFERRED: 42 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `dcde4051`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Ticket Baked Page POC|Ticket Baked Page POC]]
- [[_COMMUNITY_Baked Store API|Baked Store API]]
- [[_COMMUNITY_Cart React Components|Cart React Components]]
- [[_COMMUNITY_Baked Artifact Model|Baked Artifact Model]]
- [[_COMMUNITY_Time Demo Loaders|Time Demo Loaders]]
- [[_COMMUNITY_Product API Routes|Product API Routes]]
- [[_COMMUNITY_Rendering Mode Demos|Rendering Mode Demos]]
- [[_COMMUNITY_React Live Islands|React Live Islands]]
- [[_COMMUNITY_Demo Templates|Demo Templates]]
- [[_COMMUNITY_Product Grid Refresh|Product Grid Refresh]]
- [[_COMMUNITY_Solid Silcrow Store|Solid Silcrow Store]]
- [[_COMMUNITY_Island Mounting Docs|Island Mounting Docs]]
- [[_COMMUNITY_Sandbox Hooks|Sandbox Hooks]]
- [[_COMMUNITY_Index Loaders|Index Loaders]]
- [[_COMMUNITY_Admin Route Group|Admin Route Group]]
- [[_COMMUNITY_Workspace Instructions|Workspace Instructions]]
- [[_COMMUNITY_App Entrypoints|App Entrypoints]]
- [[_COMMUNITY_Counter Component|Counter Component]]
- [[_COMMUNITY_Route Props Loader|Route Props Loader]]
- [[_COMMUNITY_Greeting Component|Greeting Component]]
- [[_COMMUNITY_User Card Loader|User Card Loader]]
- [[_COMMUNITY_Routekit Build Scripts|Routekit Build Scripts]]
- [[_COMMUNITY_Sandbox Navigation Layout|Sandbox Navigation Layout]]
- [[_COMMUNITY_Error Pages|Error Pages]]
- [[_COMMUNITY_Demo App Macro|Demo App Macro]]
- [[_COMMUNITY_Demo Counter Data|Demo Counter Data]]
- [[_COMMUNITY_Hook Modules|Hook Modules]]
- [[_COMMUNITY_Demo Home Page|Demo Home Page]]
- [[_COMMUNITY_Sandbox Integer Params|Sandbox Integer Params]]
- [[_COMMUNITY_About Handler|About Handler]]
- [[_COMMUNITY_Greeting Island|Greeting Island]]
- [[_COMMUNITY_User Card Widget|User Card Widget]]
- [[_COMMUNITY_Sandbox App Entry|Sandbox App Entry]]
- [[_COMMUNITY_Card UI|Card UI]]
- [[_COMMUNITY_About Page|About Page]]
- [[_COMMUNITY_Live Props Demo|Live Props Demo]]
- [[_COMMUNITY_User Card Template|User Card Template]]

## God Nodes (most connected - your core abstractions)
1. `BakedPageStore` - 37 edges
2. `html_path()` - 16 edges
3. `CLAUDE.md — pilcrow-silcrow workspace` - 16 edges
4. `rebake_page()` - 15 edges
5. `text_slot()` - 12 edges
6. `fragment_composed_page_uses_shared_layout_without_rebaking_body()` - 12 edges
7. `declaration_layer_declares_lazy_text_slot_dep_and_recompute()` - 11 edges
8. `dependency_patching_updates_build_time_baked_page()` - 11 edges
9. `never_bake_refuses_prebake_and_baked_serving()` - 10 edges
10. `BakedRouteDeclaration` - 9 edges

## Surprising Connections (you probably didn't know these)
- `SSR Streaming: shell-first with deferred patch via window.__ps` --semantically_similar_to--> `SSG PRERENDER constant pattern`  [INFERRED] [semantically similar]
  pilcrow-demos/pages/streaming/index.html → pilcrow-demos/pages/ssg/index.rs
- `SSG PRERENDER constant pattern` --conceptually_related_to--> `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.rs → sandbox/pages/(admin)/_layout.html
- `pilcrow-demos build.rs` --semantically_similar_to--> `sandbox build.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/build.rs → sandbox/build.rs
- `pilcrow-demos hooks.rs` --semantically_similar_to--> `sandbox hooks.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/hooks.rs → sandbox/hooks.rs
- `SSG Demo Page` --conceptually_related_to--> `SSG PRERENDER constant pattern`  [EXTRACTED]
  pilcrow-demos/pages/ssg/index.html → pilcrow-demos/pages/ssg/index.rs

## Hyperedges (group relationships)
- **Both apps trigger routekit codegen at build time via build.rs** — pilcrow_demos_build, sandbox_build, routekit_compile_current_crate [EXTRACTED 1.00]
- **SolidJS counter component integrates with Silcrow atom layer via createSilcrowStore** — sandbox_solid_counter, sandbox_solid_silcrow_solid, silcrow_subscribe_api [INFERRED 0.85]
- **Silcrow-React island pattern: silcrow-react.ts + Counter.tsx + FragmentGrid.tsx demonstrate React islands over Silcrow transport** — silcrow_react_ts, counter_tsx, fragmentgrid_tsx [EXTRACTED 0.95]
- **Demos rendering model pages: ISR, ISR+SSG, Deferred share layout and nav** — demos_layout_html, demos_nav_html, demos_isr_html, demos_isr_ssg_html, demos_deferred_html [INFERRED 0.90]
- **SSR, SSG, Streaming demo pages collectively demonstrate Pilcrow rendering modes** — demos_ssr_page, demos_ssg_page, demos_streaming_page [INFERRED 0.95]
- **Special route pages (_layout, _not_found, _loading, _error) form the sandbox's global page shell and fallback system** — sandbox_pages_layout, sandbox_pages_not_found, sandbox_pages_loading, sandbox_pages_error [INFERRED 0.95]
- **Admin route group: dashboard, stats, and layout form a nested route group with shared URL-stripped layout** — sandbox_admin_layout, sandbox_admin_dashboard, sandbox_admin_stats [EXTRACTED 1.00]

## Communities (44 total, 20 thin omitted)

### Community 0 - "Ticket Baked Page POC"
Cohesion: 0.06
Nodes (54): add_reverse_index_entry(), atomic_temp_file_is_not_served(), baked_html_response(), baked_root(), BakedArtifactMode, BakedLayout, BakedSlotKind, BakeEligibility (+46 more)

### Community 1 - "Baked Store API"
Cohesion: 0.05
Nodes (37): CartAtom, CartBadge(), cartFormSchema, CartFormValues, CreateState, DirectAddToCartForm(), NotifState, NotifyMeForm() (+29 more)

### Community 2 - "Cart React Components"
Cohesion: 0.09
Nodes (10): add_index_slot(), BakedFragment, BakedPage, BakedPageStore, fragment_composed_page_uses_shared_layout_without_rebaking_body(), metadata_path(), replace_slot_content(), StaleState (+2 more)

### Community 3 - "Baked Artifact Model"
Cohesion: 0.15
Nodes (17): BakedPatchRegistry, BakedRouteDeclaration, BakedSlot, build_time_declaration_prebakes_before_request(), build_time_prebaked_first_get_is_hit_and_skips_ssr_load(), declaration_layer_declares_lazy_text_slot_dep_and_recompute(), declaration_layer_supports_policy_and_trusted_html_declarations(), dependency_patching_updates_build_time_baked_page() (+9 more)

### Community 4 - "Time Demo Loaders"
Cohesion: 0.06
Nodes (28): CLAUDE.md — pilcrow-silcrow workspace, graphify, Read Claude.md at workspace root., Active plans, Attribute namespace table, Build commands (from workspace root), CLAUDE.md — pilcrow-silcrow workspace, code:text (pilcrow-silcrow/) (+20 more)

### Community 5 - "Product API Routes"
Cohesion: 0.08
Nodes (20): increment(), load(), now_utc(), Props, render_posts(), load(), now_utc(), Props (+12 more)

### Community 6 - "Rendering Mode Demos"
Cohesion: 0.1
Nodes (16): ApiResponse, get(), Params, Product, render_cards(), router(), ApiResponse, Category (+8 more)

### Community 7 - "React Live Islands"
Cohesion: 0.12
Nodes (22): AsyncValue/AsyncHtml deferred field pattern, SSG Demo Page, SSR Demo Page, Streaming SSR Demo Page, ISR REVALIDATE constant pattern, data::counter::increment (atomic render counter), pilcrow-demos pages/deferred/index.rs, pilcrow-demos pages/isr/index.rs (+14 more)

### Community 8 - "Demo Templates"
Cohesion: 0.29
Nodes (7): Counter React Component (all hook demos), FragmentGrid React Component, Live Page Handler (SSE counter), LiveProp Pattern — tokio watch channel for live SSE props, Product Grid Widget Handler, Silcrow-React Bridge — useSyncExternalStore + window.Silcrow, silcrow-react TypeScript bindings

### Community 9 - "Product Grid Refresh"
Cohesion: 0.33
Nodes (6): Deferred Fields Demo Page Template, Demos Index Page Template, ISR Demo Page Template, ISR+SSG Demo Page Template, Demos Root Layout Template, Demo Nav UI Component

### Community 11 - "Island Mounting Docs"
Cohesion: 0.4
Nodes (4): code:toml ([routing]), code:html (<!-- src/pages/dashboard/index.html -->), code:tsx (// src/pages/dashboard/react/Counter.tsx), React Islands Example

### Community 12 - "Sandbox Hooks"
Cohesion: 0.4
Nodes (5): createSilcrowStore — SolidJS store bound to Silcrow atom scope, sandbox solid/Counter.tsx, sandbox solid/silcrow-solid.ts, Silcrow.snapshot API, Silcrow.subscribe API

### Community 13 - "Index Loaders"
Cohesion: 0.5
Nodes (5): <react> custom element for mounting React islands with strategy attribute, React Islands Example README, sandbox pages/index.rs, Product Grid Widget, <solid> custom element for mounting Solid islands

### Community 17 - "Workspace Instructions"
Cohesion: 0.67
Nodes (3): counter_tx(), load(), Props

### Community 23 - "Routekit Build Scripts"
Cohesion: 1.0
Nodes (3): pilcrow-demos build.rs, routekit::compile_current_crate_sources, sandbox build.rs

### Community 24 - "Sandbox Navigation Layout"
Cohesion: 0.67
Nodes (3): Sandbox Root Layout, Nav UI Component, s-boost Silcrow directive for SPA navigation

### Community 25 - "Error Pages"
Cohesion: 0.67
Nodes (3): Error Page, Loading Skeleton Page, 404 Not Found Page

## Ambiguous Edges - Review These
- `SSG PRERENDER constant pattern` → `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.html · relation: conceptually_related_to

## Knowledge Gaps
- **107 isolated node(s):** `Props`, `Props`, `Props`, `Props`, `Props` (+102 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **20 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `SSG PRERENDER constant pattern` and `Route groups with parentheses strip URL prefix but apply nested layout`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `BakedPageStore` connect `Cart React Components` to `Ticket Baked Page POC`, `Baked Artifact Model`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
- **Why does `router()` connect `Rendering Mode Demos` to `Ticket Baked Page POC`, `Baked Artifact Model`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **What connects `Props`, `Props`, `Props` to the rest of the system?**
  _107 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Ticket Baked Page POC` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Baked Store API` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._
- **Should `Cart React Components` be split into smaller, more focused modules?**
  _Cohesion score 0.09 - nodes in this community are weakly interconnected._