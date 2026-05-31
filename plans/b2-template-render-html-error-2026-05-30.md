# B2: Template Render Errors Return HTML Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `text/plain` 500 responses on template render failures with proper `text/html` responses so browsers render a readable error page instead of raw text.

**Architecture:** Two codegen emitter functions produce plain-text 500 responses when template rendering fails and no error boundary is available. Change both to emit `axum::response::Html(...)` responses. No runtime or API changes needed — same HTTP 500 status, HTML body instead of plain text.

**Tech Stack:** Rust, pilcrow-routekit (codegen), pilcrow-web.

---

## File Map

| File | Change |
|---|---|
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | `emit_render_binding`: change `has_resp_handle=false` branch to emit HTML response |
| `pilcrow/crates/routekit/src/templating/codegen/emit.rs` | `emit_app_error_body`: change `error_mod=None` branch to emit HTML response |
| `pilcrow/crates/routekit/src/templating/codegen/tests.rs` | Two new tests asserting HTML response in generated code |
| `pilcrow/PILCROW_PRODUCTION_AUDIT.md` | Mark audit #8 as fixed |
| `plans/roadmap-2026-05-29.md` | Mark B2 done |

---

## Task 1: Fix `emit_render_binding` (static pages, no req)

**Files:**
- Modify: `pilcrow/crates/routekit/src/templating/codegen/app_module.rs:117-121`
- Test: `pilcrow/crates/routekit/src/templating/codegen/tests.rs`

This fixes the path where `has_resp_handle = false` — static pages with no `load()`, no route params, no `_error.html`. These generate a handler that has no `__resp_handle` in scope. Currently emits `(500, "template render failed").into_response()` — plain text. Fix: emit `(500, Html(...)).into_response()`.

- [ ] **Step 1: Write the failing test**

Add the following test inside the `mod tests` block at the bottom of `pilcrow/crates/routekit/src/templating/codegen/tests.rs`:

```rust
#[test]
fn static_page_template_error_uses_html_response() {
    use super::app_module::{AppCodegenMaps, render_generated_app_module};
    use crate::templating::codegen::{GeneratedPageRoute, HookFlags};
    use std::collections::{HashMap, HashSet};

    // Static page: no load(), no _error.html, no route params — needs_req=false.
    let page_entry = GeneratedPageRoute {
        pattern: "/static".to_string(),
        template_path: "/tmp/src/pages/static.html".to_string(),
        symbol: "page_static".to_string(),
        render_symbol: "render_page_static".to_string(),
        route_params: vec![],
        param_matchers: HashMap::new(),
    };

    let maps = AppCodegenMaps {
        load_map: &HashMap::new(),            // no load() → needs_req=false
        layout_fields_map: &HashMap::new(),
        error_module_for_page: &HashMap::new(), // no _error.html
        not_found_module: None,
        loading_module_for_page: &HashMap::new(),
        action_map: &HashMap::new(),
        page_options_map: &HashMap::new(),
        live_fields_map: &HashMap::new(),
        has_live_fn_map: &HashMap::new(),
        fsr_live_source_map: &HashMap::new(),
        fsr_live_fields_map: &HashMap::new(),
        fsr_default_revalidate_symbols: &HashSet::new(),
        layout_chain_ids_map: &HashMap::new(),
        page_slot_map: &HashMap::new(),
    };

    let source = render_generated_app_module(
        &[page_entry],
        &[],
        &maps,
        HookFlags { has_handle: false, has_handle_error: false, has_init: false },
        false,
        false,
    )
    .expect("render_generated_app_module should succeed");

    assert!(
        !source.contains("\"template render failed\""),
        "plain-text error string must not appear in generated source:\n{source}"
    );
    assert!(
        source.contains("axum::response::Html"),
        "HTML error response must appear in generated source:\n{source}"
    );
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit --lib -- static_page_template_error_uses_html_response 2>&1 | tail -10
```

Expected: `FAILED` — test catches the current plain-text response.

