# AGENTS.md

This file provides guidance to AI coding agents working in the Pilcrow repository.

## MCP is the source of truth (AI-first policy)

Pilcrow is AI-first. Treat `tools/pilcrow-mcp` as the authoritative interface for:
- feature status and canonical usage (`registry.toml`),
- implementation evidence (`source_refs` + test refs),
- docs answers for coding agents.

When shipping or changing a feature, update **all** of:
1. Runtime/routekit/web implementation,
2. `registry.toml` feature contract (spec, canonical_usage, constraints, invalid_examples, refs),
3. MCP knowledge coverage (`tools/pilcrow-mcp/src/docs.rs`) — corpus is 3 fixed `.md` docs; only update if a doc file is renamed or a new prose doc is added.
4. At least one executable example or test reference that MCP can cite.

Goal: agents should not rely on memory or ad-hoc docs; they should be able to answer from MCP evidence first.

After any MCP update, run `cargo test --manifest-path tools/pilcrow-mcp/Cargo.toml`.

## What is Pilcrow

Pilcrow is a Rust full-stack web framework inspired by SvelteKit/Astro. It uses:

- **Axum** as the HTTP server
- **Askama** for compile-time HTML templating
- **silcrow.js** — Always read `crates/runtime/assets/silcrow.js` via MCP tool `silcrow-docs` before writing anything about it.
- A **build.rs** pipeline (`routekit`) that compiles `.html` + `.rs` files into a wired axum `Router` — no manual route registration

The repo workspace root `Cargo.toml` has only the framework crates as members. `tools/cli` is excluded from the workspace and must be built separately. In the `pilcrow-silcrow` integration workspace, `address-book/` is the consumer app that depends on Pilcrow by path.

## Build Commands

```bash
# From the pilcrow-silcrow workspace root (canonical)
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-web
cargo test  --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit
cargo test  --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit -- <test_name>

# Address-book consumer app
cargo build --manifest-path address-book/Cargo.toml
cargo run   --manifest-path address-book/Cargo.toml

# MCP server (run after any MCP file change)
cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml

# Silcrow JS runtime (run after editing silcrow/src/silcrow.js)
node silcrow/build.js
```

Config is in `Pilcrow.toml` (walks up from cwd). Defaults: web on `127.0.0.1:3000`, backend on `127.0.0.1:4000`. Env overrides: `PILCROW_WEB_HOST`, `PILCROW_WEB_PORT`, `PILCROW_BACKEND_URL`.

## Crate Map

| Crate | Role |
|---|---|
| `crates/core` | `AppError`, `AppResult`, `PilcrowConfig`, `Meta` — shared primitives with no framework deps |
| `crates/routekit` | Build-time pipeline: discovers `.html`/`.rs` sources, transpiles templates, emits generated Rust |
| `crates/runtime` | Runtime: `Req`, `Res`, SSE, WebSocket, adapters, island SSR, FSR, `LiveProp` |
| `crates/macros` | `#[handler]` proc-macro, `sse!` macro |
| `crates/client` | `PilcrowClient` — typed HTTP client wrapping `reqwest` |
| `crates/web` | Thin facade; re-exports everything a `web` app needs under `pilcrow_web::*` |
| `tools/cli` | `pilcrow new`, `pilcrow dev`, `pilcrow export`, `pilcrow routes` |

## Known Bugs (do not introduce regressions while fixing)

| # | Location | Severity | Description |
|---|----------|----------|-------------|
| — | — | — | No known bugs at this time. |

## How the Build Pipeline Works

`build.rs` in a web app calls `routekit::compile_current_crate_sources()`, which:

1. **Discovers** `pages/**/*.html`, configured `[[fragments]]`, `ui/**/*.html`, and their `.rs` code-behind files
2. **Classifies** special files: `_layout.html`, `_error.html`, `_not_found.html`, `_loading.html`
3. **Reads** `Pilcrow.toml` for fragment directory configuration (`[[fragments]]`)
4. **Transpiles** Askama-dialect HTML → Askama templates in `$OUT_DIR/pilcrow_templates/`
5. **Generates** `$OUT_DIR/generated_app.rs` — a `build_router()` fn wiring all routes
6. **Generates** `$OUT_DIR/generated_routes.rs`, `$OUT_DIR/generated_api_mods.rs`, `$OUT_DIR/generated_typed_routes.rs`, `$OUT_DIR/generated_env.rs`, `$OUT_DIR/generated_i18n.rs`
7. **Detects** `hooks.rs` — if present, wires global request/error/startup hooks

