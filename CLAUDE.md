# CLAUDE.md — pilcrow-silcrow workspace

## Workspace topology — monorepo

Everything lives in one git repo. Commit from the workspace root.

```text
pilcrow-silcrow/               ← single git repo
  pilcrow/                     # Pilcrow framework (Rust SSR engine)
  silcrow/                     # Silcrow client runtime (JS)
  demo/                        # Real consumer app, depends on Pilcrow by path
  plans/                       # Feature implementation plans
  .claude/commands/            # Project slash commands
  .claude/hooks/               # Stop hook scripts
  .claude/react-island.md      # React island operating rules + decision guide
  .claude/react-hook-guide.md  # Hook reference with examples
  .claude/rendering-models.md  # All rendering modes: SSR, ISR, SSG, streaming, deferred, islands
```

**Publishing to crates.io (future):** Run `cargo publish -p <crate-name>` from the workspace root.
Path deps in `demo/Cargo.toml` must become version deps before publishing Pilcrow crates.
Use `git subtree split --prefix=pilcrow` to extract a clean Pilcrow-only history if a separate public repo is ever needed.

NEVER call `scan_project_context` if this file was loaded at session start — topology,
architecture, and build commands are all here. Only call it when debugging a specific
build failure or verifying a file path you cannot infer from this document.

## What Pilcrow and Silcrow are

Pilcrow is a Rust SSR engine that generates HTML and routes via build-time codegen
(`pilcrow-routekit`). Silcrow is a client-side JS runtime that handles DOM patching,
reactive state, and custom directives. They communicate through embedded assets: Pilcrow
bundles silcrow.js at Cargo build time via `build.rs`.

## Pilcrow crate map

| Crate | What it owns | Key files |
| ----- | ----------- | --------- |
| `pilcrow-core` | Domain primitives, config, envelope, error types | `src/config/config.rs`, `src/envelope/envelope.rs` |
| `pilcrow-macros` | Proc-macros: `#[handler]`, SSE helpers | `src/lib.rs`, `src/handler.rs` |
| `pilcrow-routekit` | File-based routing, codegen, templating (React/Solid/i18n) | `src/routing/`, `src/templating/`, `src/codegen/` |
| `pilcrow-runtime` | Axum integration, middleware, SSE/WS, assets embed, ISR, CSRF | `src/context.rs`, `src/middleware.rs`, `src/sse/`, `src/assets/` |
| `pilcrow-web` | Web/SSR integration layer (thin adapter) | `src/lib.rs` |
| `pilcrow-client` | Client-facing extractors and error types | `src/client.rs`, `src/extractor.rs` |

Touch `pilcrow-routekit` for routing/codegen/template changes. Touch `pilcrow-runtime` for middleware, assets, SSE, or CSRF.

## Integration contract

**Never shortcut this chain:**

```
silcrow/src/silcrow.js                  ← edit source here only
  └─ node build.js                      ← run inside silcrow/ (uses terser, no npm needed)
       └─ silcrow/dist/silcrow.min.js   ← minified output
            └─ build.js auto-copies it to pilcrow/crates/runtime/assets/silcrow.js
                 └─ assets.rs embeds via include_str!("../../assets/silcrow.js")
```

After editing silcrow source: `node build.js` in `silcrow/`, then `cargo build -p pilcrow-runtime` to verify the embed.
terser must be available: `npm install` inside `silcrow/` if it is missing.

## Build commands (from workspace root)

```bash
# Pilcrow framework
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit
cargo test  --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit

# Demo consumer app
cargo build --manifest-path demo/Cargo.toml
cargo run   --manifest-path demo/Cargo.toml

# Silcrow JS runtime
cd silcrow && npm run build

# MCP server tests (run after any MCP file change)
cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml
```

Use `/sync-check` to run the full chain in sequence with failure reporting.

## Attribute namespace table

| Prefix | System | Written by | Read by |
|--------|--------|-----------|---------|
| `s-*`, `:<prop>` | Silcrow | Developer in HTML templates | Silcrow client JS |
| `data-pilcrow-*` | Pilcrow | routekit at build/render time | react-islands.js |

These two systems do not overlap. Silcrow never reads `data-pilcrow-*` attributes.

## react-islands.js

Lives in `pilcrow/crates/runtime/assets/react-islands.js` — a Pilcrow asset, not Silcrow.
It listens for Silcrow's `silcrow:patched` CustomEvent and re-scans the patched target
for `[data-pilcrow-react]` elements to mount React islands. Silcrow has no React knowledge.

## Rendering mode questions — read docs, do not scan