- [ ] **Step 3: Fix `emit_render_binding` in `app_module.rs`**

In `pilcrow/crates/routekit/src/templating/codegen/app_module.rs`, find the `else` branch of `emit_render_binding` (currently lines 117–121). Replace:

```rust
    } else {
        let _ = writeln!(
            s,
            "{pad}        return (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, \"template render failed\").into_response();"
        );
    }
```

with:

```rust
    } else {
        let _ = writeln!(
            s,
            "{pad}        return (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, ::pilcrow_web::axum::response::Html(\"<h1>500 Internal Server Error</h1>\")).into_response();"
        );
    }
```

- [ ] **Step 4: Run to confirm test passes**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit --lib -- static_page_template_error_uses_html_response 2>&1 | tail -5
```

Expected: `test static_page_template_error_uses_html_response ... ok`.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/routekit/src/templating/codegen/app_module.rs \
        pilcrow/crates/routekit/src/templating/codegen/tests.rs
git commit -m "fix(codegen): emit_render_binding no-req path returns HTML 500 not plain text"
```

---

## Task 2: Fix `emit_app_error_body` (page with req but no `_error.html`)

**Files:**
- Modify: `pilcrow/crates/routekit/src/templating/codegen/emit.rs:66-73`
- Test: `pilcrow/crates/routekit/src/templating/codegen/tests.rs`

This fixes the path where `has_resp_handle = true` but `error_mod = None` — pages with `load() -> Result<_, AppError>` and no `_error.html`. Currently emits `e.to_string()` as a plain text body. Fix: emit `Html(format!(...))`.

- [ ] **Step 1: Write the failing test**

Add inside the `mod tests` block in `tests.rs`:

```rust
#[test]
fn page_with_load_no_error_mod_uses_html_response() {
    use super::app_module::{AppCodegenMaps, render_generated_app_module};
    use crate::templating::codegen::{GeneratedPageRoute, HookFlags, LoadSignature};
    use std::collections::{HashMap, HashSet};

    // Page with load() -> Result, but no _error.html.
    // needs_req=true (any_load_returns_result), error_mod=None → emit_app_error_body(None, ...)
    let page_entry = GeneratedPageRoute {
        pattern: "/profile".to_string(),
        template_path: "/tmp/src/pages/profile.html".to_string(),
        symbol: "page_profile".to_string(),
        render_symbol: "render_page_profile".to_string(),
        route_params: vec![],
        param_matchers: HashMap::new(),
    };

    let mut load_map: HashMap<String, Option<LoadSignature>> = HashMap::new();
    load_map.insert(
        "page_profile".to_string(),
        Some(LoadSignature {
            is_async: true,
            wants_client: false,
            wants_req: true,
            wants_page: false,
            wants_live: false,
            returns_result: true,
        }),
    );

    let maps = AppCodegenMaps {
        load_map: &load_map,
        layout_fields_map: &HashMap::new(),
        error_module_for_page: &HashMap::new(), // no _error.html
        not_found_module: None,
        loading_module_for_page: &HashMap::new(),
        action_map: &HashMap::new(),
        page_options_map: &HashMap::new(),
        live_fields_map: &HashMap::new(),
        has_live_fn_map: &HashMap::new(),
        fsr_live_source_map: &HashMap::new(),
        fsr_live_fields_map: &HashMap::new(),
        fsr_default_revalidate_symbols: &HashSet::new(),
        layout_chain_ids_map: &HashMap::new(),
        page_slot_map: &HashMap::new(),
    };

    let source = render_generated_app_module(
        &[page_entry],
        &[],
        &maps,
        HookFlags { has_handle: false, has_handle_error: false, has_init: false },
        false,
        false,
    )
    .expect("render_generated_app_module should succeed");

    assert!(
        source.contains("axum::response::Html"),
        "HTML error response must appear in generated source:\n{source}"
    );
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit --lib -- page_with_load_no_error_mod_uses_html_response 2>&1 | tail -10
```

Expected: `FAILED`.

- [ ] **Step 3: Fix `emit_app_error_body` in `emit.rs`**

