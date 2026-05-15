use axum::{
    Extension,
    Json,
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
use futures_util::StreamExt as FuturesStreamExt;
use tokio_stream::wrappers::BroadcastStream;

use super::store::FsrStore;
use super::watcher::{WatcherEventTx, execute_with_params};

/// Shared atomic counter of open SSE connections.
///
/// # Contract
/// Only the accept path in `fsr_hub_handler` may call `fetch_add(1, Relaxed)`.
/// Only `ConnectionGuard::drop` may call `fetch_sub`. All other callers are reads only.
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
        let prev = self.0.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(prev > 0, "ConnectionGuard dropped with counter already at zero");
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
    let ttl_fut = Box::pin(tokio::time::sleep(std::time::Duration::from_secs(hub_config.connection_ttl_secs)));

    let after_ttl = FuturesStreamExt::take_until(rx, ttl_fut);
    let stream = tokio_stream::StreamExt::filter_map(after_ttl, move |msg| {
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
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                    tracing::debug!(
                        route = %subscribed_route,
                        lagged_by = n,
                        "FSR client lagged — sending resync"
                    );
                    Some(Ok(Event::default().event("fsr-resync").data("lagged")))
                }
            }
        });

    let guarded = GuardedStream {
        inner: stream,
        _guard: ConnectionGuard(Arc::clone(&counter)),
    };

    Sse::new(guarded)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(hub_config.keepalive_secs)),
        )
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
    let counter = counter.unwrap_or_else(|| Extension(Arc::new(AtomicUsize::new(0))));
    let hub_config = hub_config
        .map(|e| e.0.clone())
        .unwrap_or_else(|| Arc::new(FsrHubConfig::default()));
    fsr_hub_handler(query, tx, counter, Extension(hub_config)).await
}

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

    Json(serde_json::Value::Object(result)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
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
        let _resp = app.oneshot(req).await.unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

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

    #[tokio::test]
    async fn snapshot_returns_503_without_store() {
        let resp = fsr_snapshot_handler(
            Query(FsrHubQuery { route: None, slots: None }),
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
