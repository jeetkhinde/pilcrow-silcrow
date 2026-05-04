# Pilcrow Rendering Models

## Quick reference

| Mode | Trigger | Cache | Streaming | Node required |
|------|---------|-------|-----------|---------------|
| SSR (default) | — | none | no | no |
| ISR | `REVALIDATE: u64` | yes, TTL-based | no | no |
| SSG | `PRERENDER: bool` | yes, forever (u64::MAX TTL) | no | no |
| SSG + ISR | `PRERENDER` + `REVALIDATE` | yes, ISR TTL | no | no |
| SSR Streaming | `STREAMING: bool` | none | page-level | no |
| Deferred fields | `Deferred<T>` / `DeferredHtml` | compatible with ISR/SSG | field-level | no |
| Live Props | `LiveProp<T>` field in Props | none (always live) | SSE persistent | no |
| Pilcrow Islands | `<island>` tag | none (per-request fragment fetch) | no | no |
| React Islands (CSR) | `<react strategy="load/idle/visible">` | browser SW | no | no |
| React Islands (SSR) | `<react strategy="shell/ssr">` | browser SW | no | yes (ssr only) |

---

## 1. SSR — Server-Side Rendering (default)

`load()` runs synchronously on every request. No special constant needed.
A page with no `.rs` file renders the HTML template directly with no data loading.

```rust
// code-behind — no special constants; this is the default
pub async fn load(req: Req) -> AppResult<Props> { … }
```

**Key files**
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — standard handler emit
- `pilcrow/crates/routekit/src/templating/page_options.rs` — `PageOptions` struct

---

## 2. ISR — Incremental SSR

Stale-while-revalidate page caching. Three cache states: Fresh → serve immediately, Stale → serve + background revalidate, Miss → blocking render + store.

```rust
// code-behind
pub const REVALIDATE: u64 = 60;                      // required: cache TTL in seconds
pub const MAX_STALE: u64 = 3600;                     // optional: serve stale for up to this many seconds beyond TTL
pub const CACHE_TAGS: &[&str] = &["products"];        // optional: group invalidation tag
pub const CACHE_VARY: &[&str] = &["session", "accept-language"]; // optional: vary cache key by cookie (checked first) or header
```

All constants are stripped from emitted code — they never appear in generated Rust.

### Cache key format

```
{path}?{sorted_query_params}#{vary_hash}
```

Empty segments omitted. Missing CACHE_VARY keys produce an empty string segment so the key stays stable.

### Cache states

| State | Condition | Response |
|-------|-----------|----------|
| Miss | No entry exists | Blocking render, store result |
| Fresh | `age <= REVALIDATE` | Serve cached HTML immediately |
| Stale | `age > REVALIDATE` and `age - REVALIDATE <= MAX_STALE` | Serve cached HTML, revalidate in background |
| Expired | `age > REVALIDATE + MAX_STALE` | Blocking render, store result |

If `MAX_STALE` is absent, stale entries are served indefinitely until background revalidation completes.

### Invalidation API (from action handlers)

```rust
req.cache.revalidate("/products/1");           // by path prefix
req.cache.revalidate_tag("products");          // by tag
req.res.bypass_cache();                        // skip cache read AND write (preview/admin mode)
```

### Cache provider — `Pilcrow.toml`

```toml
[cache]
provider = "memory"            # memory | filesystem | sqlite | redis (redis = not yet implemented)
dir = ".pilcrow-cache"         # filesystem provider only
path = ""                      # sqlite provider only
url = ""                       # redis provider only
revalidate_timeout_secs = 30   # max seconds a background revalidation task may run
```

### Constraints

- Cannot combine with `STREAMING = true`
- Pages with `Deferred<T>` / `DeferredHtml` fields fall through to normal SSR (ISR is not applied)
- Requires a `load()` fn — static pages (no `.rs` file) are served directly without caching

