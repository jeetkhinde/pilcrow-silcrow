# Security and Performance Findings - 2026-05-22

Follow-up findings after the Tokio-related fixes.

## High

- Service worker can cache private/authenticated GET responses.
  - `pilcrow/crates/runtime/src/sw.rs`
  - The generated service worker intercepts broad same-origin GET traffic and caches any successful response without honoring `Cache-Control`, `Vary`, cookies, or authorization context.
  - Safer default: cache only explicit static/precache assets or responses with a framework opt-in header.

- HTML injection middlewares buffer response bodies without a size cap.
  - `pilcrow/crates/runtime/src/start.rs`
  - `pilcrow/crates/runtime/src/dev.rs`
  - `pilcrow/crates/runtime/src/sw.rs`
  - `to_bytes(..., usize::MAX)` can load large or streaming HTML responses fully into memory.
  - Add a size limit, check `Content-Length`, and skip injection above the cap.

- React island SSR can block async request workers.
  - `pilcrow/crates/runtime/src/island_ssr.rs`
  - Request handling takes a `std::sync::Mutex` and performs blocking stdin/stdout IO to the Node worker.
  - Move render calls to `spawn_blocking`, add a timeout, and consider a small worker pool.

## Medium

- Malformed or oversized form bodies are silently treated as empty forms.
  - `pilcrow/crates/runtime/src/context.rs`
  - `Form::<Vec<(String, String)>>::from_request(...).await.unwrap_or_default()` masks parse/body-limit failures.
  - Return `400` or `413` instead of constructing an empty `FormMap`.

- `LiveProp::poll(Duration::ZERO, ...)` can panic.
  - `pilcrow/crates/runtime/src/deferred.rs`
  - `tokio::time::interval_at` panics on zero duration.
  - Clamp to a minimum interval or return a construction error.

- MCP inspection helpers can read absolute paths outside the project.
  - `pilcrow/tools/pilcrow-mcp/src/inspect.rs`
  - `inspect_template` and `inspect_code_behind` accept absolute paths and read them directly.
  - Canonicalize and require paths to stay under the resolved app/project root.

## Low

- Service worker activation deletes unrelated same-origin caches.
  - `pilcrow/crates/runtime/src/sw.rs`
  - Activation deletes every cache key except the current one.
  - Filter deletion to cache names with the `pilcrow-` prefix.

## Gemini Tokio Audit Cross-Check

Confirmed or mostly confirmed:

- React island SSR uses blocking process IO behind `std::sync::Mutex`.
  - `pilcrow/crates/runtime/src/island_ssr.rs`
  - Strong fix: move to `tokio::process` plus `tokio::io` async stdin/stdout, or isolate the current blocking worker behind `spawn_blocking` with a timeout. A channel/actor design is likely cleaner than exposing a mutex to request middleware.

- Response HTML injection still uses unbounded `to_bytes(..., usize::MAX)`.
  - `pilcrow/crates/runtime/src/start.rs`
  - `pilcrow/crates/runtime/src/dev.rs`
  - `pilcrow/crates/runtime/src/sw.rs`
  - Correction: this is response-body buffering, not direct attacker request-body reading. It is still a DoS risk for large or streaming HTML responses.

- No framework-level default request body limit was found in `start.rs`.
  - Consider applying an Axum `DefaultBodyLimit` or equivalent at router construction, with configurable defaults.

- Redis FSR cache entries do not use TTLs.
  - `pilcrow/crates/runtime/src/fsr/cache.rs`
  - `set_html`, `set_json`, and `patch_slot` persist keys indefinitely unless tombstoned or explicitly deleted. Add configurable expiry for route artifacts and slot hashes.

- FSR background tasks are fire-and-forget.
  - `pilcrow/crates/runtime/src/start.rs`
  - `pilcrow/crates/runtime/src/fsr/watcher.rs`
  - Current loops retry many connection failures internally, but panics still silently kill tasks. Add task supervision/logging and restart behavior.

- FSR file patching can create many independent `tokio::fs` reads/writes.
  - `pilcrow/crates/runtime/src/fsr/watcher.rs`
  - Batch multiple slot updates targeting the same HTML/JSON file into one read/patch/write pass.

Partially addressed but still improvable:

- FSR watcher DB fan-out is now bounded with `stale.chunks(8)` plus `join_all`.
  - `pilcrow/crates/runtime/src/fsr/watcher.rs`
  - This avoids unbounded pool pressure, but it is stop-and-go concurrency. `stream::iter(...).buffer_unordered(8)` would keep the pipeline full while preserving bounded concurrency. If output order matters for the later `zip`, carry the row with the result.

Needs careful framing:

- `tokio::sync::Mutex` is not automatically better than `std::sync::Mutex`.
  - Use `tokio::sync::Mutex` only if the lock must be held across `.await`. For the current SSR worker, the larger issue is blocking IO on async workers and global serialization. An async actor or `spawn_blocking` boundary is preferable to simply swapping mutex types.
