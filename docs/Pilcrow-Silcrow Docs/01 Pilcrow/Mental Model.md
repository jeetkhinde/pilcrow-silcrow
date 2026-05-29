# Pilcrow Mental Model

Pilcrow is a Rust full-stack web framework built around file conventions and build-time code generation.

## What Pilcrow Owns

- Route discovery from `pages/`, `api/`, configured fragment directories, and support folders.
- HTML template transpilation for Askama.
- Code-behind validation for `Props`, `load()`, actions, and route options.
- Generated Axum router wiring.
- Runtime middleware, request context, response helpers, SSE, assets, security defaults, and platform adapters.
- Embedding the Silcrow runtime asset.

## What Silcrow Owns

Silcrow runs in the browser. It handles:

- DOM patching.
- Enhanced form and link navigation.
- SSE-driven live patches.
- Atoms and client-side subscriptions.
- Optimistic UI confirmation and rollback.

Pilcrow writes `data-pilcrow-*` attributes and runtime routes. Silcrow reads those attributes and applies browser-side behavior.

## Build-Time Flow

1. The app's `build.rs` calls routekit.
2. Routekit discovers pages, layouts, fragments, APIs, params, i18n, env config, and hooks.
3. Routekit emits generated Rust into `OUT_DIR`.
4. `pilcrow_app!()` includes generated modules.
5. The app runs a generated Axum router.

## Source of Truth

Use this order when documenting behavior:

1. `pilcrow/registry.toml`
2. MCP `list_features` / `explain_feature`
3. Dedicated docs such as `.claude/rendering-models.md`
4. Source refs and tests when the docs disagree