**Key files**
- `pilcrow/crates/runtime/src/isr.rs` — `IsrCache`, `IsrCacheState`, background revalidation
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — `emit_isr_handler`
- `pilcrow/crates/routekit/src/templating/page_options.rs` — `IsrOpts`
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — parses `REVALIDATE`, `MAX_STALE`, `CACHE_TAGS`, `CACHE_VARY`

---

## 3. SSG — Static Site Generation

Pages are pre-rendered at server startup (before connections are accepted) and stored with `u64::MAX` TTL (never expires). No-op for pages without `load()` — those are already static.

```rust
// code-behind
pub const PRERENDER: bool = true;

// Required for dynamic routes (routes with :param segments):
pub async fn entries() -> Vec<HashMap<String, String>> {
    vec![
        [("id".to_string(), "1".to_string())].into(),
        [("id".to_string(), "2".to_string())].into(),
    ]
}
```

```rust
// main.rs — must use pilcrow_start, not pilcrow_web::start
pilcrow_app!();
#[tokio::main]
async fn main() {
    pilcrow_start(pilcrow_router()).await
}
```

### Static export

```rust
pilcrow_export(dir)  // writes pre-rendered HTML files to disk for static hosting
```

### Constraints

- Dynamic routes must provide `entries()`
- Cannot combine with `STREAMING = true`
- Pages with `Deferred<T>` / `DeferredHtml` fields: `PRERENDER` is a no-op

**Key files**
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — `emit_ssg_handler`, `emit_prerender_all`
- `pilcrow/crates/routekit/src/templating/page_options.rs` — `SsgOpts`
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — parses `PRERENDER`, `entries()`
- `pilcrow/crates/web/src/lib.rs` — `pilcrow_start()`, `pilcrow_export()`

---

## 4. Combined PRERENDER + REVALIDATE (startup-warm ISR)

Declare both to pre-render at startup **and** revalidate on a TTL. Cache is stored with the ISR TTL (not `u64::MAX`) so normal stale-while-revalidate applies after startup.

```rust
pub const PRERENDER: bool = true;
pub const REVALIDATE: u64 = 60;
pub const CACHE_TAGS: &[&str] = &["products"];
```

Recommended for pages that need fast first-load AND fresh data over time. Requires `pilcrow_start()` (same as `PRERENDER`-only). Dynamic routes still require `entries()`.

**Key files**
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — `emit_prerender_all`, `emit_isr_handler`

---

## 5. SSR Streaming

The page shell is sent immediately with default `Props`. `load()` runs concurrently; when it resolves, the real props are streamed as a single `Silcrow.patch()` call.

```rust
// code-behind
pub const STREAMING: bool = true;
```

### How it works

1. Layout `load()` functions execute sequentially first (preserves `req.locals` propagation).
2. Page shell rendered with layout data + **default** page `Props` values and flushed.
3. `window.__ps` shim injected before `</head>`: `<script>window.__ps=function(v){Silcrow.patch(v,document.body)}</script>`
4. Page `load()` spawned on `tokio::spawn`.
5. When `load()` resolves, `Props` serialized to JSON and streamed as `<script>window.__ps({...})</script>`.

### Template authoring — critical distinction

| Syntax | Shell value | Updated by stream patch? |
|--------|-------------|--------------------------|
| `:text="field"` (Silcrow binding) | Default value | **Yes** |
| `{{ field }}` (Askama interpolation) | Default value | **No** — baked into HTML |

Use `:text`, `:value`, `s-for`, etc. for all fields that should update after the shell renders.

### Constraints

- `STREAMING` without `load()` is a no-op
- Cannot combine with `REVALIDATE` (ISR)
- Cannot combine with `PRERENDER` (SSG)
- Cannot use `Deferred<T>` or `DeferredHtml` in `Props`
- Cannot call `PilcrowClient` inside `load()`
- All page `Props` fields must implement `Default` (auto-derived for no-layout pages)
- Toasts and response headers work — `__resp_handle.apply_to()` is called on the streaming response

