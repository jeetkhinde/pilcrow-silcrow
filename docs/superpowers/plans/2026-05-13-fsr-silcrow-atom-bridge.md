# FSR → Silcrow Atom Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the FSR watcher pushes an object-valued slot over SSE, route it through `window.Silcrow.publish('fsr.<slot>', value)` so `s-use="fsr.<slot>"` and `:text` bindings update reactively — without changing `s-live` scalar patching.

**Architecture:** The FSR inline JS script (embedded in generated route modules) already handles the SSE event. A single branch on `typeof v === 'object'` is all that separates scalar patching from atom publishing. `LiveProp<T>` and `from_row()` codegen already use `serde_json::from_value` universally — no codegen changes needed.

**Tech Stack:** Rust (routekit codegen), inline JavaScript string in `app_module.rs`, `window.Silcrow.publish()` (existing Silcrow public API)

---

## File Map

| File | Change |
|------|--------|
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | Add `fsr_patch_script()` test helper; replace FSR JS string at lines 883 and 930 |
| `pilcrow/crates/routekit/src/templating/codegen/tests.rs` | Add two tests for FSR JS branch behaviour |
| `pilcrow/crates/runtime/src/fsr/live_props.rs` | Add doc comment documenting T requirements for object fields |

No other files change. `from_row()` codegen (`fsr.rs`) already emits `serde_json::from_value` for all field types and does not need modification.

---

### Task 1: Update FSR inline JS to branch on value type

The FSR `fsr` SSE event handler currently always writes `textContent`. Add a branch: object values call `window.Silcrow.publish('fsr.' + k, v)`; scalars continue to patch `[s-live]` via `textContent`.

The JS string appears **twice** identically in `app_module.rs` (at the `let fsr_script = "..."` line inside each `if has_fsr` block, currently lines 883 and 930). Both must be updated to the same new string.

**Files:**
- Modify: `pilcrow/crates/routekit/src/templating/codegen/app_module.rs`
- Test: `pilcrow/crates/routekit/src/templating/codegen/tests.rs`

- [ ] **Step 1: Write failing tests**

Add these two tests to `pilcrow/crates/routekit/src/templating/codegen/tests.rs` inside the existing `mod tests` block:

```rust
#[test]
fn fsr_script_routes_objects_to_silcrow_publish() {
    let script = super::app_module::fsr_patch_script();
    assert!(
        script.contains("window.Silcrow&&window.Silcrow.publish"),
        "expected Silcrow.publish call in FSR script"
    );
    assert!(
        script.contains("typeof v==='object'"),
        "expected typeof object check in FSR script"
    );
}

#[test]
fn fsr_script_still_patches_scalars_via_s_live() {
    let script = super::app_module::fsr_patch_script();
    assert!(script.contains("s-live"), "expected s-live selector");
    assert!(script.contains("textContent"), "expected textContent assignment");
}
```

- [ ] **Step 2: Add `fsr_patch_script()` to `app_module.rs` — with the OLD string**

Add this function at the bottom of `app_module.rs` (before the final closing brace). It must return the current (unchanged) JS string so the tests fail as expected:

```rust
/// Returns the FSR inline patch script. Exposed for tests only.
#[cfg(test)]
pub fn fsr_patch_script() -> &'static str {
    "(function(){var __fsr_route=window.location.pathname;var __fsr_es=null;function __fsr_slots(){return Array.from(document.querySelectorAll('[s-live]')).map(function(e){return e.getAttribute('s-live');}).filter(Boolean).join(',');}function __fsr_connect(){if(__fsr_es){__fsr_es.close();}var url='/__pilcrow/fsr?route='+encodeURIComponent(__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots());__fsr_es=new EventSource(url);__fsr_es.addEventListener('fsr',function(e){try{var d=JSON.parse(e.data);Object.keys(d).forEach(function(k){document.querySelectorAll('[s-live=\"'+k+'\"]').forEach(function(n){n.textContent=d[k]==null?'':String(d[k]);});});}catch(x){}});}__fsr_connect();document.addEventListener('silcrow:navigate',function(){__fsr_route=window.location.pathname;__fsr_connect();});})()";
}
```

