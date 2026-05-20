# Slice E — SSE Anchor + Keyed List Wire Format Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the per-page inline `<script>` LiveProp SSE anchor with a `<div data-pilcrow-live>` DOM element Silcrow can manage, and add a `list-patch` SSE event type for keyed row-level list updates.

**Architecture:** The LiveProp inline script (`__LIVE_SHIM` + inline `EventSource`) is removed from codegen; instead a `<div data-pilcrow-live="/__pilcrow/live{path}">` element is injected. Silcrow discovers it via `initLiveElements()`, uses the existing SSE-hub infrastructure (`openLive`/`connectSseHub`), and adds a `live` event listener that patches `[data-pilcrow-live-field]` text nodes. A new `list-patch` SSE event type is added to the Rust `SilcrowEvent` API and handled by `connectSseHub` to patch `[data-pilcrow-key]` rows inside `[data-pilcrow-list]` containers.

**Tech Stack:** Rust (tokio, axum, serde_json), JavaScript (vanilla ES2020), Askama templates, pilcrow-routekit codegen.

---

## File Map

| File | Change |
|------|--------|
| `pilcrow/crates/runtime/src/sse/server_sent_events.rs` | Add `EventKind::ListPatch` + `SilcrowEvent::list_patch()` + wire serialisation |
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | Remove `__LIVE_SHIM` + inline EventSource script; inject `<div data-pilcrow-live="...">` before `</body>` |
| `silcrow/src/silcrow.js` | `initLiveElements()` discovers `[data-pilcrow-live]`; `connectSseHub()` adds `live` + `list-patch` listeners; post-swap re-scan extended; MutationObserver cleanup extended |
| `silcrow/docs/silcrow-api.md` | Document `data-pilcrow-live`, `data-pilcrow-list`, `data-pilcrow-key`, `list-patch` event, `live` event |
| `pilcrow/registry.toml` | Add `sse-anchor` and `list-patch-wire-format` feature entries |

---

## Task 1: Add `ListPatch` event to `server_sent_events.rs`

**Files:**
- Modify: `pilcrow/crates/runtime/src/sse/server_sent_events.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `pilcrow/crates/runtime/src/sse/server_sent_events.rs`, inside a `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::sse::Event;

    fn event_to_string(e: SilcrowEvent) -> String {
        let axum_event: Event = e.into();
        // Event has no public accessor — render via Debug, then extract data field
        format!("{axum_event:?}")
    }

    #[test]
    fn list_patch_serialises_with_all_fields() {
        let data = serde_json::json!({"status": "open", "priority": 1});
        let evt = SilcrowEvent::list_patch("tickets", "ticket:42", data);
        let rendered = event_to_string(evt);
        assert!(rendered.contains("list-patch"), "event name missing: {rendered}");
        assert!(rendered.contains("tickets"), "list name missing: {rendered}");
        assert!(rendered.contains("ticket:42"), "key missing: {rendered}");
        assert!(rendered.contains("status"), "changed field missing: {rendered}");
    }

    #[test]
    fn list_patch_bad_key_type_still_serialises() {
        let data = serde_json::json!({"count": 5});
        // key is numeric — should still work
        let evt = SilcrowEvent::list_patch("items", "99", data.clone());
        let rendered = event_to_string(evt);
        assert!(rendered.contains("list-patch"));
    }
}
```

Run: `cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime sse::server_sent_events::tests 2>&1`
Expected: FAIL — `list_patch` method not found.

- [ ] **Step 2: Add `ListPatch` variant and constructor**

In `server_sent_events.rs`, add `ListPatch` to the `EventKind` enum (after `Custom`):

```rust
    ListPatch {
        list: String,
        key: String,
        data: Result<serde_json::Value, String>,
    },
```

Add the constructor on `impl SilcrowEvent` (after the `custom` constructor):

```rust
    /// Sends a keyed list-row patch. `data` contains only the changed fields.
    /// Client targets `[data-pilcrow-list="list"][data-pilcrow-key="key"]`.
    pub fn list_patch(
        list: impl Into<String>,
        key: impl Into<String>,
        data: impl serde::Serialize,
    ) -> Self {
        Self {
            kind: EventKind::ListPatch {
                list: list.into(),
                key: key.into(),
                data: serde_json::to_value(data).map_err(|e| e.to_string()),
            },
            id: None,
        }
    }