**Key files**
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — `emit_streaming_handler`
- `pilcrow/crates/runtime/src/deferred.rs` — `__streaming_props_response`, `__serialize_page_props`

---

## 6. Deferred fields (field-level streaming)

Fine-grained streaming at the `Props` field level. Compatible with ISR and SSG. Uses a different wire protocol from page-level `STREAMING`.
Deferred responses include `silcrow-full-reload: true`, so Silcrow boosted top-level GET navigation falls back to normal browser navigation. That preserves browser-native HTML streaming and inline patch script execution instead of buffering the full response through `fetch().text()`.

### `Deferred<T>`

Wraps a scalar future. The template interpolation is rewritten into a `data-pilcrow-async-value` text target. It renders as **empty string** in the shell. Resolved value is sent as `window.__pilcrow_async_value(key, value)` and sets target `textContent` directly.

```rust
pub struct Props {
    pub title: String,       // normal — rendered in shell
    pub count: Deferred<i32>, // deferred — empty in shell, streamed later
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Hello".into(),
        count: Deferred::spawn(async { expensive_db_call().await }),
        // or: Deferred::ready(42) — already-resolved, no streaming overhead
    })
}
```

### `DeferredHtml`

Wraps an async future that returns an HTML string. The template interpolation is rewritten into a `data-pilcrow-async-html` target. Resolved value is sent as `window.__pilcrow_async_html(slot_name, html)`. Can display a loading skeleton while resolving.

```rust
pub struct Props {
    pub products: DeferredHtml,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        products: DeferredHtml::spawn(async {
            let items = db::get_products().await?;
            product_list::render(product_list::Props { items })
        })
        .with_loading("<div class='skeleton'></div>"),
    })
}
```

### Constraints

- Cannot mix with `STREAMING = true` on the same page
- Compatible with `REVALIDATE` and `PRERENDER`
- `Deferred<T>`: type `T` must implement `Display`

**Key files**
- `pilcrow/crates/runtime/src/deferred.rs` — `Deferred<T>`, `DeferredHtml`
- `pilcrow/crates/routekit/src/templating/codegen/templates.rs` — shell slot generation

---

## 7. Pilcrow Islands (`<island>` tag)

Server-driven HTML islands: Silcrow fetches a fragment from the server and injects it into the page. Not React — these are ordinary Pilcrow pages/fragments with their own `load()` and `Props`.

```html
<!-- page template -->
<island src="./counter" strategy="visible" initial="0" />
<island src="./user-card" strategy="idle" user-id="{{ props.user_id }}" />
<island src="/shared/global-nav" />
```

```rust
// pages/dashboard/islands/counter.rs  (co-located island code-behind)
pub struct Props { pub count: i32 }
pub async fn load(req: Req) -> AppResult<Props> {
    let count = req.query.get("initial").unwrap_or("0").parse().unwrap_or(0);
    Ok(Props { count })
}
```

### How it works

Each `<island>` tag compiles to a hidden `<a>` with `s-get` + `s-html` + a trigger script:

- **`load`** (default) — trigger fires immediately (`.click()`)
- **`visible`** — `IntersectionObserver` fires when container enters viewport
- **`idle`** — `requestIdleCallback` (fallback: `setTimeout 200ms`) fires when browser is free

Silcrow handles the `s-get` fetch and replaces the container div. While loading, the container gets `class="silcrow-loading" aria-busy="true"`.

### URL resolution for `src`

| src value | Resolves to |
|-----------|-------------|
| `"./name"` or `"name"` | `{page_url_base}/islands/{name}` |
| `"/absolute/path"` | Used as-is |

Extra attributes (beyond `src` and `strategy`) become query params. Values may contain Askama expressions: `count="{{ props.count }}"` → `?count=<rendered>`.

### File layout

