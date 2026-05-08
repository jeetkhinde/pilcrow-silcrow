# Graph Report - .  (2026-05-08)

## Corpus Check
- Corpus is ~13,760 words - fits in a single context window. You may not need a graph.

## Summary
- 354 nodes · 623 edges · 44 communities (24 shown, 20 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 43 edges (avg confidence: 0.83)
- Token cost: 3,200 input · 1,800 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Baked Pages Core Logic|Baked Pages Core Logic]]
- [[_COMMUNITY_React Counter + Cart UI|React Counter + Cart UI]]
- [[_COMMUNITY_BakedPageStore Render Pipeline|BakedPageStore Render Pipeline]]
- [[_COMMUNITY_BakedPage Data Structures|BakedPage Data Structures]]
- [[_COMMUNITY_Pilcrow-Demos Data Layer|Pilcrow-Demos Data Layer]]
- [[_COMMUNITY_Rendering Mode Patterns|Rendering Mode Patterns]]
- [[_COMMUNITY_Products API + Rendering|Products API + Rendering]]
- [[_COMMUNITY_React Islands Live Data|React Islands Live Data]]
- [[_COMMUNITY_Demo HTML Templates|Demo HTML Templates]]
- [[_COMMUNITY_Product Grid Widget|Product Grid Widget]]
- [[_COMMUNITY_SolidJS + Silcrow Integration|SolidJS + Silcrow Integration]]
- [[_COMMUNITY_Baked Pages Entry Points|Baked Pages Entry Points]]
- [[_COMMUNITY_React + Solid Island Tags|React + Solid Island Tags]]
- [[_COMMUNITY_Home Page Handlers|Home Page Handlers]]
- [[_COMMUNITY_Live SSE Page|Live SSE Page]]
- [[_COMMUNITY_ServeState Response Headers|ServeState Response Headers]]
- [[_COMMUNITY_Agent Instruction Docs|Agent Instruction Docs]]
- [[_COMMUNITY_App Entry Points|App Entry Points]]
- [[_COMMUNITY_Solid Counter Component|Solid Counter Component]]
- [[_COMMUNITY_About Page|About Page]]
- [[_COMMUNITY_Greeting React Island|Greeting React Island]]
- [[_COMMUNITY_User Card Widget|User Card Widget]]
- [[_COMMUNITY_Routekit Codegen Build|Routekit Codegen Build]]
- [[_COMMUNITY_Sandbox Layout + Nav|Sandbox Layout + Nav]]
- [[_COMMUNITY_Sandbox Error Pages|Sandbox Error Pages]]
- [[_COMMUNITY_Pilcrow App Macro|Pilcrow App Macro]]
- [[_COMMUNITY_Pilcrow-Demos Data Module|Pilcrow-Demos Data Module]]
- [[_COMMUNITY_Build Hooks|Build Hooks]]
- [[_COMMUNITY_Demo Index Page|Demo Index Page]]
- [[_COMMUNITY_Integer Param Handler|Integer Param Handler]]
- [[_COMMUNITY_About Page Handler|About Page Handler]]
- [[_COMMUNITY_Greeting Component|Greeting Component]]
- [[_COMMUNITY_User Card Handler|User Card Handler]]
- [[_COMMUNITY_Card UI Component|Card UI Component]]
- [[_COMMUNITY_About Page Template|About Page Template]]
- [[_COMMUNITY_Live Page Template|Live Page Template]]
- [[_COMMUNITY_User Card Template|User Card Template]]

## God Nodes (most connected - your core abstractions)
1. `BakedPageStore` - 25 edges
2. `html_path()` - 16 edges
3. `rebake_page()` - 15 edges
4. `text_slot()` - 12 edges
5. `declaration_layer_declares_lazy_text_slot_dep_and_recompute()` - 11 edges
6. `dependency_patching_updates_build_time_baked_page()` - 11 edges
7. `never_bake_refuses_prebake_and_baked_serving()` - 10 edges
8. `BakedRouteDeclaration` - 9 edges
9. `serve_baked_page()` - 9 edges
10. `ticket_patch_registry()` - 9 edges

## Surprising Connections (you probably didn't know these)
- `SSR Streaming: shell-first with deferred patch via window.__ps` --semantically_similar_to--> `SSG PRERENDER constant pattern`  [INFERRED] [semantically similar]
  pilcrow-demos/pages/streaming/index.html → pilcrow-demos/pages/ssg/index.rs
