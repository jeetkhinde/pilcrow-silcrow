# FSR SSE Best Practices Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden the FSR SSE layer with a connection limit, per-connection TTL, lag-resync via a snapshot endpoint, configurable keepalive, window-scoped client script, and live connection count in the dev inspect page.

**Architecture:** A `FsrHubConfig` struct (injected as Axum Extension) carries the three new tunables. A `FsrConnectionCounter` (`Arc<AtomicUsize>`) is shared across all connections; a `GuardedStream` wrapper ensures the counter is decremented exactly when Axum drops the SSE body (not when the handler returns). Lagged clients receive an `fsr-resync` SSE event and respond by fetching `/__pilcrow/fsr/snapshot` for fresh slot values.

**Tech Stack:** Rust, Axum (SSE + Extensions), tokio-stream (`take_until`, `filter_map`), sqlx (Postgres), vanilla JS (FSR client script in codegen constant).

---

## File Map

| File | Change |
|------|--------|
| `pilcrow/crates/core/src/config/config.rs` | Add `max_sse_connections`, `connection_ttl_secs`, `keepalive_secs` to `FsrConfig` |
| `pilcrow/crates/runtime/src/fsr/watcher.rs` | Change `execute_with_params` to `pub(crate)` |
| `pilcrow/crates/runtime/src/fsr/hub.rs` | Add `FsrHubConfig`, `FsrConnectionCounter`, `ConnectionGuard`, `GuardedStream`; update both handlers; add snapshot handler |
| `pilcrow/crates/runtime/src/fsr/store.rs` | Add `fetch_slots_for_snapshot` method |
| `pilcrow/crates/runtime/src/fsr/inspect.rs` | Accept `FsrConnectionCounter` extension, show count in HTML |
| `pilcrow/crates/runtime/src/fsr/mod.rs` | Export `FsrConnectionCounter`, `FsrHubConfig`, `fsr_snapshot_handler` |
| `pilcrow/crates/runtime/src/start.rs` | Wire counter + hub config extensions; register snapshot route |
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | Update `FSR_PATCH_SCRIPT` constant |

---

## Task 1: Add config fields to `FsrConfig`

**Files:**
- Modify: `pilcrow/crates/core/src/config/config.rs:56-79`

- [ ] **Step 1: Write the failing test**

