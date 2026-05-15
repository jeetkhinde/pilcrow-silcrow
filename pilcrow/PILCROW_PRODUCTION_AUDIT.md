# Pilcrow Production Audit

Date: 2026-04-29

## Phase 1: Architecture

Pilcrow is a compile-time codegen SSR framework over Axum.

Core structure:

- `crates/routekit`: file discovery, route parsing, template preprocessing, layout wrapping, Askama codegen, generated Axum route modules.
- `crates/runtime`: runtime web server, request/response types, ISR cache, deferred streaming, SSE/WS, i18n, image handler, dev reload, adapters.
- `crates/web`: public framework facade and `pilcrow_app!()` macro that includes generated app/router/env/i18n code.
- `crates/client`: typed backend HTTP client injected into `load()`.
- `crates/core`: config, errors, envelopes.
- `tools/cli`: `new`, `dev`, `export`.
- `tools/mcp/pilcrow-mcp`: MCP docs, inspection, diagnostics, validation, scaffolding.
- `sandbox/apps/web`: example app.

Entry points:

- App entry: `sandbox/apps/web/src/main.rs`
- Public app macro: `crates/web/src/lib.rs`
- Runtime server start: `crates/runtime/src/start.rs`
- Build-time route/template compile: `crates/routekit/src/lib.rs`
- CLI entry: `tools/cli/src/main.rs`

Current architecture in one sentence: Pilcrow scans `src/pages`, `src/ui`, `src/api`, and configured fragments at build time, emits Askama template modules plus Axum route handlers, then runs those handlers through a runtime layer that provides `Req`, `Res`, ISR, deferred streaming, client navigation, SSE/WS, images, i18n, and dev reload.

## Phase 2: Critical Gaps

### Rendering

- SSR exists and is type-checked through generated Axum handlers.
- Deferred streaming exists via inline `<script>` patch chunks, but this is not full HTML streaming with backpressure-aware nested suspense semantics.
- Islands exist as fragment routes and `<island>` transpilation, but this is not mature Astro-style island hydration with component lifecycle, bundling, props serialization guarantees, and per-framework adapters.
- Client boundaries are Pilcrow-specific, not ecosystem-compatible React/Svelte/Solid boundaries.

### Routing

- File routing, groups, dynamic params, catch-all, layouts, loading, error, and not-found pages are present.
- Weakness: conventions are half Next-like, half custom. `_not_found.html` is discovered, while parser comments still mention `not-found`; this needs one canonical public convention.
- Nested layouts exist, but route-level metadata, redirects, parallel routes, and intercepting routes appear partially represented rather than fully integrated.

### Data Layer

- `load(req)` model is clear and Rust-native.
- `PilcrowClient` is thin HTTP sugar only.
- No first-class server actions comparable to Next/SvelteKit ergonomics beyond form/action generation.
- MCP integration is strong for framework knowledge, inspection, validation, and scaffolding, but it is not a content system yet.

### Caching

- ISR exists, but production cache providers are config-only for Redis/SQLite. Runtime falls back to memory unless provider is filesystem.
- Cache invalidation exists by path/tag.
- Missing: distributed coalescing, native TTL eviction, cache headers, admin-safe inspect endpoint, stale revalidation timeout enforcement.

### DX

- CLI is intentionally thin. `dev` shells out to `cargo watch` and `cargo run`, which keeps the workflow aligned with common Rust tooling.
- Build errors are often panics or generated-code errors, not source-mapped diagnostics.
- Hot reload is partial: CSS hot swap plus cargo-watch restart.
- Boilerplate is acceptable for Rust, too heavy versus SvelteKit/Astro.

### Deployment

- Binary deployment is a strength.
- Adapters exist conceptually, including Lambda behind a feature.
- Edge/WASM readiness is weak because the runtime assumes Tokio/Axum/reqwest/filesystem in core paths.
- No production adapter contract docs, no examples for Fly/Cloud Run/Lambda, no zero-config deploy flow.

## Phase 3: Bug And Code Quality Audit

### 1. Startup panics on config load

File: `crates/runtime/src/start.rs`

Problem: server startup panics on config load.

Why it matters: production processes should return structured startup errors, not crash with `expect`.

Suggested fix: make `start*` return `Result<(), PilcrowError>` or add `try_start*`.

```rust
pub async fn try_start_with_adapter<F, Fut, A>(...) -> AppResult<()> {
    let config = Arc::new(PilcrowConfig::load_from_current_dir()?);
    adapter.serve(&bind_addr, app).await?;
    Ok(())
}
```

### 2. ISR cache uses blocking mutex

File: `crates/runtime/src/isr.rs`

Problem: ISR uses `std::sync::Mutex` around the entire cache and performs filesystem writes while holding the lock.

Why it matters: this blocks async worker threads and serializes all cache operations under load.