For ANY question about rendering modes (SSR, ISR, SSG, streaming, deferred fields, island strategies):

1. Read `.claude/rendering-models.md` — all modes, configuration constants, constraints, and combination rules

Do NOT open `pilcrow/crates/runtime/src/isr.rs`, `codegen/app_module.rs`, or `deferred.rs` unless
you are actively debugging a mismatch between the docs and real behavior.

## React island questions — read docs, do not scan

For ANY question about React usage in Pilcrow/Silcrow:

1. Read `.claude/react-island.md` — operating rules, mental model, decision guide
2. Read `.claude/react-hook-guide.md` — every hook with usage examples

Do NOT open `pilcrow/crates/routekit/src/templating/react.rs`, `react-islands.js`,
or `demo/react/` unless the user asks for implementation internals or you are
actively debugging a mismatch between the docs and real behavior.

The authoritative hook source is the `PILCROW_REACT_TS` constant in `react.rs` —
but the two files above should answer all normal usage questions without reading it.

## MCP tools — when to call vs skip

| Tool | When to call |
|------|-------------|
| `scan_project_context` | SKIP (this file is loaded). Only on build failure or unknown file path. |
| `list_features` | Freely — cheap read |
| `explain_feature` | Before writing any feature-specific code |
| `compare_patterns` | When choosing between two implementation approaches |
| `validate_implementation` | Before scaffolding any planned/unstable syntax |
| `diagnose_project` | Only when `cargo build` fails |
| `diagnose_route` | Only when a specific route returns a runtime error |
| `inspect_generated_route` | Only when debugging generated code output |
| `codegen_read` | Before modifying any codegen output format |
| `orchestrate_feature` | For multi-step feature scaffolding only |

**Silcrow MCP note**: `silcrow-mcp` docs may be stale for the atom API layer
(`mergePath`, `createAtom`, `routeAtoms`). Read `silcrow/src/silcrow.js` directly
for atom API shape — do not trust silcrow-mcp for this.

## Mandatory update checklist — ENFORCE ON EVERY FEATURE CHANGE

**The four-layer rule:** When shipping or changing any feature, all four layers must be updated in the same commit:
1. **Implementation** — source code
2. **`pilcrow/registry.toml`** — `[[features]]` entry with `id`, `name`, `domain`, `status`, `summary`, `spec`, `source_refs`, `test_refs`
3. **MCP knowledge** — `pilcrow/tools/pilcrow-mcp/src/docs.rs` `DocumentSpec` entries for new/changed files
4. **Executable test** — at least one runnable test covering the new behaviour

Use `/update-docs <feature-name>` to run this interactively.

**After ANY Pilcrow framework behavior change:**
1. `pilcrow/registry.toml` — add/update `[[features]]` entry (must have `id`, `domain`, `summary`)
2. `pilcrow/tools/pilcrow-mcp/src/validation.rs` — remove resolved / add new error rules
3. `pilcrow/tools/pilcrow-mcp/src/docs.rs` — corpus is 3 fixed `.md` docs only; no source-file entries. Only update if a doc file is renamed or a new prose doc is added.
4. Run: `cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml`
   (`cargo check` is NOT sufficient — golden tests fail silently under check)

**After adding or changing any hook in `PILCROW_REACT_TS` (react.rs):**
1. Update `.claude/react-island.md` — Core APIs table and Decision guide
2. Update `.claude/react-hook-guide.md` — add/update the hook section and the choosing table

**After ANY Silcrow public API change:**

1. Edit `silcrow/src/` only (never `dist/`)
2. `node build.js` in `silcrow/` — auto-copies minified output to `pilcrow/crates/runtime/assets/silcrow.js`
3. Update `silcrow/docs/silcrow-api.md` in the same commit
4. `cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime` to verify embed

**After ANY AGENTS.md-level architecture change:**

- Update `AGENTS.md` and this `CLAUDE.md` to reflect the new topology or contract

## Active plans

None.

## Known issues — do not work around without fixing root cause

None.

## Hard rules — never violate

- **Never manually edit** `pilcrow/crates/runtime/assets/silcrow.js` — always regenerated by `node build.js` in `silcrow/`; edit `silcrow/src/silcrow.js` instead
- **Pilcrow-specific JS glue** (slot shims, `__pd` deferred helpers) must go in inline `<script>` from `codegen.rs`, never in `silcrow.js`

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- ALWAYS read graphify-out/GRAPH_REPORT.md before reading any source files, running grep/glob searches, or answering codebase questions. The graph is your primary map of the codebase.
- IF graphify-out/wiki/index.md EXISTS, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
