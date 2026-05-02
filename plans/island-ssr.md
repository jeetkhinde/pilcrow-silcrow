# Island SSR Plan

Server-render React island HTML so the initial HTTP response contains real markup,
then hydrate on the client. Closes the gap between Silcrow DOM patching (HTML exists
from first byte) and the current React islands (empty `<div>` until JS runs).

---

## Current state

```
cargo build
  └─ react.rs → Vite → {id}-{hash}.js (createRoot, CSR only)
                      → generated_react_assets.rs (bytes embedded in binary)

Request time (Rust)
  └─ Askama renders: <div data-pilcrow-react data-id="..." data-src="..." ...></div>
                      ↑ always empty

Browser
  └─ react-islands.js → import({src}) → createRoot(el).render(<Component {...props} />)
```

Props are Rust values resolved at request time (e.g. `product-id="{{ product.id }}"`),
so SSR cannot be run at `cargo build` time — it must run when the Rust handler executes.

---

## Architecture decision

Two modes, opt-in per island:

| Mode | `strategy` attribute | When SSR runs | Node required at runtime |
|---|---|---|---|
| **Shell** | `strategy="shell"` | Build time, with default props | No |
| **SSR** | `strategy="ssr"` | Request / ISR cache time | Yes |

Existing `load` / `idle` / `visible` strategies remain CSR.
`shell` and `ssr` both use `hydrateRoot` on the client; `shell` emits static markup,
`ssr` emits markup rendered with the actual props.

---

## Phase 1 — Dual Vite bundle

**Goal:** produce a second SSR bundle alongside the existing client bundle.

### `pilcrow/crates/routekit/src/templating/react.rs`

**`render_entry_wrapper`** — add a second SSR entry file per island:

```typescript
// {id}.client.tsx  (existing, updated to support hydrateRoot)
import React from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import Component from "{src}";

export function mount(el, props) {
  if (el.__pilcrowReactRoot) {
    el.__pilcrowReactRoot.render(React.createElement(Component, props));
    return;
  }
  if (el.children.length > 0) {
    // Server rendered HTML present — hydrate instead of replacing
    hydrateRoot(el, React.createElement(Component, props));
    return;
  }
  const root = createRoot(el);
  el.__pilcrowReactRoot = root;
  root.render(React.createElement(Component, props));
}

window.__pilcrowReactMounts = window.__pilcrowReactMounts || {};
window.__pilcrowReactMounts[{id_json}] = mount;
```

```typescript
// {id}.server.tsx  (new)
import React from "react";
import { renderToString } from "react-dom/server";
import Component from "{src}";

export function render(props) {
  return renderToString(React.createElement(Component, props));
}
```

**`render_vite_config`** — add a second call to `run_vite` with an SSR config:

```javascript
// vite.ssr.config.mjs  (new, generated alongside vite.config.mjs)
export default {
  root: process.cwd(),
  resolve: { /* same aliases */ },
  esbuild: { jsx: "automatic" },
  build: {
    ssr: true,
    outDir: "{dist_ssr_dir}",
    emptyOutDir: true,
    rollupOptions: {
      input: { "{id}": "{id}.server.tsx", ... },
      output: {
        entryFileNames: "[name].ssr.js",
        format: "esm"
      }
    }
  }
};
```

**`build_react_assets`** — after the existing Vite client build, call `run_vite` again
with the SSR config. Collect the `dist_ssr/` paths and write a second generated module:

```
generated_react_ssr.rs   (new, parallel to generated_react_assets.rs)

pub fn ssr_bundle(id: &str) -> Option<&'static str> {
  match id {
    "counter_0abc" => Some(include_str!("/path/to/dist_ssr/counter_0abc.ssr.js")),
    _ => None,
  }
}
```

SSR bundles are embedded as `&'static str` (not bytes) because Rust needs to pass them
as JavaScript source to Node. They are NOT served to the browser.

**Files changed:**
- `pilcrow/crates/routekit/src/templating/react.rs`
  - `render_entry_wrapper` → generates both `.client.tsx` and `.server.tsx`
  - `render_vite_config` → renamed to `render_client_vite_config`; new `render_ssr_vite_config`
  - `build_react_assets` → second `run_vite` call; new `write_react_ssr_module`