Suggested fix: use `tokio::sync::RwLock` or `DashMap`; move persistence outside the critical section.

### 3. Filesystem cache writes are ignored and non-atomic

File: `crates/runtime/src/isr.rs`

Problem: filesystem cache writes are ignored and non-atomic.

Why it matters: silent cache corruption/loss and bad production observability.

Suggested fix: write to temp file, `sync_all` as configured, rename atomically, return/log errors.

### 4. Redis/SQLite providers are advertised but not implemented

File: `crates/core/src/config/config.rs`

Problem: config advertises `Sqlite` and `Redis`, but runtime only implements filesystem/memory.

Why it matters: users can configure a production-looking cache and silently get non-production behavior.

Suggested fix: fail startup for unsupported providers until implemented.

### 5. ISR inspect endpoint is always mounted

File: `crates/runtime/src/start.rs`

Problem: `/__pilcrow/isr` is always mounted.

Why it matters: exposes cache keys/tags in production.

Suggested fix: gate behind dev mode or explicit admin auth/config.

### 6. Response state lock can cascade panics

File: `crates/runtime/src/context.rs`

Problem: `Res` methods use `lock().unwrap()` unlike `Locals`, which handles poisoned locks.

Why it matters: one panic can poison response state and trigger cascading panics.

Suggested fix: use `unwrap_or_else(|e| e.into_inner())` or return a result.

### 7. Codegen validation uses panic

File: `crates/routekit/src/templating/codegen/app_module.rs`

Problem: build validation uses `panic!`.

Why it matters: errors are not source-spanned and are harder for users/AI tools to repair.

Suggested fix: return `io::Error` with route, source path, and suggested patch.

### 8. Generated handlers panic on template render errors

File: `crates/routekit/src/templating/codegen/app_module.rs`

Problem: generated handlers use `.expect("template render failed")`.

Why it matters: a render failure can crash request handling instead of rendering `_error.html`.

Suggested fix: convert render errors into `AppError::Internal` and route through page error boundary.

### 9. HTML rewriting is custom string scanning

File: `crates/routekit/src/templating/compiler.rs`

Problem: HTML rewriting is custom string scanning.

Why it matters: fragile around malformed HTML, comments, case variants, and framework syntax.

Suggested fix: use an HTML parser for structural transforms or constrain syntax and produce diagnostics.

### 10. Deferred fields spawn unbounded tasks

File: `crates/runtime/src/deferred.rs`

Problem: deferred fields spawn unbounded tasks per request.

Why it matters: high traffic plus slow futures can exhaust runtime resources.

Suggested fix: add cancellation on client disconnect, limits, tracing spans, and configurable timeout.

### 11. Deferred streaming assumes runtime globals and permissive CSP

File: `crates/runtime/src/deferred.rs`

Problem: inline streamed scripts assume client runtime globals exist.

Why it matters: pages without runtime or strict CSP break deferred streaming.

Suggested fix: inject nonce-aware shims and support external runtime script mode.

### 12. Dev workflow should standardize on Rust-native tooling

File: `tools/cli/src/dev.rs`

Problem: the audit originally treated external `cargo-watch` as a weakness, but using the normal Cargo ecosystem is the right default for Pilcrow.

Why it matters: developers should not need to learn a Pilcrow-specific watcher when `cargo watch` already works well and is familiar across Rust projects.

Suggested fix: keep `pilcrow dev` as a thin, well-documented wrapper over `cargo watch`; add repo-level `just` recipes for contributors; reserve `xtask` for future workflows that need real Rust logic.

## Phase 4: Production Readiness Checklist

### Must Have

- Replace panics/expect in startup, generated handlers, and codegen validation with structured errors.
- Implement or reject Redis/SQLite cache providers.
- Secure or disable internal inspect endpoints in production.
- Async-safe cache internals and atomic persistence.
- Source-mapped diagnostics for template/codegen errors.
- CSP-compatible runtime/deferred streaming.
- Documented security model for forms, CSRF, actions, cookies, headers, and HTML patching.
- End-to-end tests for generated sandbox app routes, actions, ISR, errors, and client runtime.
- Deployment guides and tested adapters.

### Should Have

- Standardized contributor commands with `just`; keep app dev powered by `cargo watch`.
- Better CLI: `pilcrow check`, `pilcrow routes`, `pilcrow doctor`, `pilcrow build`.
- Route manifest inspection.
- Bundle/runtime size budget for `silcrow.js`.
- Examples for auth, database, forms, streaming, cache invalidation.
- OpenTelemetry/tracing integration.

### Nice To Have

- Edge/WASM adapter.
- Plugin ecosystem.
- Visual route inspector.
- AI-generated migration/fix suggestions via MCP.

## Phase 5: DX Improvements

Preferred cache policy syntax:

Keep the existing Rust-constant style. It is simple, type-checked, grep-friendly, and consistent with the current code-behind pattern.