In `pilcrow/crates/routekit/src/templating/codegen/emit.rs`, find the `else` branch of `emit_app_error_body` (currently lines 66–73). Replace:

```rust
    } else {
        let _ = writeln!(
            s,
            "{pad}let mut __err_resp = (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();"
        );
        let _ = writeln!(s, "{pad}__resp_handle.apply_to(&mut __err_resp);");
        let _ = writeln!(s, "{pad}return __err_resp;");
    }
```

with:

```rust
    } else {
        let _ = writeln!(
            s,
            "{pad}let mut __err_resp = (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, ::pilcrow_web::axum::response::Html(format!(\"<h1>500 Internal Server Error</h1><p>{{e}}</p>\"))).into_response();"
        );
        let _ = writeln!(s, "{pad}__resp_handle.apply_to(&mut __err_resp);");
        let _ = writeln!(s, "{pad}return __err_resp;");
    }
```

- [ ] **Step 4: Run to confirm test passes**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit --lib -- page_with_load_no_error_mod_uses_html_response 2>&1 | tail -5
```

Expected: `test page_with_load_no_error_mod_uses_html_response ... ok`.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/routekit/src/templating/codegen/emit.rs \
        pilcrow/crates/routekit/src/templating/codegen/tests.rs
git commit -m "fix(codegen): emit_app_error_body no-error-mod path returns HTML 500 not plain text"
```

---

## Task 3: Full build gate + docs update

**Files:**
- Modify: `pilcrow/PILCROW_PRODUCTION_AUDIT.md`
- Modify: `plans/roadmap-2026-05-29.md`

- [ ] **Step 1: Run full test suite**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit --lib 2>&1 | tail -5
cargo build --manifest-path pilcrow/Cargo.toml 2>&1 | tail -3
cargo build --manifest-path address-book/Cargo.toml 2>&1 | tail -3
cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml 2>&1 | tail -3
```

Expected: all green.

- [ ] **Step 2: Mark audit #8 fixed in `PILCROW_PRODUCTION_AUDIT.md`**

Find the section starting with `### 8. Generated handlers panic on template render errors`. Change the heading to:

```markdown
### 8. ✅ Generated handlers panic on template render errors (FIXED)
```

Add a line after the `Suggested fix` line:

```markdown
**Status:** Fixed. `emit_render_binding` and `emit_app_error_body` now emit `axum::response::Html(...)` responses instead of plain-text tuples when no error boundary is available.
```

- [ ] **Step 3: Mark B2 done in roadmap**

In `plans/roadmap-2026-05-29.md`, replace:

```markdown
- [ ] **B2. Generated handlers `.expect("template render failed")`** — a single bad template crashes the request instead of rendering `_error.html`. Route render errors through `AppError::Internal` → page error boundary (`codegen/app_module.rs`, audit #8). This is also the real fix behind the "error boundary DX" gap.
```

with:

```markdown
- [x] **B2. Generated handlers returned plain-text 500 on template render errors.** ✅ `emit_render_binding` (no-req path) and `emit_app_error_body` (no-error-mod path) now emit `axum::response::Html` 500 responses. Pages with `_error.html` already used the full error boundary — only the plain-text fallback was wrong.
```

- [ ] **Step 4: Commit**

```bash
git add pilcrow/PILCROW_PRODUCTION_AUDIT.md plans/roadmap-2026-05-29.md
git commit -m "docs: mark audit #8 and roadmap B2 complete — template render errors now return HTML"
```

---

## Success criteria

- `cargo test -p pilcrow-routekit --lib` passes including 2 new tests
- `cargo build pilcrow/Cargo.toml` + `address-book/Cargo.toml` green
- `cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml` green
- Generated handler for a static page: no `"template render failed"` plain string, has `axum::response::Html`
- Generated handler for a load-result page with no `_error.html`: error branch uses `axum::response::Html`
- Pages with `_error.html` unchanged — still use `AppError::Internal` → error boundary (not regressed)