---

## Phase 2 — `hydrateRoot` in `react-islands.js`

**Goal:** when the div already has children (server rendered), hydrate instead of replace.

### `pilcrow/crates/runtime/assets/react-islands.js`

The `mount` function already delegates to a `mountFn` imported from the island bundle.
The bundle's `mount` export (updated in Phase 1) already handles the `hydrateRoot` check.
No change needed to `react-islands.js` itself — it calls `mountFn(el, readProps(el))`
and the bundle decides whether to `createRoot` or `hydrateRoot`.

**Files changed:** none (the bundle handles it).

---

## Phase 3 — Build-time static shell (`strategy="shell"`)

**Goal:** produce a static HTML shell from `renderToString(Component, {})` at build time
and embed it in the Askama template so the `<div>` is never empty in the response,
even without Node at runtime.

### `pilcrow/crates/routekit/src/templating/react.rs`

After the SSR Vite build (Phase 1), for each island that has at least one `shell`
strategy usage, call Node to render the default shell:

```rust
fn render_static_shell(id: &str, ssr_bundle_path: &Path, manifest_dir: &Path) -> io::Result<String> {
    let script = format!(
        r#"import {{ render }} from "{}";
process.stdout.write(render({{}}));"#,
        ssr_bundle_path.to_string_lossy().replace('\\', "/")
    );
    let output = Command::new("node")
        .arg("--input-type=module")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .current_dir(manifest_dir)
        .spawn()?
        .wait_with_output_from_stdin(script.as_bytes())?;
    // ... error handling ...
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

The shell is stored in a new generated module:

```
generated_react_shells.rs

pub fn shell(id: &str) -> Option<&'static str> {
  match id {
    "add_to_cart_0abc" => Some("<button type=\"submit\">Add to cart</button>"),
    _ => None,
  }
}
```

### `pilcrow/crates/routekit/src/templating/react.rs` — `parse_react_tag`

When `strategy == "shell"`, embed the shell into the `<div>` placeholder:

```rust
// Current:
html.push_str("></div>");

// With shell strategy:
if strategy == "shell" {
    html.push_str(&format!(
        ">{REACT_SHELL_PLACEHOLDER_PREFIX}{id}__</div>"
    ));
} else {
    html.push_str("></div>");
}
```

### `pilcrow/crates/routekit/src/templating/pipeline.rs`

Add `replace_react_shell_placeholders` pass after the existing placeholder pass:
replaces `__PILCROW_REACT_SHELL_{id}__` with the shell HTML from `generated_react_shells.rs`.

**New constant:** `pub const REACT_SHELL_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_SHELL_";`

**Files changed:**
- `react.rs` — `parse_react_tag`, new `render_static_shell`, new `write_react_shells_module`
- `pipeline.rs` — new shell placeholder pass
- new file: `generated_react_shells.rs` (generated at build time into `OUT_DIR`)

---

## Phase 4 — Runtime Node worker for full SSR (`strategy="ssr"`)

**Goal:** render islands with the actual Rust props at request time. Required for dynamic
content where the shell would show the wrong value until hydration.

### Node worker protocol

Pilcrow spawns a single persistent Node process on startup. Worker reads
newline-delimited JSON from stdin, writes newline-delimited JSON to stdout:

```
stdin:  {"id":"counter_0abc","props":{"initialCount":"3"}}\n
stdout: {"html":"<div>...</div>"}\n

stdin:  {"id":"counter_0abc","props":{"initialCount":"7"}}\n
stdout: {"html":"<div>...</div>"}\n
```

The worker script is generated at build time and written to `OUT_DIR/island_ssr_worker.js`.
It imports all SSR bundles and dispatches on `id`:

```javascript
// island_ssr_worker.js  (generated)
import { render as render_counter_0abc } from "./dist_ssr/counter_0abc.ssr.js";
// ...
const dispatch = {
  "counter_0abc": render_counter_0abc,
  // ...
};
import { createInterface } from "node:readline";
const rl = createInterface({ input: process.stdin });
rl.on("line", (line) => {
  const { id, props } = JSON.parse(line);
  const fn = dispatch[id];
  const html = fn ? fn(props) : "";
  process.stdout.write(JSON.stringify({ html }) + "\n");
});
```