- `SSG PRERENDER constant pattern` --conceptually_related_to--> `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.rs → sandbox/pages/(admin)/_layout.html
- `pilcrow-demos build.rs` --semantically_similar_to--> `sandbox build.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/build.rs → sandbox/build.rs
- `pilcrow-demos hooks.rs` --semantically_similar_to--> `sandbox hooks.rs`  [INFERRED] [semantically similar]
  pilcrow-demos/hooks.rs → sandbox/hooks.rs
- `GEMINI.md — Gemini Agent Instructions` --semantically_similar_to--> `CLAUDE.md — Claude Code Project Instructions`  [INFERRED] [semantically similar]
  GEMINI.md → CLAUDE.md

## Hyperedges (group relationships)
- **pilcrow-demos pages demonstrate all Pilcrow rendering modes (SSR, SSG, ISR, ISR+SSG, deferred, streaming)** — pilcrow_demos_pages_ssr, pilcrow_demos_pages_ssg, pilcrow_demos_pages_isr, pilcrow_demos_pages_isr_ssg, pilcrow_demos_pages_deferred, pilcrow_demos_pages_streaming [INFERRED 0.95]
- **Both apps trigger routekit codegen at build time via build.rs** — pilcrow_demos_build, sandbox_build, routekit_compile_current_crate [EXTRACTED 1.00]
- **SolidJS counter component integrates with Silcrow atom layer via createSilcrowStore** — sandbox_solid_counter, sandbox_solid_silcrow_solid, silcrow_subscribe_api [INFERRED 0.85]
- **Baked Page POC: store + patch registry + slot patching form atomic cache-update pattern** — baked_page_store, baked_patch_registry, slot_patching_pattern [INFERRED 0.90]
- **Silcrow-React island pattern: silcrow-react.ts + Counter.tsx + FragmentGrid.tsx demonstrate React islands over Silcrow transport** — silcrow_react_ts, counter_tsx, fragmentgrid_tsx [EXTRACTED 0.95]
- **Demos rendering model pages: ISR, ISR+SSG, Deferred share layout and nav** — demos_layout_html, demos_nav_html, demos_isr_html, demos_isr_ssg_html, demos_deferred_html [INFERRED 0.90]
- **SSR, SSG, Streaming demo pages collectively demonstrate Pilcrow rendering modes** — demos_ssr_page, demos_ssg_page, demos_streaming_page [INFERRED 0.95]
- **Admin route group: dashboard, stats, and layout form a nested route group with shared URL-stripped layout** — sandbox_admin_layout, sandbox_admin_dashboard, sandbox_admin_stats [EXTRACTED 1.00]
- **Special route pages (_layout, _not_found, _loading, _error) form the sandbox's global page shell and fallback system** — sandbox_pages_layout, sandbox_pages_not_found, sandbox_pages_loading, sandbox_pages_error [INFERRED 0.95]

## Communities (44 total, 20 thin omitted)

### Community 0 - "Baked Pages Core Logic"
Cohesion: 0.08
Nodes (51): add_reverse_index_entry(), atomic_temp_file_is_not_served(), baked_root(), BakedSlotKind, BakeEligibility, dependency_key(), DependencyKey, duplicate_marker_refuses_patch() (+43 more)

### Community 1 - "React Counter + Cart UI"
Cohesion: 0.05
Nodes (37): CartAtom, CartBadge(), cartFormSchema, CartFormValues, CreateState, DirectAddToCartForm(), NotifState, NotifyMeForm() (+29 more)

### Community 2 - "BakedPageStore Render Pipeline"
Cohesion: 0.16
Nodes (15): BakedPatchRegistry, BakedRouteDeclaration, BakedSlot, build_time_declaration_prebakes_before_request(), build_time_prebaked_first_get_is_hit_and_skips_ssr_load(), declaration_layer_declares_lazy_text_slot_dep_and_recompute(), declaration_layer_supports_policy_and_trusted_html_declarations(), dependency_patching_updates_build_time_baked_page() (+7 more)

### Community 3 - "BakedPage Data Structures"
Cohesion: 0.12
Nodes (9): add_index_slot(), BakedPage, BakedPageStore, metadata_path(), StaleState, storage_name(), temp_path_for(), unix_timestamp() (+1 more)

### Community 4 - "Pilcrow-Demos Data Layer"
Cohesion: 0.08
Nodes (20): increment(), load(), now_utc(), Props, render_posts(), load(), now_utc(), Props (+12 more)