```
pages/dashboard/islands/counter.html   # auto-discovered, no Pilcrow.toml entry needed
pages/dashboard/islands/counter.rs     # load(), actions
```

Shared cross-page islands use `[[fragments]]` in `Pilcrow.toml`.

### Constraints

- `src` must be a static string — Askama expressions in `src` are not supported
- `strategy` defaults to `load`; must be one of: `load`, `visible`, `idle`
- Island files receive no layout wrapping
- `<island>` tags inside HTML comments are ignored and preserved as comments

**Key files**
- `pilcrow/crates/routekit/src/templating/compiler.rs` — `<island>` transpilation
- `pilcrow/crates/routekit/src/templating/pipeline.rs` — co-location discovery

---

## 8. React island strategies

Configured via the `strategy` attribute on the `<react>` tag.

```html
<react src="/react/Counter.tsx" strategy="load" initial-count="{{ props.count }}" />
```

| Strategy | Type | When component mounts |
|----------|------|-----------------------|
| `load` | CSR | Bundle available |
| `idle` | CSR | `requestIdleCallback` |
| `visible` | CSR | `IntersectionObserver` |
| `shell` | SSR | Build-time shell, hydrates with real props |
| `ssr` | SSR | Request-time render with real props |

`shell` and `ssr` require `[client.react] ssr = true` in `Pilcrow.toml`.  
`ssr` additionally requires Node.js at runtime (persistent worker spawned at startup).

See [`.claude/react-island.md`](react-island.md) for full usage rules, hook reference, and decision guide.

**Required config**

```toml
[routing]
ignore_directories = ["react"]

[client.react]
enabled = true
dirs = ["react"]
```

**Key files**
- `pilcrow/crates/routekit/src/templating/react.rs` — strategy parsing, validation, Vite build
- `pilcrow/crates/runtime/src/island_ssr.js` — Node.js worker for `ssr` strategy
- `pilcrow/crates/runtime/assets/react-islands.js` — client-side island mount logic

---

## 9. Live Props (`LiveProp<T>`)

`LiveProp<T>` is an **update mode** orthogonal to the render mode (SSR/ISR/SSG). Fields typed as `LiveProp<T>` render their initial value in the SSR shell and then **auto-subscribe the client to a server-pushed SSE channel** for live updates — no React, no manual WebSocket wiring.

### DX — developer experience

Declare a `LiveProp<T>` field in `Props` and build the producer in `load()`. That's it.

```rust
use std::sync::OnceLock;
use tokio::sync::watch;
use pilcrow_web::{AppResult, LiveProp, Req};

// Global channel — shared across all connections
static COUNTER: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn counter_tx() -> &'static watch::Sender<u64> {
    COUNTER.get_or_init(|| {
        let (tx, _rx) = watch::channel(0u64);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                COUNTER.get().unwrap().send_modify(|v| *v += 1);
            }
        });
        tx
    })
}

pub struct Props {
    pub label: String,          // normal SSR field
    pub count: LiveProp<u64>,   // opt-in: rendered in shell + live-patched via SSE
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        label: "Live counter".to_string(),
        count: LiveProp::watch(counter_tx().subscribe()),
        // initial value = *rx.borrow() at request time
    })
}
```

Template — write `{{ count }}` exactly as you would any other field:

```html
<p>Count: {{ count }}</p>
```

Pilcrow rewrites this at build time into:

```html
<p>Count: <span data-pilcrow-live-field="count">{{ count }}</span></p>
```

The initial value bakes into the HTML at request time (`14`, `42`, etc.). After page load Silcrow opens an SSE connection and patches the span's `textContent` on every update — **no full re-render, no hydration**.

### Producers

```rust
LiveProp::watch(rx)                   // tokio watch::Receiver<T>; initial = *rx.borrow()
LiveProp::poll(initial, dur, || fut)  // interval-based async poll
LiveProp::stream(initial, stream)     // arbitrary Stream<Item = T>
LiveProp::initial(v)                  // static — no producer, use as placeholder in load()
```