```

Update `serialize_check` to match the new variant:

```rust
    fn serialize_check(&self) -> Result<(), String> {
        match &self.kind {
            EventKind::Patch { data, .. }
            | EventKind::Custom { data, .. }
            | EventKind::ListPatch { data, .. } => data.as_ref().map(|_| ()).map_err(Clone::clone),
            _ => Ok(()),
        }
    }
```

Add serialisation to `impl From<SilcrowEvent> for Event` (after the `Custom` arm):

```rust
            EventKind::ListPatch { list, key, data } => match data {
                Err(e) => {
                    tracing::warn!("SilcrowEvent::list_patch dropped — serialization failed: {e}");
                    Event::default().comment("pilcrow:serialize_error")
                }
                Ok(mut payload) => {
                    // Merge list and key into the payload object so the client
                    // receives a single flat object: { list, key, ...changed_fields }
                    if let serde_json::Value::Object(ref mut map) = payload {
                        map.insert("list".to_string(), serde_json::Value::String(list));
                        map.insert("key".to_string(), serde_json::Value::String(key));
                    }
                    apply_id(
                        Event::default()
                            .event("list-patch")
                            .json_data(payload)
                            .unwrap_or_else(|_| Event::default().comment("pilcrow:encode_error")),
                        id,
                    )
                }
            },
```

- [ ] **Step 3: Run the tests to verify they pass**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1 | tail -8
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git -C /home/user/pilcrow-silcrow add pilcrow/crates/runtime/src/sse/server_sent_events.rs
git -C /home/user/pilcrow-silcrow commit -m "feat(sse): add ListPatch event kind and SilcrowEvent::list_patch constructor"
```

---

## Task 2: Change codegen to emit `<div data-pilcrow-live>` instead of inline script

**Files:**
- Modify: `pilcrow/crates/routekit/src/templating/codegen/app_module.rs`
- Modify: `pilcrow/crates/routekit/src/templating/codegen/tests.rs`

### Background

The current codegen block at lines ~782–814 of `app_module.rs` does two things:

1. Injects `__LIVE_SHIM` — a `<script>` that defines `window.__pilcrow_live_patch` — before `</head>`.
2. Injects an inline `<script>` that opens an `EventSource` and calls `window.__pilcrow_live_patch` — before `</body>`.

After this task, the shim injection is removed entirely, and the anchor becomes:

```html
<div data-pilcrow-live="/__pilcrow/live/actual/path" style="display:none"></div>
```

injected before `</body>`. Silcrow's `initLiveElements()` (modified in Task 3) will discover this element and manage the SSE connection.

- [ ] **Step 1: Write the failing test**

Add to `pilcrow/crates/routekit/src/templating/codegen/tests.rs`:

```rust
#[test]
fn live_anchor_emits_data_pilcrow_live_element() {
    // We verify that generated GET handler source contains data-pilcrow-live
    // and does NOT contain the old __LIVE_SHIM or inline EventSource script.
    use crate::templating::codegen::app_module::emit_app_module;
    use crate::templating::codegen::types::TemplateCodegenInput;
    use crate::templating::pipeline::PageModuleEntry;
    use std::collections::HashMap;

    let entry = PageModuleEntry {
        symbol: "page_index".to_string(),
        pattern: "/".to_string(),
        html_template: "<html><head></head><body><h1>hi</h1></body></html>".to_string(),
        ..Default::default()
    };
    let mut live_fields_map: HashMap<String, Vec<String>> = HashMap::new();
    live_fields_map.insert("page_index".to_string(), vec!["count".to_string()]);

    let input = TemplateCodegenInput {
        page_entries: &[entry],
        live_fields_map: &live_fields_map,
        ..TemplateCodegenInput::test_default()
    };
    let source = emit_app_module(&input);

    assert!(
        source.contains("data-pilcrow-live"),
        "anchor element missing in:\n{source}"
    );
    assert!(
        !source.contains("__LIVE_SHIM"),
        "__LIVE_SHIM should not appear in:\n{source}"
    );
    assert!(
        !source.contains("window.__pilcrow_live_patch"),
        "old shim fn should not appear in:\n{source}"
    );
}
```