### Community 5 - "Rendering Mode Patterns"
Cohesion: 0.12
Nodes (22): AsyncValue/AsyncHtml deferred field pattern, SSG Demo Page, SSR Demo Page, Streaming SSR Demo Page, ISR REVALIDATE constant pattern, data::counter::increment (atomic render counter), pilcrow-demos pages/deferred/index.rs, pilcrow-demos pages/isr/index.rs (+14 more)

### Community 6 - "Products API + Rendering"
Cohesion: 0.1
Nodes (15): ApiResponse, get(), Params, Product, render_cards(), router(), ApiResponse, Category (+7 more)

### Community 7 - "React Islands Live Data"
Cohesion: 0.29
Nodes (7): Counter React Component (all hook demos), FragmentGrid React Component, Live Page Handler (SSE counter), LiveProp Pattern — tokio watch channel for live SSE props, Product Grid Widget Handler, Silcrow-React Bridge — useSyncExternalStore + window.Silcrow, silcrow-react TypeScript bindings

### Community 8 - "Demo HTML Templates"
Cohesion: 0.33
Nodes (6): Deferred Fields Demo Page Template, Demos Index Page Template, ISR Demo Page Template, ISR+SSG Demo Page Template, Demos Root Layout Template, Demo Nav UI Component

### Community 10 - "SolidJS + Silcrow Integration"
Cohesion: 0.4
Nodes (5): createSilcrowStore — SolidJS store bound to Silcrow atom scope, sandbox solid/Counter.tsx, sandbox solid/silcrow-solid.ts, Silcrow.snapshot API, Silcrow.subscribe API

### Community 11 - "Baked Pages Entry Points"
Cohesion: 0.6
Nodes (5): BakedPageStore — filesystem abstraction for baked HTML, Baked Pages POC (slot patching, atomic FS), BakedPatchRegistry — dependency-keyed slot recompute registry, Sandbox App Entry Point, Slot Patching Pattern — HTML comment markers + atomic writes

### Community 12 - "React + Solid Island Tags"
Cohesion: 0.5
Nodes (5): <react> custom element for mounting React islands with strategy attribute, React Islands Example README, sandbox pages/index.rs, Product Grid Widget, <solid> custom element for mounting Solid islands

### Community 16 - "Live SSE Page"
Cohesion: 0.67
Nodes (3): counter_tx(), load(), Props

### Community 18 - "Agent Instruction Docs"
Cohesion: 0.5
Nodes (4): AGENTS.md — Agent Instructions, CLAUDE.md — Claude Code Project Instructions, Pilcrow+Silcrow Integration Context Doc, GEMINI.md — Gemini Agent Instructions

### Community 24 - "Routekit Codegen Build"
Cohesion: 1.0
Nodes (3): pilcrow-demos build.rs, routekit::compile_current_crate_sources, sandbox build.rs

### Community 25 - "Sandbox Layout + Nav"
Cohesion: 0.67
Nodes (3): Sandbox Root Layout, Nav UI Component, s-boost Silcrow directive for SPA navigation

### Community 26 - "Sandbox Error Pages"
Cohesion: 0.67
Nodes (3): Error Page, Loading Skeleton Page, 404 Not Found Page

## Ambiguous Edges - Review These
- `SSG PRERENDER constant pattern` → `Route groups with parentheses strip URL prefix but apply nested layout`  [AMBIGUOUS]
  pilcrow-demos/pages/ssg/index.html · relation: conceptually_related_to

## Knowledge Gaps
- **84 isolated node(s):** `Props`, `Props`, `Props`, `Props`, `Props` (+79 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **20 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `SSG PRERENDER constant pattern` and `Route groups with parentheses strip URL prefix but apply nested layout`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `router()` connect `Products API + Rendering` to `Baked Pages Core Logic`, `BakedPageStore Render Pipeline`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Why does `BakedPageStore` connect `BakedPage Data Structures` to `Baked Pages Core Logic`, `BakedPageStore Render Pipeline`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **What connects `Props`, `Props`, `Props` to the rest of the system?**
  _84 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Baked Pages Core Logic` be split into smaller, more focused modules?**
  _Cohesion score 0.08 - nodes in this community are weakly interconnected._
- **Should `React Counter + Cart UI` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._
- **Should `BakedPage Data Structures` be split into smaller, more focused modules?**
  _Cohesion score 0.12 - nodes in this community are weakly interconnected._