- [ ] **Step 3: Run tests to confirm they fail**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit fsr_script 2>&1 | tail -20
```

Expected: `fsr_script_routes_objects_to_silcrow_publish` fails with "expected Silcrow.publish call in FSR script". `fsr_script_still_patches_scalars_via_s_live` passes (s-live and textContent already present in old script).

- [ ] **Step 4: Replace the FSR script string at line 883 and update `fsr_patch_script()`**

The new JS string differs only in the `forEach` callback body. Old callback: `document.querySelectorAll('[s-live=\"'+k+'\"]').forEach(function(n){n.textContent=d[k]==null?'':String(d[k]);});` — new callback introduces a `var v=d[k]` and branches on `typeof v==='object'`.

In `app_module.rs`, find the `let fsr_script = "..."` at the first `if has_fsr` block (near line 883). Replace the entire `let fsr_script = "...";` line with:

```rust
let fsr_script = "(function(){var __fsr_route=window.location.pathname;var __fsr_es=null;function __fsr_slots(){return Array.from(document.querySelectorAll('[s-live]')).map(function(e){return e.getAttribute('s-live');}).filter(Boolean).join(',');}function __fsr_connect(){if(__fsr_es){__fsr_es.close();}var url='/__pilcrow/fsr?route='+encodeURIComponent(__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots());__fsr_es=new EventSource(url);__fsr_es.addEventListener('fsr',function(e){try{var d=JSON.parse(e.data);Object.keys(d).forEach(function(k){var v=d[k];if(v!==null&&typeof v==='object'){if(window.Silcrow&&window.Silcrow.publish){window.Silcrow.publish('fsr.'+k,v);}}else{document.querySelectorAll('[s-live=\"'+k+'\"]').forEach(function(n){n.textContent=v==null?'':String(v);});}});}catch(x){}});}__fsr_connect();document.addEventListener('silcrow:navigate',function(){__fsr_route=window.location.pathname;__fsr_connect();});})()";
```

Also update `fsr_patch_script()` at the bottom of the file to return this same new string.

- [ ] **Step 5: Replace the identical FSR script string at line 930**

The second `if has_fsr` block (near line 930) contains an identical `let fsr_script = "..."` line. Replace it with the same new string from Step 4.

- [ ] **Step 6: Run tests to confirm they pass**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit 2>&1 | tail -20
```

Expected: all tests pass including both new `fsr_script_*` tests.

- [ ] **Step 7: Commit**

```bash
git add pilcrow/crates/routekit/src/templating/codegen/app_module.rs \
        pilcrow/crates/routekit/src/templating/codegen/tests.rs
git commit -m "feat(fsr): route object LiveProp values to Silcrow.publish for s-use binding"
```

---

### Task 2: Document T requirements on LiveProp

`from_row()` codegen emits `serde_json::from_value(v.clone()).ok().unwrap_or_default()` for every field. This compiles only when `T: DeserializeOwned + Default`. Currently `LiveProp<T>` has no documentation explaining this — a developer using `LiveProp<MyStruct>` will get a cryptic error deep in generated code if they forget the derives.

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/live_props.rs`

- [ ] **Step 1: Replace the doc comment on `LiveProp<T>`**

In `live_props.rs`, find:

```rust
/// A field whose value is tracked, cached, and live-patched by Pilcrow FSR.
///
/// `T` must implement `serde::Serialize + serde::de::DeserializeOwned`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProp<T> {
```

Replace with:

```rust
/// A field whose value is tracked, cached, and live-patched by Pilcrow FSR.
///
/// `T` must implement `serde::Serialize + serde::de::DeserializeOwned + Default`.
/// For scalar types (`String`, `i64`, `bool`, etc.) these bounds are satisfied
/// automatically. For struct fields, add the derives explicitly:
///
/// ```rust,ignore
/// #[derive(Serialize, Deserialize, Default)]
/// pub struct TicketBadge { pub label: String, pub color: String }
///
/// pub ticket_badge: LiveProp<TicketBadge>,
/// ```
///
/// On SSE patch, object values are published to the Silcrow atom `"fsr.<slot_name>"`.
/// Bind with `s-use="fsr.ticket_badge"` and `:text="label"` in the template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProp<T> {
```

- [ ] **Step 2: Build to confirm no regressions**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1 | tail -10
```

Expected: compiles clean, no warnings introduced.

- [ ] **Step 3: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/live_props.rs
git commit -m "docs(fsr): document T bounds and object field pattern on LiveProp"
```