The `pilcrow_app!()` macro in `main.rs` includes these generated files and also generates `pilcrow_router()`, `pilcrow_start()`, and `pilcrow_export()` functions.

## File Conventions

```
src/
  main.rs               # Cargo binary entrypoint only
pages/
  index.html            # Route: GET /
  index.rs              # Code-behind: Props struct, load(), named action fns
  _layout.html          # Auto-wraps all sibling/child pages — not a route
  _error.html           # Shown when load() returns Err — not a route
  _not_found.html       # axum fallback — not a route
  _loading.html         # Skeleton shown during navigation — injected as <template>
  [id]/
    index.html          # Route: GET /:id
  [id=integer]/
    index.html          # Route: GET /:id, but only when id passes params/integer::match_param
  (admin)/
    _layout.html        # Layout group — wraps children, NOT part of the URL
    dashboard.html      # Route: GET /dashboard  (group name stripped from URL)
ui/
  Button.html           # Reusable components, imported with frontmatter imports
widgets/                # Example fragment dir (configured in Pilcrow.toml)
  user-card.html        # Route: GET /widgets/user-card (no layout wrapping)
api/
  health.rs             # API route: handlers live here, separate from page routes
params/
  integer.rs            # Param matcher: `pub fn match_param(value: &str) -> bool`
hooks.rs                # Optional: global request/error/startup hooks
```

Route parameters use `[param]` in directory names. `[id=integer]` maps to `params/integer.rs`.

### Per-page Options

Declare in a code-behind file (or `---` frontmatter):

```rust
pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
pub const LAYOUT: &str = "none";           // opt out of all layout wrapping
pub const PROMOTE_AFTER: u32 = 50;         // FSR: bake after N hits (0 = bake on first hit)
```

- **`PROMOTE_AFTER: u32`**: FSR hit-count promotion threshold. `0` means bake on first hit. This is the only supported static-baking surface.
- Use `pilcrow_start(pilcrow_router()).await` (generated by `pilcrow_app!()`) to start the server.

**Removed constants — produce a build error if declared:**

| Constant | Replacement |
|----------|------------|
| `PRERENDER: bool` | Use `PROMOTE_AFTER: u32 = 0` for bake-on-first-hit |
| `REVALIDATE: u64` | Use `#[revalidate(N)]` on `LiveProp<T>` fields in `Props` |
| `STREAMING: bool` | Removed — no replacement |
| `CACHE_TAGS`, `MAX_STALE`, `CACHE_VARY` | Removed — no replacement |

All valid constants are stripped from the emitted module and never reach the template.

## Code-Behind Pattern

Each page has two files: `products.html` (template) and `products.rs` (logic).

`products.rs` exports:
- `pub struct Props { ... }` — passed to the template
- `pub async fn load(req: Req) -> AppResult<Props>` — GET handler (**required** if `.rs` file exists)
- `pub async fn <name>(req: Req) -> ActionResult` — named actions. The URL fragment `?/<name>` dispatches to `fn <name>` (POST only).

**`load()` signature is enforced by the build pipeline.** Must be `async`, return `AppResult<Props>`, take `req: Req`. If a page needs no dynamic data, omit the `.rs` file — the page is served as a static template.

The framework injects `use pilcrow_web::Req;`, `use pilcrow_web::ActionResult;`, and `use pilcrow_web::redirect;` at the top of code-behind files automatically.

## Key Types

**`Req`** — unified request context: `.params`, `.query: FormMap`, `.form: FormMap`, `.cookies`, `.headers`, `.path`, `.is_enhanced`, `.locals: Locals`, `.res: Res`.
- `req.fail(FormErrors) -> ActionResult` — JSON if enhanced, flash cookie + redirect if plain POST
- `req.take_form_flash() -> Option<FormErrors>` — reads and clears `silcrow_form_flash` cookie

**`Locals`** — per-request typed store: `.set<T>(value)`, `.get<T>() -> Option<T>`, `.require<T>() -> Result<T, AppError>`, `.has<T>() -> bool`.