Run: `cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit templating::codegen::tests::live_anchor_emits 2>&1`
Expected: FAIL — `data-pilcrow-live` not found.

> **Note on `TemplateCodegenInput::test_default()`:** If this helper doesn't exist, look for how other tests construct `TemplateCodegenInput` in the same file — `tests.rs` may use a different helper. Match whatever pattern is already used there rather than creating a new one.

- [ ] **Step 2: Replace the shim + inline script block**

In `app_module.rs`, find the block that starts at `if !live_fields.is_empty() && !has_fsr {` (around line 782) and replace the entire block:

**Remove this block** (lines ~783–815):

```rust
                if !live_fields.is_empty() && !has_fsr {
                    let live_shim_str = "window.__pilcrow_live_patch=function(data){...}";
                    let live_shim_tag = format!("<script>{live_shim_str}</script>");
                    let live_shim_lit = rust_string(&live_shim_tag);
                    let _ = writeln!(out, "            const __LIVE_SHIM: &str = {live_shim_lit};");
                    // ... (entire shim + anchor injection, ~30 lines total)
                }
```

**Replace with:**

```rust
                // ── Live props: inject data-pilcrow-live anchor ───────────────
                // Silcrow's initLiveElements() discovers [data-pilcrow-live] and
                // manages the SSE connection and live-event patching.
                if !live_fields.is_empty() && !has_fsr {
                    out.push_str("            let html = {\n");
                    out.push_str("                let __live_anchor = format!(\"<div data-pilcrow-live=\\\"/__pilcrow/live{}\\\" style=\\\"display:none\\\"></div>\", __live_path);\n");
                    out.push_str("                if let Some(__pos) = html.rfind(\"</body>\") {\n");
                    out.push_str("                    let mut __s = String::with_capacity(html.len() + __live_anchor.len());\n");
                    out.push_str("                    __s.push_str(&html[..__pos]);\n");
                    out.push_str("                    __s.push_str(&__live_anchor);\n");
                    out.push_str("                    __s.push_str(&html[__pos..]);\n");
                    out.push_str("                    __s\n");
                    out.push_str("                } else {\n");
                    out.push_str("                    format!(\"{}{}\", html, __live_anchor)\n");
                    out.push_str("                }\n");
                    out.push_str("            };\n");
                }
```

- [ ] **Step 3: Verify the test passes and the full suite is green**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit 2>&1 | tail -8
```

Expected: `189 passed` (or more) — all tests pass, none fail.

- [ ] **Step 4: Commit**

```bash
git -C /home/user/pilcrow-silcrow add pilcrow/crates/routekit/src/templating/codegen/app_module.rs \
    pilcrow/crates/routekit/src/templating/codegen/tests.rs