### `pilcrow/crates/runtime/src/island_ssr.rs` (new)

```rust
pub struct IslandSsrWorker {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl IslandSsrWorker {
    pub fn spawn(worker_js_path: &Path, manifest_dir: &Path) -> io::Result<Self> { ... }
    pub fn render(&mut self, id: &str, props: &serde_json::Value) -> io::Result<String> { ... }
}
```

A single `Arc<Mutex<IslandSsrWorker>>` is held in the Pilcrow app state, created in `start.rs`.

### Template integration

When `strategy == "ssr"`, `parse_react_tag` emits a new placeholder:

```
__PILCROW_REACT_SSR_{id}_PROPS_{props_json_b64}__
```

The pipeline's placeholder replacement pass (at request time, not build time) resolves
this by calling `IslandSsrWorker::render(id, props)` and inlining the returned HTML.

Since this is request-time work, the replacement happens in the Axum handler, not in
the build-time `replace_react_placeholders` function.

### ISR + SSG integration

For ISR pages: the SSR call happens during the cache-population render (first request
after TTL expiry). The rendered HTML is stored in the ISR cache alongside the page HTML.

For SSG: `cargo build` spawns the worker and renders all SSR islands with their static
props. No runtime Node dependency for SSG output.

**Files changed:**
- `react.rs` — `parse_react_tag` (`ssr` strategy branch), new `write_island_ssr_worker`
- new file: `pilcrow/crates/runtime/src/island_ssr.rs`
- `pilcrow/crates/runtime/src/start.rs` — spawn worker, add to app state
- `pipeline.rs` — SSR placeholder handling at request time

---

## Phase 5 — Streaming SSR (`strategy="ssr:stream"`)

**Goal:** pipe `renderToPipeableStream` output into Pilcrow's existing SSR streaming
response so the island HTML arrives inline with the page stream.

This phase depends on Phase 4. The Node worker gains a streaming mode:

```
stdin:  {"id":"...","props":{...},"stream":true}\n
stdout: {"chunk":"<div"}\n{"chunk":" data-"}\n...{"end":true}\n
```

Pilcrow's Axum handler pipes chunks into the response body as they arrive.

This is the last phase and only matters for large, slow-to-render island components.
Most islands render in < 1ms and don't need streaming.

**Files changed:**
- `island_ssr.rs` — streaming render method
- Axum handler layer for `strategy="ssr:stream"` islands

---

## Implementation order

```
Phase 1  Dual Vite bundle + generated_react_ssr.rs
Phase 2  (free — handled by Phase 1 bundle change)
Phase 3  Build-time shell: strategy="shell", generated_react_shells.rs
Phase 4  Runtime Node worker: strategy="ssr", island_ssr.rs
Phase 5  Streaming: strategy="ssr:stream"
```

Phase 1 and Phase 3 have no runtime dependency on Node — safe to ship first.
Phase 4 adds Node as a production requirement and should be gated behind a
`[client.react] ssr = true` flag in `Pilcrow.toml`.

---

## Developer-facing API summary

```html
<!-- CSR (current, unchanged) -->
<react src="/react/Counter.tsx" strategy="load" initial-count="{{ count }}" />
<react src="/react/Counter.tsx" strategy="idle" initial-count="{{ count }}" />
<react src="/react/Counter.tsx" strategy="visible" initial-count="{{ count }}" />

<!-- Static shell: HTML in response, renders with default props, hydrates with real props -->
<react src="/react/Counter.tsx" strategy="shell" initial-count="{{ count }}" />

<!-- Full SSR: HTML in response, rendered with actual props, hydrates seamlessly -->
<react src="/react/Counter.tsx" strategy="ssr" initial-count="{{ count }}" />

<!-- Streaming SSR: island HTML streams inline with page -->
<react src="/react/Counter.tsx" strategy="ssr:stream" initial-count="{{ count }}" />
```

`Pilcrow.toml` opt-in:

```toml
[client.react]
enabled = true
ssr = false          # set to true to allow strategy="ssr" and strategy="ssr:stream"
node_bin = "node"    # path to Node binary if not on PATH
```