**`Res`** — response modifier via `req.res`: `.with_status()`, `.with_header()`, `.with_cookie()`, `.no_cache()`, `.with_toast()`, `.trigger_event()`, `.retarget()`, `.push_history()`, `.patch_target()`, `.invalidate_target()`, `.client_navigate()`, `.sse()`, `.ws()`.
- `trigger_event`, `patch_target`, `invalidate_target` accumulate; others overwrite.

**`LiveProp`** — re-evaluated on interval or explicit trigger for real-time UI updates.

## Response Builders

| Builder | Use |
|---|---|
| `redirect("/path")` | `ActionResult`: 303 redirect (action fns) |
| `navigate("/path")` | `NavigateResponse`: 303 redirect with `ResponseExt` |
| `json(value)` | JSON response |
| `status(StatusCode::...)` | Bare status |
| `form_errors().error("field", "msg").value("field", val)` | In-place form patching via silcrow.js |
| `AppError::Redirect("/path")` | Redirect from `load()` before render |

All builders implement `ResponseExt`: `.with_toast()`, `.with_header()`, `.with_status()`, `.no_cache()`, `.trigger_event()`, `.retarget()`, `.push_history()`, `.patch_target()`, `.invalidate_target()`, `.client_navigate()`, `.sse()`, `.ws()`.

## Actions

Named actions are `pub async fn <name>(req: Req) -> ActionResult`. POST only. Plain `POST /path` with no `?/<name>` returns 404. Unknown action names return `AppError::NotFound` via `_error.html`. Layouts and UI components cannot define actions (build error).

## silcrow.js

> **Always read `crates/runtime/assets/silcrow.js` (or use MCP `silcrow-docs`) before writing anything about silcrow's API.**

HTTP verb attributes: `s-get`, `s-post`, `s-put`, `s-patch`, `s-delete`. Reactive bindings: `:text`, `:show`, `:value`, `:class`, `:style`, `:disabled`. List reconciliation: `<template s-for="item in items" :key="item.id">`.

## ISR (Incremental Static Regeneration) — Removed

`REVALIDATE`, `PRERENDER`, `STREAMING`, `CACHE_TAGS`, `MAX_STALE`, and `CACHE_VARY` are **removed**. Declaring any of them in a code-behind file is a **build error**.

- Replace `PRERENDER: bool = true` with `PROMOTE_AFTER: u32 = 0` (FSR bake-on-first-hit).
- Replace `REVALIDATE: u64 = N` with `#[revalidate(N)]` on `LiveProp<T>` fields in `Props`, or set `[fsr] revalidate_seconds` in `Pilcrow.toml` for a global default.

## Adapter System

Implemented via `PilcrowAdapter` trait (`crates/runtime/src/adapter.rs`). Built-in adapters: `TokioAdapter` (default), `PortEnvAdapter`, `FlyAdapter`, `RailwayAdapter`, `CloudRunAdapter`, `RenderAdapter`, `VercelAdapter`. Lambda adapter requires `lambda` feature flag.

Use: `pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, MyAdapter).await`.

## routekit Codegen

Codegen is split across `crates/routekit/src/templating/codegen/`: `types.rs`, `api_routes.rs`, `page_routes.rs`, `templates.rs`, `instrument.rs`, `app_module.rs`, `emit.rs`, `util.rs`.

Key structs:
- `LoadSignature` — all page/layout loads validated as `async fn(req: Req) -> AppResult<Props>`
- `ActionFn` — discovered actions; `emit_action_route` generates one POST route per page with `match req.action()` dispatch
- `make_merged_props_struct` — builds `__MergedProps` when layout has `load()`; detects field name collisions at build time

When editing codegen, always run `cargo test -p pilcrow-routekit`. Note: tests at lines 544 and 625 currently fail to compile (Bug #1).

## Codex-Specific Notes

- **ECC Baseline**: Treat `.codex/config.toml` as the default ECC-safe baseline for work in this repository. The generated baseline enables GitHub, Context7, Exa, Memory, Playwright, and Sequential Thinking.
- **Skills**:
  - Repo-generated Codex skill: `.agents/skills/Pilcrow/SKILL.md`
  - Claude-facing companion skill: `.claude/skills/Pilcrow/SKILL.md`
- **Credentials**: Keep user-specific credentials and private MCPs in `~/.codex/config.toml`, not in this repo.
- **Multi-Agent Support**:
  - Explorer: read-only evidence gathering
  - Reviewer: correctness, security, and regression review
  - Docs researcher: API and release-note verification