git -C /home/user/pilcrow-silcrow commit -m "feat(codegen): replace inline LiveProp SSE script with data-pilcrow-live anchor element"
```

---

## Task 3: Update `silcrow.js` — anchor discovery, live/list-patch handlers, cleanup

**Files:**
- Modify: `silcrow/src/silcrow.js`

There are four independent sub-changes in this task; apply them all before running the build.

### 3a — `initLiveElements()`: add `[data-pilcrow-live]` discovery

Find `initLiveElements()` (around line 1374). The function currently scans `[s-sse]` and `[s-ws]`/`[s-wss]`. Add a third block **before** the WebSocket block:

```javascript
function initLiveElements() {
  // 1. Pilcrow-injected live prop anchor (auto-injected by codegen)
  document.querySelectorAll("[data-pilcrow-live]").forEach(el => {
    const url = el.getAttribute("data-pilcrow-live");
    if (url) openLive(el, url);
  });

  // 2. Server-Sent Events (SSE)
  document.querySelectorAll("[s-sse]").forEach(el => {
    const url = el.getAttribute("s-sse");
    if (url) openLive(el, url);
  });

  // 3. WebSockets (WS/WSS)
  document.querySelectorAll("[s-ws], [s-wss]").forEach(el => {
    const url = el.getAttribute("s-ws") || el.getAttribute("s-wss");
    if (url) openWsLive(el, url);
  });
}
```

### 3b — `connectSseHub()`: add `live` and `list-patch` event listeners

Inside `connectSseHub()`, after the existing `es.addEventListener("custom", ...)` block and before `es.onerror`, add:

```javascript
  es.addEventListener("live", function (e) {
    try {
      const data = JSON.parse(e.data);
      if (!data || typeof data !== "object" || Array.isArray(data)) return;
      Object.keys(data).forEach(function (k) {
        const v = data[k];
        document.querySelectorAll('[data-pilcrow-live-field="' + k + '"]').forEach(function (n) {
          n.textContent = v == null ? "" : String(v);
        });
      });
    } catch (err) {
      warn("Failed to parse SSE live event: " + err.message);
    }
  });

  es.addEventListener("list-patch", function (e) {
    try {
      const payload = JSON.parse(e.data);
      if (!payload || typeof payload !== "object") return;
      const listName = payload.list;
      const key = String(payload.key);
      if (!listName || payload.key == null) return;
      const container = document.querySelector(
        '[data-pilcrow-list="' + CSS.escape(listName) + '"]'
      );
      if (!container) return;
      const row = container.querySelector(
        '[data-pilcrow-key="' + CSS.escape(key) + '"]'
      );
      if (!row) return;
      const changes = Object.assign({}, payload);
      delete changes.list;
      delete changes.key;
      patch(changes, row);
    } catch (err) {
      warn("Failed to parse SSE list-patch event: " + err.message);
    }
  });
```

### 3c — Post-fragment-swap re-scan: include `[data-pilcrow-live]`

Find the post-swap `[s-sse]` re-scan in `finalizeNavigation` (around line 2026):

```javascript
  if (targetEl) {
    targetEl.querySelectorAll("[s-sse]").forEach(function (el) {
      const url = el.getAttribute("s-sse");
      if (url) openLive(el, url);
    });
  }
```

Replace with:

```javascript
  if (targetEl) {
    targetEl.querySelectorAll("[data-pilcrow-live]").forEach(function (el) {
      const url = el.getAttribute("data-pilcrow-live");
      if (url) openLive(el, url);
    });
    targetEl.querySelectorAll("[s-sse]").forEach(function (el) {
      const url = el.getAttribute("s-sse");
      if (url) openLive(el, url);
    });
  }