Add inside the `#[cfg(test)]` block at the bottom of `config.rs` (create it if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsr_config_new_fields_have_correct_defaults() {
        let cfg = FsrConfig::default();
        assert_eq!(cfg.max_sse_connections, 1000);
        assert_eq!(cfg.connection_ttl_secs, 3600);
        assert_eq!(cfg.keepalive_secs, 30);
    }

    #[test]
    fn fsr_config_new_fields_deserialize_from_toml() {
        let toml = r#"
            max_sse_connections = 500
            connection_ttl_secs  = 7200
            keepalive_secs       = 45
        "#;
        let cfg: FsrConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.max_sse_connections, 500);
        assert_eq!(cfg.connection_ttl_secs, 7200);
        assert_eq!(cfg.keepalive_secs, 45);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-core fsr_config_new_fields 2>&1
```

Expected: compile error — fields don't exist yet.

- [ ] **Step 3: Add the three fields to `FsrConfig` and its `Default`**

In `config.rs`, add to the `FsrConfig` struct after `purge_after_seconds`:

```rust
    /// Maximum concurrent SSE connections before returning 503.
    pub max_sse_connections: u32,
    /// Seconds before forcing a client reconnect (EventSource auto-reconnects).
    pub connection_ttl_secs: u64,
    /// SSE keep-alive heartbeat interval in seconds.
    pub keepalive_secs: u64,
```

In the `Default` impl, add after `purge_after_seconds: 2_592_000,`:

```rust
            max_sse_connections: 1000,
            connection_ttl_secs: 3600,
            keepalive_secs: 30,
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-core fsr_config_new_fields 2>&1
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/core/src/config/config.rs
git commit -m "feat(fsr): add max_sse_connections, connection_ttl_secs, keepalive_secs to FsrConfig"
```

---

## Task 2: Expose `execute_with_params` in watcher.rs

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/watcher.rs:136`

- [ ] **Step 1: Change visibility**

In `watcher.rs`, change the `execute_with_params` function signature from:

```rust
async fn execute_with_params(
```

to:

```rust
pub(crate) async fn execute_with_params(
```

- [ ] **Step 2: Verify the crate compiles**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1
```

Expected: builds without error.

- [ ] **Step 3: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/watcher.rs
git commit -m "refactor(fsr): expose execute_with_params as pub(crate) for snapshot handler"
```

---

## Task 3: New types in `hub.rs` — `FsrHubConfig`, `FsrConnectionCounter`, `ConnectionGuard`, `GuardedStream`

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/hub.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `hub.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn connection_guard_decrements_on_drop() {
        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(3));
        {
            let _guard = ConnectionGuard(Arc::clone(&counter));
            assert_eq!(counter.load(Ordering::Relaxed), 3);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn fsr_hub_config_default_values() {
        let cfg = FsrHubConfig::default();
        assert_eq!(cfg.max_connections, 1000);
        assert_eq!(cfg.connection_ttl_secs, 3600);
        assert_eq!(cfg.keepalive_secs, 30);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime connection_guard 2>&1
```

Expected: compile error — types don't exist yet.

- [ ] **Step 3: Add the types to `hub.rs`**

Replace the top of `hub.rs` (all current imports + the file contents) with the following. Keep the existing `FsrHubQuery` struct and both handler stubs intact — only add new declarations at the top:

```rust
use axum::{
    Extension,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_core::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::store::FsrStore;
use super::watcher::{WatcherEventTx, execute_with_params};

/// Shared atomic counter of open SSE connections.
pub type FsrConnectionCounter = Arc<AtomicUsize>;

/// Runtime configuration for the FSR SSE hub, derived from `FsrConfig`.
#[derive(Debug, Clone)]
pub struct FsrHubConfig {
    pub max_connections: usize,
    pub connection_ttl_secs: u64,
    pub keepalive_secs: u64,
}

impl Default for FsrHubConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_ttl_secs: 3600,
            keepalive_secs: 30,
        }
    }
}

/// Decrements the connection counter when dropped (i.e. when the SSE stream ends).
struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Wraps a stream and keeps a `ConnectionGuard` alive until the stream is dropped.
///
/// Axum drops the SSE body (and therefore the stream) when the HTTP connection
/// closes — that is the correct moment to decrement the counter, not when the
/// handler function returns.
///
/// Requires `S: Unpin`. In `fsr_hub_handler` we guarantee this by passing
/// `Box::pin(sleep(...))` to `take_until` — `tokio::time::Sleep` is `!Unpin`
/// but `Pin<Box<Sleep>>` is `Unpin`, so the composed stream remains `Unpin`.
struct GuardedStream<S> {
    inner: S,
    _guard: ConnectionGuard,
}

impl<S: Stream + Unpin> Stream for GuardedStream<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl<S: Unpin> Unpin for GuardedStream<S> {}
```

Leave the rest of the file (`FsrHubQuery`, both handlers) exactly as they are for now — they will be updated in Tasks 5 and 6.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime connection_guard fsr_hub_config_default 2>&1
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/hub.rs
git commit -m "feat(fsr): add FsrHubConfig, FsrConnectionCounter, ConnectionGuard, GuardedStream types"
```

---

## Task 4: Add `fetch_slots_for_snapshot` to `FsrStore`

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/store.rs`

- [ ] **Step 1: Write the failing test**

Add inside the `store.rs` test block (or create `#[cfg(test)] mod tests {}`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_slot_fields_cover_snapshot_needs() {
        // Compile-time check: StaleSlot has all fields needed by the snapshot handler.
        fn _assert_fields(s: StaleSlot) {
            let _: Option<String> = s.query;
            let _: Option<serde_json::Value> = s.query_params;
            let _: Option<String> = s.column_name;
            let _: String = s.slot;
        }
    }
}
```

- [ ] **Step 2: Run test to verify it passes immediately** (it's a compile check)

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime stale_slot_fields 2>&1
```

Expected: passes (fields already exist on `StaleSlot`).

- [ ] **Step 3: Add the method to `FsrStore`**

After `fetch_stale_slots` (line ~166) in `store.rs`, add:

```rust
    /// Fetch all slot rows for a route for use by the snapshot endpoint.
    ///
    /// When `slots` is non-empty, only those slot names are returned.
    /// Never returns the route-level row (slot = '').
    pub async fn fetch_slots_for_snapshot(
        &self,
        route: &str,
        slots: &[&str],
    ) -> sqlx::Result<Vec<StaleSlot>> {
        if slots.is_empty() {
            sqlx::query_as(
                "SELECT route, slot, query, query_params, depends_on, promoted, \
                 debounce_secs, html_path, json_path, column_name \
                 FROM pilcrow_fsr \
                 WHERE route = $1 AND slot != '' \
                 ORDER BY slot",
            )
            .bind(route)
            .fetch_all(&*self.pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT route, slot, query, query_params, depends_on, promoted, \
                 debounce_secs, html_path, json_path, column_name \
                 FROM pilcrow_fsr \
                 WHERE route = $1 AND slot != '' AND slot = ANY($2) \
                 ORDER BY slot",
            )
            .bind(route)
            .bind(slots)
            .fetch_all(&*self.pool)
            .await
        }
    }
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/store.rs
git commit -m "feat(fsr): add FsrStore::fetch_slots_for_snapshot for lag-resync endpoint"
```

---

## Task 5: Add `fsr_snapshot_handler` to `hub.rs`

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/hub.rs`

- [ ] **Step 1: Write the failing test**

In the `#[cfg(test)]` block in `hub.rs`, add:

```rust
    #[tokio::test]
    async fn snapshot_returns_503_without_store() {
        let resp = fsr_snapshot_handler(
            Query(FsrHubQuery { route: None, slots: None }),
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime snapshot_returns_503 2>&1
```

Expected: compile error — `fsr_snapshot_handler` doesn't exist yet.

- [ ] **Step 3: Implement `fsr_snapshot_handler`**

Add before the `#[cfg(test)]` block in `hub.rs`:

```rust
/// Handler for `GET /__pilcrow/fsr/snapshot?route=...&slots=...`.
///
/// Re-executes stored queries for the requested slots and returns their current
/// values as JSON. Called by the client after receiving an `fsr-resync` event.
pub async fn fsr_snapshot_handler(
    Query(query): Query<FsrHubQuery>,
    store: Option<Extension<Arc<FsrStore>>>,
) -> Response {
    let Some(Extension(store)) = store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "FSR store unavailable").into_response();
    };

    let route = query.route.unwrap_or_default();
    let slot_names: Vec<&str> = query
        .slots
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();

    let slots = match store.fetch_slots_for_snapshot(&route, &slot_names).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "FSR snapshot: DB error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut result = serde_json::Map::new();
    for slot in &slots {
        let Some(ref sql) = slot.query else { continue };
        let params: Vec<serde_json::Value> = slot
            .query_params
            .as_ref()
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        match execute_with_params(store.pool(), sql, &params).await {
            Ok(Some(row)) => {
                let col_key = slot.column_name.as_deref().unwrap_or(&slot.slot);
                if let Some(v) = row.get(col_key) {
                    result.insert(slot.slot.clone(), v.clone());
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(slot = %slot.slot, error = %e, "FSR snapshot: query error for slot");
            }
        }
    }

    axum::Json(serde_json::Value::Object(result)).into_response()
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime snapshot_returns_503 2>&1
```

Expected: passes.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/hub.rs
git commit -m "feat(fsr): add fsr_snapshot_handler for lag-resync slot re-fetch"
```

---

## Task 6: Update `fsr_hub_handler` with limit, TTL, lag resync, configurable keepalive

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/hub.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)]` block in `hub.rs`:

```rust
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt as _;
    use crate::fsr::watcher::SlotPatch;

    fn make_app(counter: FsrConnectionCounter, max: usize) -> Router {
        let (tx, _) = tokio::sync::broadcast::channel::<SlotPatch>(1);
        let event_tx: Arc<WatcherEventTx> = Arc::new(tx);
        let hub_config = Arc::new(FsrHubConfig {
            max_connections: max,
            connection_ttl_secs: 3600,
            keepalive_secs: 30,
        });
        Router::new()
            .route("/__pilcrow/fsr", get(fsr_hub_handler))
            .layer(Extension(event_tx))
            .layer(Extension(counter))
            .layer(Extension(hub_config))
    }

    #[tokio::test]
    async fn hub_returns_503_when_limit_reached() {
        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(1)); // already at limit
        let app = make_app(counter, 1);
        let req = Request::builder()
            .uri("/__pilcrow/fsr?route=/test&slots=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn hub_returns_200_when_below_limit() {
        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(0));
        let app = make_app(counter, 10);
        let req = Request::builder()
            .uri("/__pilcrow/fsr?route=/test&slots=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn hub_increments_counter_on_connect() {
        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(0));
        let app = make_app(Arc::clone(&counter), 10);
        let req = Request::builder()
            .uri("/__pilcrow/fsr?route=/test&slots=")
            .body(Body::empty())
            .unwrap();
        // Drive the response to open the SSE stream.
        let _resp = app.oneshot(req).await.unwrap();
        // Counter is incremented during the connection.
        // After oneshot completes the response is returned (SSE header sent)
        // but the body hasn't been consumed yet — counter should be 1.
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime hub_returns_503 hub_returns_200 hub_increments 2>&1
```

Expected: compile error — `fsr_hub_handler` still has the old signature.

- [ ] **Step 3: Replace `fsr_hub_handler` and `fsr_hub_handler_or_unavailable`**

Replace both existing handler functions (from the `#[derive(Debug, Deserialize)]` for `FsrHubQuery` through to the end of `fsr_hub_handler_or_unavailable`) with the following:

```rust
#[derive(Debug, Deserialize)]
pub struct FsrHubQuery {
    pub route: Option<String>,
    pub slots: Option<String>,
}

/// SSE handler at `/__pilcrow/fsr`.
///
/// - Returns 503 when the connection limit (`FsrHubConfig::max_connections`) is reached.
/// - Sends `fsr-resync` when the broadcast buffer overflows (client missed events).
/// - Closes the stream after `connection_ttl_secs`; `EventSource` auto-reconnects.
/// - Heartbeat interval is `keepalive_secs`.
pub async fn fsr_hub_handler(
    Query(query): Query<FsrHubQuery>,
    Extension(event_tx): Extension<Arc<WatcherEventTx>>,
    Extension(counter): Extension<FsrConnectionCounter>,
    Extension(hub_config): Extension<Arc<FsrHubConfig>>,
) -> Response {
    let current = counter.fetch_add(1, Ordering::Relaxed);
    if current >= hub_config.max_connections {
        counter.fetch_sub(1, Ordering::Relaxed);
        return (StatusCode::SERVICE_UNAVAILABLE, "FSR connection limit reached").into_response();
    }

    let subscribed_route = query.route.unwrap_or_default();
    let subscribed_slots: Vec<String> = query
        .slots
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    tracing::debug!(
        route = %subscribed_route,
        active_connections = current + 1,
        "FSR client connected"
    );

    let rx = BroadcastStream::new(event_tx.subscribe());

    // Box::pin is required: tokio::time::Sleep is !Unpin, and GuardedStream
    // requires its inner stream to be Unpin so we can poll it through Pin<&mut Self>.
    let ttl_fut = Box::pin(tokio::time::sleep(Duration::from_secs(hub_config.connection_ttl_secs)));

    let stream = rx
        .take_until(ttl_fut)
        .filter_map(move |msg| {
            let subscribed_route = subscribed_route.clone();
            let subscribed_slots = subscribed_slots.clone();
            match msg {
                Ok(patch) => {
                    if patch.route != subscribed_route {
                        return None;
                    }
                    if !subscribed_slots.is_empty() && !subscribed_slots.contains(&patch.slot) {
                        return None;
                    }
                    let payload = serde_json::json!({ &patch.slot: patch.value });
                    Some(Ok::<Event, Infallible>(
                        Event::default().event("fsr").data(payload.to_string()),
                    ))
                }
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    tracing::debug!(
                        route = %subscribed_route,
                        lagged_by = n,
                        "FSR client lagged — sending resync"
                    );
                    Some(Ok(Event::default().event("fsr-resync").data("lagged")))
                }
                Err(_) => None,
            }
        });

    let guarded = GuardedStream {
        inner: stream,
        _guard: ConnectionGuard(Arc::clone(&counter)),
    };

    Sse::new(guarded)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(hub_config.keepalive_secs)))
        .into_response()
}

/// SSE handler variant that returns 503 when FSR is not configured.
///
/// Use this when registering the route manually without guaranteed extensions.
pub async fn fsr_hub_handler_or_unavailable(
    query: Query<FsrHubQuery>,
    event_tx: Option<Extension<Arc<WatcherEventTx>>>,
    counter: Option<Extension<FsrConnectionCounter>>,
    hub_config: Option<Extension<Arc<FsrHubConfig>>>,
) -> Response {
    let Some(tx) = event_tx else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "FSR not configured (DATABASE_URL missing)",
        )
            .into_response();
    };
    let counter =
        counter.unwrap_or_else(|| Extension(Arc::new(AtomicUsize::new(0))));
    let hub_config = hub_config
        .map(|e| e.0.clone())
        .unwrap_or_else(|| Arc::new(FsrHubConfig::default()));
    fsr_hub_handler(query, tx, counter, Extension(hub_config)).await
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime hub_returns_503 hub_returns_200 hub_increments 2>&1
```

Expected: all 3 pass.

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/hub.rs
git commit -m "feat(fsr): connection limit, TTL, lag-resync, configurable keepalive in fsr_hub_handler"
```

---

## Task 7: Show live connection count in the dev inspect page

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/inspect.rs`

- [ ] **Step 1: Write the failing test**

In `inspect.rs` tests block, add:

```rust
    #[tokio::test]
    async fn inspect_shows_connection_count() {
        use crate::fsr::hub::FsrConnectionCounter;
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;
        use axum::body::to_bytes;

        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(42));
        let resp = fsr_inspect_handler(None, Some(Extension(counter))).await;
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("42"), "expected connection count 42 in HTML, got: {html}");
    }

    #[tokio::test]
    async fn inspect_shows_zero_without_counter() {
        use axum::body::to_bytes;
        let resp = fsr_inspect_handler(None, None).await;
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains('0'), "expected 0 connections in HTML, got: {html}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime inspect_shows_connection 2>&1
```

Expected: compile error — `fsr_inspect_handler` signature doesn't accept a counter yet.

- [ ] **Step 3: Update `fsr_inspect_handler` and `build_html`**

Replace `fsr_inspect_handler` and `not_configured_html` in `inspect.rs`:

Add `use super::hub::FsrConnectionCounter;` to the imports at the top of `inspect.rs` (alongside the existing `use super::store::{FsrStore, InspectRow};`), then replace the handler and `not_configured_html`:

```rust
/// Dev-only handler for `GET /__pilcrow/fsr/inspect`.
pub async fn fsr_inspect_handler(
    store: Option<Extension<Arc<FsrStore>>>,
    counter: Option<Extension<FsrConnectionCounter>>,
) -> axum::response::Response {
    let content_type = [(header::CONTENT_TYPE, "text/html; charset=utf-8")];
    let conn_count = counter
        .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(0);

    let Some(Extension(store)) = store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            content_type,
            not_configured_html(conn_count),
        )
            .into_response();
    };

    match store.fetch_all_for_inspect().await {
        Ok(rows) => (content_type, build_html(&rows, conn_count)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            content_type,
            format!("<!doctype html><html><body><pre>DB error: {e}</pre></body></html>"),
        )
            .into_response(),
    }
}

fn not_configured_html(conn_count: usize) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>FSR Inspect</title></head>\
         <body><h1>FSR Inspect</h1>\
         <p><strong>Live SSE connections:</strong> {conn_count}</p>\
         <p>FSR store unavailable — <code>DATABASE_URL</code> not configured or connection failed.</p>\
         </body></html>"
    )
}
```

In `build_html`, change the signature to `fn build_html(rows: &[InspectRow], conn_count: usize) -> String` and add one line after `<h1>FSR Inspect</h1>`:

```rust
    out.push_str(&format!(
        "<p><strong>Live SSE connections:</strong> {conn_count}</p>\n"
    ));
```

Also update the existing tests in `inspect.rs` that call `fsr_inspect_handler(None)` to pass the new second argument:

```rust
// Change every occurrence of:
fsr_inspect_handler(None).await
// to:
fsr_inspect_handler(None, None).await
```

- [ ] **Step 4: Run all inspect tests**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime inspect 2>&1
```

Expected: all 5 tests pass (`handler_returns_503_without_store`, `route_absent_returns_404`, `route_present_returns_non_404_in_dev_mode`, `inspect_shows_connection_count`, `inspect_shows_zero_without_counter`).

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/inspect.rs
git commit -m "feat(fsr): show live SSE connection count in dev inspect page"
```

---

## Task 8: Update `mod.rs` exports

**Files:**
- Modify: `pilcrow/crates/runtime/src/fsr/mod.rs`

- [ ] **Step 1: Add new exports**

Replace the current `pub use hub::fsr_hub_handler;` line with:

```rust
pub use hub::{
    fsr_hub_handler,
    fsr_hub_handler_or_unavailable,
    fsr_snapshot_handler,
    FsrConnectionCounter,
    FsrHubConfig,
};
```

- [ ] **Step 2: Verify build**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add pilcrow/crates/runtime/src/fsr/mod.rs
git commit -m "chore(fsr): export FsrConnectionCounter, FsrHubConfig, fsr_snapshot_handler from fsr mod"
```

---

## Task 9: Wire everything in `start.rs`

**Files:**
- Modify: `pilcrow/crates/runtime/src/start.rs:190-241`

- [ ] **Step 1: Update the FSR wiring block**

Find the `#[cfg(feature = "live-props")]` block (around line 191). Replace the FSR route + extension setup with:

```rust
    #[cfg(feature = "live-props")]
    {
        use crate::fsr::watcher::spawn_embedded_watcher;
        use crate::fsr::{
            FsrConnectionCounter, FsrHubConfig, FsrStore, WatcherConfig, WatcherEventTx,
        };
        use std::sync::atomic::AtomicUsize;

        let fsr_tx: Arc<WatcherEventTx> =
            Arc::new(tokio::sync::broadcast::channel::<crate::fsr::watcher::SlotPatch>(256).0);

        let fsr_counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(0));

        let fsr_hub_config = Arc::new(FsrHubConfig {
            max_connections: fsr_config.max_sse_connections as usize,
            connection_ttl_secs: fsr_config.connection_ttl_secs,
            keepalive_secs: fsr_config.keepalive_secs,
        });

        app = app
            .route(
                "/__pilcrow/fsr",
                axum::routing::get(crate::fsr::fsr_hub_handler),
            )
            .route(
                "/__pilcrow/fsr/snapshot",
                axum::routing::get(crate::fsr::fsr_snapshot_handler),
            )
            .layer(axum::Extension(Arc::clone(&fsr_tx)))
            .layer(axum::Extension(Arc::clone(&fsr_counter)))
            .layer(axum::Extension(Arc::clone(&fsr_hub_config)));

        if dev_mode {
            app = app.route(
                "/__pilcrow/fsr/inspect",
                axum::routing::get(crate::fsr::fsr_inspect_handler),
            );
        }

        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            match sqlx::PgPool::connect(&db_url).await {
                Ok(pool) => {
                    let fsr_store = Arc::new(FsrStore::new(pool));
                    app = app.layer(axum::Extension(Arc::clone(&fsr_store)));

                    if fsr_config.watcher == "embedded" {
                        let watcher_cfg = WatcherConfig {
                            poll_interval_ms: fsr_config.poll_interval_ms,
                            promote_after_hits: fsr_config.promote_after_hits,
                            patch_debounce_secs: fsr_config.patch_debounce_secs,
                            purge_after_seconds: fsr_config.purge_after_seconds,
                        };
                        spawn_embedded_watcher(
                            Arc::clone(&fsr_store),
                            watcher_cfg,
                            Some((*fsr_tx).clone()),
                        );
                        tracing::info!(
                            "FSR: embedded watcher started (poll: {}ms, max_connections: {}, ttl: {}s)",
                            fsr_config.poll_interval_ms,
                            fsr_config.max_sse_connections,
                            fsr_config.connection_ttl_secs,
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!("FSR: failed to connect to DATABASE_URL for FsrStore: {err}");
                }
            }
        }
    }
```

- [ ] **Step 2: Verify the full runtime crate builds**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1
```

Expected: clean.

- [ ] **Step 3: Verify the demo app builds**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add pilcrow/crates/runtime/src/start.rs
git commit -m "feat(fsr): wire FsrConnectionCounter, FsrHubConfig, snapshot route in start.rs"
```

---

## Task 10: Update `FSR_PATCH_SCRIPT` in `app_module.rs`

**Files:**
- Modify: `pilcrow/crates/routekit/src/templating/codegen/app_module.rs:10`

- [ ] **Step 1: Replace the `FSR_PATCH_SCRIPT` constant**

Find `const FSR_PATCH_SCRIPT: &str = "...";` (line 10) and replace the entire string value with:

```rust
const FSR_PATCH_SCRIPT: &str = "(function(){window.__fsr_route=window.location.pathname;\
function __fsr_slots(){return Array.from(document.querySelectorAll('[s-live]')).map(function(e){return e.getAttribute('s-live');}).filter(Boolean).join(',');}\
function __fsr_patch(d){try{Object.keys(d).forEach(function(k){var v=d[k];if(v!==null&&typeof v==='object'&&!Array.isArray(v)){if(window.Silcrow&&window.Silcrow.publish){window.Silcrow.publish('fsr.'+k,v);}}else{document.querySelectorAll('[s-live=\"'+k+'\"]').forEach(function(n){n.textContent=v==null?'':String(v);});}});}catch(x){}}\
function __fsr_resync(){fetch('/__pilcrow/fsr/snapshot?route='+encodeURIComponent(window.__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots())).then(function(r){return r.json();}).then(__fsr_patch).catch(function(){});}\
function __fsr_connect(){if(window.__fsr_es){window.__fsr_es.close();}var url='/__pilcrow/fsr?route='+encodeURIComponent(window.__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots());window.__fsr_es=new EventSource(url);window.__fsr_es.addEventListener('fsr',function(e){try{__fsr_patch(JSON.parse(e.data));}catch(x){}});window.__fsr_es.addEventListener('fsr-resync',function(){__fsr_resync();});}\
__fsr_connect();\
if(!window.__fsr_nav_bound){window.__fsr_nav_bound=true;document.addEventListener('silcrow:navigate',function(){window.__fsr_route=window.location.pathname;__fsr_connect();});}\
})()";
```

Changes from the old script:
- `__fsr_route`, `__fsr_es`, `__fsr_nav_bound` are now `window.*` — shared across IIFE re-executions
- `__fsr_patch` extracted as a named function (DRY between `fsr` and `fsr-resync` events)
- `__fsr_resync` fetches `/__pilcrow/fsr/snapshot` and applies `__fsr_patch`
- `fsr-resync` event listener added inside `__fsr_connect`
- Navigate listener is guarded by `window.__fsr_nav_bound` to prevent duplicate bindings

- [ ] **Step 2: Verify routekit builds**

```bash
cargo build --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit 2>&1
```

Expected: clean.

- [ ] **Step 3: Verify demo app builds**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add pilcrow/crates/routekit/src/templating/codegen/app_module.rs
git commit -m "feat(fsr): window-scoped client vars, fsr-resync handler, nav-listener guard in FSR_PATCH_SCRIPT"
```

---

## Task 11: Full test suite verification

- [ ] **Step 1: Run all runtime tests**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime 2>&1
```

Expected: all tests pass with no failures.

- [ ] **Step 2: Run all routekit tests**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-routekit 2>&1
```

Expected: all tests pass.

- [ ] **Step 3: Run MCP server tests**

```bash
cargo test --manifest-path pilcrow/tools/mcp/pilcrow-mcp/Cargo.toml 2>&1
```

Expected: all tests pass.

- [ ] **Step 4: Final demo build**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1
```

Expected: clean build.

- [ ] **Step 5: Commit if any minor fixes were needed**

```bash
git add -p
git commit -m "fix(fsr): address any test failures found during full suite run"
```

---

## Pilcrow.toml reference (for app developers)

After this change, the `[fsr]` section in `Pilcrow.toml` accepts three new optional fields:

```toml
[fsr]
max_sse_connections = 1000   # 503 when exceeded; default 1000
connection_ttl_secs  = 3600  # force reconnect after N seconds; default 3600
keepalive_secs       = 30    # heartbeat interval; default 30
```

All three are optional — existing apps with no `[fsr]` section get the defaults automatically.