### `LiveTarget` — field-level delivery mode

```rust
.target(LiveTarget::Dom)          // default: patch [data-pilcrow-live-field] text only
.target(LiveTarget::DomAndStore)  // patch DOM + publish to Silcrow stream: atom
.target(LiveTarget::Store)        // atom only — no span generated in template
```

`DomAndStore` / `Store` let React / Solid islands subscribe to the stream atom:
```ts
Silcrow.subscribe("stream:/__pilcrow/live/my-page", (data) => { ... })
```

### Pattern B — separate `live()` when `load()` is expensive

When `load()` does heavy DB work you don't want to repeat on each SSE reconnect, declare a `LiveProps` struct and a `live()` function. Codegen calls `live()` for the SSE route and `load()` only for the initial HTTP render.

```rust
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        label: expensive_db_call().await?,  // runs once per page load only
        count: LiveProp::initial(0),        // placeholder value; live() overrides for SSE
    })
}

pub struct LiveProps { pub count: LiveProp<u64> }

pub async fn live(_req: Req) -> AppResult<LiveProps> {
    Ok(LiveProps { count: LiveProp::watch(counter_tx().subscribe()) })
}
```

### Under the hood — full pipeline

#### Build time (codegen)

1. **`instrument.rs`** — detects fields whose type is `LiveProp<_>`, collects their names into `live_fields`.
2. **`compiler.rs` `inject_live_text_spans`** — rewrites `{{ field }}` for each `Dom`/`DomAndStore` live field to `<span data-pilcrow-live-field="field">{{ field }}</span>`. The initial value is still rendered by Askama into the span's text content.
3. **`app_module.rs`** — two injections into the rendered HTML string:
   - Before `</head>`: `<script>window.__pilcrow_live_patch = function(data) { Object.keys(data).forEach(k => querySelectorAll('[data-pilcrow-live-field="'+k+'"]').forEach(n => n.textContent = String(data[k]))) }</script>`
   - Before `</body>`: `<div data-pilcrow-live style="display:none" s-sse="/__pilcrow/live/..."></div>`
4. **`app_module.rs` `emit_live_handler`** — registers `GET /__pilcrow/live/{page_params}` as an Axum SSE route.

#### Runtime (server)

When a browser opens the SSE route:

1. `live()` is called if present; otherwise `load()` is re-called. Static `Props` fields are discarded — only `LiveProp<T>` fields are used.
2. Each field's producer (watch channel / poll / stream) is converted via `__into_sse_stream` to a `Stream<Item = (&'static str, String)>` (field name + JSON-serialized value).
3. All field streams are merged via `select_all` (i.e., updates from different fields interleave freely).
4. `__live_props_response` wraps the merged stream in an Axum SSE response. On each item it emits a `custom` SSE frame:
   ```
   event: custom
   data: {"event":"live","data":{"count":15}}
   ```

#### Runtime (client)

1. **Silcrow** sees the `[s-sse]` div on page load and calls `openLive(el, url)`, opening a single `EventSource` per URL (hub-pooled — multiple tabs to the same page share the hub within that tab).
2. Silcrow's `es.addEventListener("custom", ...)` handler parses the frame and dispatches:
   ```js
   document.dispatchEvent(new CustomEvent("silcrow:sse:live", {
     detail: { url, data: { count: 15 } }
   }))
   ```
3. Silcrow's permanent `silcrow:sse:live` listener (added in `init`) calls `window.__pilcrow_live_patch(data)`.
4. `window.__pilcrow_live_patch` does `querySelectorAll('[data-pilcrow-live-field="count"]')` and sets `textContent`. No diffing, no VDOM — direct string write.

#### s-boost navigation (SPA-style)

When the user navigates via s-boost:

- The `MutationObserver` in Silcrow detects the removed `[s-sse]` div and closes its subscription (hub closes EventSource when subscriber count reaches 0).
- `finalizeNavigation` scans the new `targetEl` for `[s-sse]` elements and calls `openLive` on each, opening a fresh SSE connection for the new page.
- The `silcrow:sse:live` listener is permanent — it was registered once at startup and handles all subsequent pages. If `window.__pilcrow_live_patch` was never defined (navigated from a non-live page), the listener falls back to direct `querySelectorAll` patching inline.

### Cross-browser / cross-tab behaviour

- **Cross-browser tabs**: each tab opens its own `EventSource` to `/__pilcrow/live/...`. The server-side producer (e.g. a `watch::Receiver`) is cloned per connection — all clients receive every update.
- **Reconnects**: Silcrow's hub has exponential back-off reconnect logic. The browser's native `EventSource` also auto-reconnects on network interruption.
- **`LiveProp::watch` initial value**: the initial value baked into the shell is `*rx.borrow()` at the moment `load()` ran. The SSE connection delivers the first update as soon as the watch channel changes — there is no gap because the SSE connection subscribes to the same `watch::Receiver` clone used for the initial borrow.

### Constraints

- `T` must implement `Display` (for shell rendering) + `Clone` + `Serialize` + `Send + 'static`
- Pattern B validation: codegen checks every `LiveProps` field exists in `Props` with matching type — build error on mismatch
- One SSE connection per page URL per tab; all live fields share it