```

### 3d — MutationObserver cleanup: extend child selector

In the `liveObserver` MutationObserver callback (around line 2591), the cleanup selector is:

```javascript
for (const child of removed.querySelectorAll("[s-sse], [s-ws], [s-wss]")) {
```

Extend it to include `[data-pilcrow-live]`:

```javascript
for (const child of removed.querySelectorAll("[data-pilcrow-live], [s-sse], [s-ws], [s-wss]")) {
```

- [ ] **Step 1: Apply all four sub-changes (3a, 3b, 3c, 3d) to `silcrow/src/silcrow.js`**

Make all four edits described above.

- [ ] **Step 2: Build silcrow and verify embed**

```bash
cd /home/user/pilcrow-silcrow/silcrow && node build.js 2>&1
```

Expected: outputs `✓ wrote dist/silcrow.min.js` and `✓ copied to ../pilcrow/crates/runtime/assets/silcrow.js` (or similar success lines).

```bash
cargo build --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-runtime 2>&1 | tail -4
```

Expected: `Finished dev profile`.

- [ ] **Step 3: Verify key symbols in the built output**

```bash
grep -c "data-pilcrow-live\|list-patch\|data-pilcrow-list\|data-pilcrow-key" \
    /home/user/pilcrow-silcrow/silcrow/dist/silcrow.min.js
```

Expected: `1` (all on one line but grep -c counts line occurrences; any non-zero value is fine).

```bash
grep "data-pilcrow-live\|list-patch" /home/user/pilcrow-silcrow/silcrow/dist/silcrow.min.js | wc -c
```

Expected: > 0

- [ ] **Step 4: Run routekit tests to confirm codegen still passes**

```bash
cargo test --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-routekit 2>&1 | tail -4
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cd /home/user/pilcrow-silcrow
git add silcrow/src/silcrow.js silcrow/dist/silcrow.js silcrow/dist/silcrow.min.js \
    pilcrow/crates/runtime/assets/silcrow.js
git commit -m "feat(silcrow): add data-pilcrow-live anchor discovery, live/list-patch SSE handlers"
```

---

## Task 4: Update `silcrow-api.md` documentation

**Files:**
- Modify: `silcrow/docs/silcrow-api.md`

- [ ] **Step 1: Add `data-pilcrow-live`, `data-pilcrow-list`, `data-pilcrow-key` to the Live Connections section**

Find the **Live Connections** subsection under `## HTML Attributes (Directives)`:

```markdown
**Live Connections**
- `s-sse`
- `s-ws`
- `s-wss`
```

Replace with:

```markdown
**Live Connections**
- `s-sse`
- `s-ws`
- `s-wss`
- `data-pilcrow-live="<url>"` — auto-injected by Pilcrow codegen for pages with `LiveProp<T>` fields; Silcrow discovers this element and opens a managed SSE connection to `url`. Do not write manually.
- `data-pilcrow-list="<field>"` — marks a list container; `list-patch` events target rows within this element by `data-pilcrow-key`. Auto-injected by Pilcrow codegen (Slice F).
- `data-pilcrow-key="<key>"` — marks a list row with its unique key; targeted by `list-patch` events. Auto-injected by Pilcrow codegen (Slice F).
```

- [ ] **Step 2: Add `live` and `list-patch` to the SSE Event Types section**

Find `## SSE Event Types` and add two entries:

```markdown
## SSE Event Types

- `message` (default)
- `patch` — payload: `{ target, data[, mutation_id] }`. When `mutation_id` is present, silcrow.js calls `confirmOptimistic(mutation_id)` before applying the patch.
- `html`
- `invalidate`
- `navigate`
- `custom`
- `live` — payload: flat JSON object `{ "<field>": <value>, ... }`. Patches all `[data-pilcrow-live-field="field"]` text nodes. Emitted by Pilcrow's per-page SSE route (`/__pilcrow/live{pattern}`) for `LiveProp<T>` fields.
- `list-patch` — payload: `{ "list": "<field>", "key": "<row_key>", "<changed_field>": <value>, ... }`. Silcrow finds `[data-pilcrow-list="field"]` then `[data-pilcrow-key="row_key"]` within it, and calls `patch(changes, row)`.
```

- [ ] **Step 3: Commit**

```bash
git -C /home/user/pilcrow-silcrow add silcrow/docs/silcrow-api.md
git -C /home/user/pilcrow-silcrow commit -m "docs(silcrow-api): document data-pilcrow-live anchor, live/list-patch SSE events"
```

---

## Task 5: Update `registry.toml` and `implementation-progress.md`

**Files:**
- Modify: `pilcrow/registry.toml`
- Modify: `plans/implementation-progress.md`

- [ ] **Step 1: Add feature entries to `registry.toml`**

Append to `pilcrow/registry.toml`:

```toml
[[features]]
id = "sse-anchor"
name = "data-pilcrow-live SSE Anchor"
domain = "silcrow"
status = "stable"
summary = "Codegen injects a <div data-pilcrow-live> element for pages with LiveProp fields. Silcrow discovers it via initLiveElements(), manages the SSE connection through the existing hub infrastructure, and patches [data-pilcrow-live-field] text nodes on 'live' events."
spec = """
Pages with LiveProp<T> fields get a <div data-pilcrow-live="/__pilcrow/live{path}" style="display:none">
injected before </body> at render time. This replaces the former inline EventSource <script> + __LIVE_SHIM.

Silcrow's initLiveElements() scans for [data-pilcrow-live] on page load and after every fragment swap,
calling openLive(el, url) which reuses the SSE hub pool (one connection per URL, shared across
subscribers). The 'live' event listener in connectSseHub() patches [data-pilcrow-live-field="name"]
text nodes whenever the server sends a live event.

The MutationObserver cleanup extended to include [data-pilcrow-live] in its child-selector sweep,
so the SSE hub is released when the element is removed from the DOM.
"""
constraints = [
  "data-pilcrow-live is auto-injected by codegen; do not write it manually in templates.",
  "The element is always display:none — it is a connection anchor, not a visible UI element.",
  "Fragment navigation re-scans the swapped subtree for [data-pilcrow-live] after each swap.",
]
source_refs = [
  "crates/routekit/src/templating/codegen/app_module.rs — live props anchor injection block",
  "crates/runtime/assets/silcrow.js — initLiveElements, connectSseHub live listener",
]
test_refs = [
  "crates/routekit/src/templating/codegen/tests.rs — live_anchor_emits_data_pilcrow_live_element",
]
scaffold_templates = []

[[features]]
id = "list-patch-wire-format"
name = "Keyed List Patch Wire Format"
domain = "pilcrow"
status = "stable"
summary = "SilcrowEvent::list_patch(list, key, data) emits event: list-patch with {list, key, ...changed_fields}. Client targets [data-pilcrow-list] then [data-pilcrow-key] within it and calls patch(changes, row)."
spec = """
Wire format:
  event: list-patch
  data: { "list": "<field_name>", "key": "<row_key>", "<changed>": <value>, ... }

Server: SilcrowEvent::list_patch("tickets", "ticket:42", serde_json::json!({"status": "open"}))

Client (connectSseHub list-patch listener):
  1. Find container: document.querySelector('[data-pilcrow-list="tickets"]')
  2. Find row: container.querySelector('[data-pilcrow-key="ticket:42"]')
  3. Apply: patch({status: "open"}, row)

The data-pilcrow-list and data-pilcrow-key attributes are auto-injected by codegen (Slice F).
Until Slice F ships, developers can manually add these attributes for server-pushed list updates.
"""
constraints = [
  "list and key fields are stripped from the payload before passing to patch(); only changed_fields remain.",
  "If the container or row is not found in the DOM the event is silently dropped.",
  "key is always coerced to String before CSS.escape().",
]
source_refs = [
  "crates/runtime/src/sse/server_sent_events.rs — EventKind::ListPatch, SilcrowEvent::list_patch",
  "crates/runtime/assets/silcrow.js — connectSseHub list-patch listener",
]
test_refs = [
  "crates/runtime/src/sse/server_sent_events.rs — list_patch_serialises_with_all_fields, list_patch_bad_key_type_still_serialises",
]
scaffold_templates = []
```

- [ ] **Step 2: Mark TODOs #11 and #12 as done in `implementation-progress.md`**

Find the two pending rows and update them:

```markdown
| 11 | One SSE per page enforced — one `data-pilcrow-live` anchor per page; all producers merge via `select_all` | ✅ Done (Slice E) |
| 12 | Keyed list patch wire format — `{ list, key, ...changed_fields }` SSE message; client targets `data-pilcrow-key` rows | ✅ Done (Slice E) |
```

Add a new section in the progress file after the Slice D section:

```markdown
### Slice E — SSE anchor + keyed list wire format  ✅ DONE

**TODOs #11–#12**: `data-pilcrow-live` DOM anchor replaces inline LiveProp SSE script; `list-patch` SSE event type added.

| Layer | File | What changed |
|-------|------|--------------|
| Rust | `pilcrow/crates/runtime/src/sse/server_sent_events.rs` | `EventKind::ListPatch`; `SilcrowEvent::list_patch(list, key, data)`; wire: `event: list-patch` + `{list, key, ...fields}` |
| Codegen | `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | Removed `__LIVE_SHIM` + inline EventSource script; injects `<div data-pilcrow-live="/__pilcrow/live{path}" style="display:none">` before `</body>` |
| JS | `silcrow/src/silcrow.js` | `initLiveElements()` scans `[data-pilcrow-live]`; `connectSseHub()` adds `live` + `list-patch` listeners; post-swap re-scan + MutationObserver cleanup extended |
| Docs | `silcrow/docs/silcrow-api.md` | `data-pilcrow-live`, `data-pilcrow-list`, `data-pilcrow-key` attributes; `live` and `list-patch` SSE events |
| Registry | `pilcrow/registry.toml` | `sse-anchor` + `list-patch-wire-format` feature entries |
```

Also update the Slice Plan summary line:

```markdown
Slice E  (done)   — #11, #12  One SSE per page + keyed list wire format
```

- [ ] **Step 3: Commit**

```bash
git -C /home/user/pilcrow-silcrow add pilcrow/registry.toml plans/implementation-progress.md
git -C /home/user/pilcrow-silcrow commit -m "chore: update registry and progress for Slice E (SSE anchor + list-patch wire format)"
```

---

## Task 6: Full build chain verification + MCP tests

- [ ] **Step 1: Run the full Silcrow → Pilcrow build chain**

```bash
cd /home/user/pilcrow-silcrow/silcrow && node build.js 2>&1
cargo build --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-routekit 2>&1 | tail -4
cargo build --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-runtime 2>&1 | tail -4
```

Expected: each ends with `Finished`.

- [ ] **Step 2: Run all Pilcrow tests**

```bash
cargo test --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-routekit 2>&1 | tail -4
cargo test --manifest-path /home/user/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-runtime 2>&1 | tail -6
```

Expected: routekit: all pass; runtime: 0 failed.

- [ ] **Step 3: Run MCP tests (required by CLAUDE.md after any framework change)**

```bash
cargo test --manifest-path /home/user/pilcrow-silcrow/pilcrow/tools/pilcrow-mcp/Cargo.toml 2>&1 | tail -6
```

Expected: same 5 pre-existing failures (from `status = "removed"` entries written in Slice C) and no new failures. The 5 known-failing tests are:
- `docs::tests::delegates_exact_silcrow_runtime_questions`
- `docs::tests::explains_registry_feature_with_evidence`
- `registry::tests::parses_registry_and_filters_status`
- `workspace::tests::default_scan_finds_sandbox_routes_and_versions`
- `workspace::tests::route_graph_includes_url_patterns`

If the count increases, investigate before continuing.

- [ ] **Step 4: Push to remote**

```bash
git -C /home/user/pilcrow-silcrow push -u origin claude/slice-d-layout-patterns-U5Yix 2>&1
```

Expected: `Everything up-to-date` or `claude/slice-d-layout-patterns-U5Yix -> claude/slice-d-layout-patterns-U5Yix`.

---

## Self-review

**Spec coverage:**
- ✅ TODO #11 — `data-pilcrow-live` anchor: Tasks 2 (codegen), 3 (Silcrow)
- ✅ TODO #11 — `select_all` merge: already in `deferred.rs:28` — no change needed
- ✅ TODO #12 — `list-patch` wire format: Task 1 (Rust), Task 3b (JS handler)
- ✅ `data-pilcrow-list` / `data-pilcrow-key` targeting: Task 3b
- ✅ Docs: Task 4
- ✅ Registry: Task 5
- ✅ Tests: Task 1 (Rust unit tests), Task 2 (codegen test)

**Placeholder scan:** No TBDs, no "handle edge cases" vagueness. All code blocks are complete.

**Type consistency:**
- `SilcrowEvent::list_patch` takes `impl Into<String>` for both `list` and `key` — consistent with `SilcrowEvent::patch` signature pattern.
- `EventKind::ListPatch` uses same `data: Result<serde_json::Value, String>` as `Patch` and `Custom` — consistent.
- JS `patch(changes, row)` — `patch` is an existing function called throughout silcrow.js — consistent.