```rust
pub const REVALIDATE: u64 = 60;
pub const CACHE_TAGS: &[&str] = &["products"];
```

Do not replace this with a procedural attribute unless there is a future need for richer compile-time validation that constants cannot provide.

Before:

```rust
pub async fn load(req: Req) -> AppResult<Props> {
    let id = req.params.get("id").ok_or(AppError::NotFound)?;
    Ok(Props { id: id.to_string() })
}
```

After:

```rust
// Route path is still owned by the filesystem:
// src/pages/products/[id:int].html
//
// Routekit generates:
// pub struct Params { pub id: i64 }
// pub type Page = pilcrow_web::Page<Params>;
pub async fn load(ctx: Page) -> AppResult<Props> {
    Ok(Props { id: ctx.params.id })
}
```

Decision: no `#[page("/products/[id:int]")]` for normal pages. Routekit remains file/folder based; the path is the source of truth. Dynamic route params are generated as a page-local `Params` type plus a page-local `Page` alias, matching the existing `Props` convention and avoiding duplicated route declarations.

Decision: no `#[cache(...)]` attribute for normal cache policy. Routekit should continue discovering `REVALIDATE` and `CACHE_TAGS` constants from code-behind files.

CLI improvements:

- `pilcrow dev`: thin wrapper around `cargo watch`, with clear install guidance when missing.
- `pilcrow check`: run routekit compile plus diagnostics without full app build.
- `pilcrow routes`: print route tree, layouts, cache policy, actions.
- `pilcrow explain /products/[id]`: show generated handler sources and matched files.
- `pilcrow doctor`: MCP-backed project diagnosis.

Contributor command runner decision:

- Adopt `just` for the Pilcrow repository itself. It is cross-platform enough for Rust contributors, easy to read, supports environment variables and multi-step workflows, and avoids hiding normal Cargo commands.
- Do not require `just` for generated Pilcrow apps. Scaffolded apps should work with `cargo run`, `cargo watch`, and `pilcrow dev`.
- Do not use Cargo aliases as the primary workflow layer. They are useful for short local conveniences but too limited for multi-crate commands, separate manifests, and environment setup.
- Do not use `make` as the primary workflow layer. It is ubiquitous on Unix, but less pleasant on Windows and less idiomatic for modern Rust projects.
- Do not add `xtask` yet. Use it later if Pilcrow needs substantial custom automation that benefits from Rust types and shared code.

## Phase 6: Architecture Recommendations

### MCP replacing markdown

Strong recommendation: do not replace markdown entirely.

Pros:

- AI-native querying.
- Project-aware diagnostics.
- Generated examples.
- Structured feature registry.

Cons:

- Poor SEO.
- Poor GitHub readability.
- Worse onboarding.
- Editor/tooling friction.
- Vendor/protocol risk.

Recommendation: use a hybrid. Markdown/MDX should be the canonical public docs. MCP should expose the same docs plus route inspection, validation, examples, and codegen diagnostics.

### silcrow.js runtime

It should exist, but only as a hypermedia/progressive enhancement runtime.

Do not try to compete with React/Svelte/Solid for rich client apps.

Let Pilcrow support islands that mount existing ecosystem components later.

Keep `silcrow.js` small, boring, CSP-compatible, and focused on forms, patching, SSE/WS, navigation, and deferred slots.

### AI-first framework

AI should drive:

- Scaffolding.
- Diagnosis.
- Migration.
- Route inspection.
- Safe code generation.

AI should not be in:

- The request path.
- Routing semantics.
- Rendering correctness.
- Cache invalidation core.

The framework must be excellent without AI; AI should make it faster to use.

## Phase 7: Roadmap

### Week 1-2

- Remove production panics in startup/generated handlers.
- Fail fast for unsupported Redis/SQLite providers.
- Gate `/__pilcrow/isr`.
- Add `pilcrow check` and `pilcrow routes`.
- Add source-path diagnostics for the most common template/codegen failures.

### Month 1

- Async-safe ISR cache with atomic filesystem backend.
- Real Redis backend or remove Redis config.
- `just`-based contributor workflow and polished `pilcrow dev` cargo-watch wrapper.
- End-to-end sandbox tests through HTTP.
- CSP-compatible deferred streaming.
- Documentation site generated from markdown plus MCP resources.

### Pre-launch

- Production deployment examples for binary, Docker, Fly/Cloud Run, and Lambda.
- Security review of `silcrow.js`, HTML patching, CSRF, cookies, headers.
- Stable public conventions for routes/layouts/errors/loading.
- Compatibility story for islands using React/Svelte/Solid or a clear decision not to support them.
- Performance benchmarks against simple Next/SvelteKit/Astro SSR pages.

## Verification

`cargo test` passed across the workspace during the audit: 244 tests passed, plus doc tests ignored as expected.