**Key files:**
- `pilcrow/crates/runtime/src/deferred.rs` — `LiveProp<T>`, `LiveProducer<T>`, `LiveTarget`, `__live_props_response`, `__into_sse_stream`
- `pilcrow/crates/routekit/src/templating/codegen/instrument.rs` — detection of `LiveProp` fields
- `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` — head shim + `s-sse` anchor injection, `emit_live_handler`
- `pilcrow/crates/routekit/src/templating/compiler.rs` — `inject_live_text_spans`
- `silcrow/src/silcrow.js` — `initLiveElements`, `openLive`, `silcrow:sse:live` listener, `finalizeNavigation` re-init

---

## 10. Service worker caching

Client-side GET caching. Orthogonal to server rendering mode. Disabled automatically in dev (`PILCROW_DEV=1`). `/_silcrow/` and `/__pilcrow/` prefixes always excluded.

```toml
[service_worker]
enabled = true
strategy = "network-first"        # network-first | cache-first | stale-while-revalidate
precache = ["/", "/about"]        # silcrow.js always added automatically
exclude = ["/api/"]
offline_fallback = "/offline"
```

**Key files**
- `pilcrow/crates/runtime/src/sw.rs` — `generate_sw_source()`, `sw_inject_layer()`
- `pilcrow/crates/core/src/config/config.rs` — `ServiceWorkerConfig`, `SwStrategy`

---

## Combination rules

| Combination | Allowed? |
|-------------|----------|
| ISR + Deferred fields | No — ISR falls through to normal SSR for pages with Deferred fields |
| SSG + Deferred fields | No — `PRERENDER` is a no-op for pages with Deferred fields |
| SSG + ISR (`PRERENDER` + `REVALIDATE`) | Yes — startup-warm ISR, cache stored with ISR TTL |
| Streaming + ISR | **No** — build error |
| Streaming + SSG | **No** — build error |
| Streaming + Deferred fields | **No** — build error |
| React islands + ISR | Yes |
| React islands + SSR Streaming | Yes |
| Pilcrow islands + any page mode | Yes — islands are independent fragment fetches |
| LiveProp + SSR | Yes — SSE supplements every request |
| LiveProp + ISR | Yes — ISR caches HTML shell; SSE provides live data layer on top |
| LiveProp + SSG | Yes — Static HTML; SSE provides live data layer |
| LiveProp + Streaming | Yes — Shell streams first, then SSE connects |
| LiveProp + Deferred fields | Yes — Can coexist on same page |
| LiveProp + React/Pilcrow islands | Yes |
