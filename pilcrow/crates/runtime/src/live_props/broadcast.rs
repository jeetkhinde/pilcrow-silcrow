use tokio::sync::broadcast;

// ── InvalidationEvent ─────────────────────────────────────────────────────────

/// Payload sent over the broadcast channel when a dep key is invalidated.
///
/// SSE handlers subscribe to `LiveBroadcast` and use this to push
/// `SilcrowEvent::patch` updates to connected clients.
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    /// The dep key that was marked stale (e.g. `"tickets:id=123"`).
    pub dep_key: String,
    /// Distinct route patterns whose `pilcrow_cache` rows were marked stale.
    pub affected_routes: Vec<String>,
}

// ── LiveBroadcast ─────────────────────────────────────────────────────────────

/// Shared broadcast state. Registered as an Axum Extension at startup.
///
/// Handlers call `pilcrow::invalidate!(key)` which writes to `pilcrow_cache`
/// and then calls `LiveBroadcast::send` to fan out to all SSE subscribers.
///
/// # SSE subscription example
///
/// ```rust,no_run
/// # use runtime::live_props::{LiveBroadcast, LivePageStore};
/// # use runtime::sse::{sse_stream, SilcrowEvent, EmitError};
/// # use axum::Extension;
/// # use std::sync::Arc;
/// #
/// async fn live_events(
///     Extension(broadcast): Extension<LiveBroadcast>,
///     Extension(store): Extension<Arc<LivePageStore>>,
/// ) -> impl axum::response::IntoResponse {
///     sse_stream(move |emitter| async move {
///         let mut rx = broadcast.subscribe();
///         loop {
///             let event = rx.recv().await.map_err(|_| EmitError::Disconnected)?;
///             for route in &event.affected_routes {
///                 let slots = store
///                     .read_live_slots(route, &serde_json::json!({}))
///                     .await
///                     .unwrap_or_default();
///                 for (slot_name, value) in slots {
///                     let selector = format!("[data-pilcrow-live-field='{}']", slot_name);
///                     emitter.send(SilcrowEvent::patch(value, &selector)).await?;
///                 }
///             }
///         }
///     })
/// }
/// ```
#[derive(Debug, Clone)]
pub struct LiveBroadcast {
    sender: broadcast::Sender<InvalidationEvent>,
}

impl LiveBroadcast {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Send an invalidation event to all active SSE subscribers.
    /// No-op if there are no subscribers.
    pub fn send(&self, event: InvalidationEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<InvalidationEvent> {
        self.sender.subscribe()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriber_receives_sent_event() {
        let broadcast = LiveBroadcast::new(64);
        let mut rx = broadcast.subscribe();
        broadcast.send(InvalidationEvent {
            dep_key: "tickets:id=1".to_string(),
            affected_routes: vec!["/tickets/:id".to_string()],
        });
        let event = rx.try_recv().unwrap();
        assert_eq!(event.dep_key, "tickets:id=1");
        assert_eq!(event.affected_routes, vec!["/tickets/:id".to_string()]);
    }

    #[test]
    fn no_subscribers_send_does_not_panic() {
        let broadcast = LiveBroadcast::new(64);
        broadcast.send(InvalidationEvent {
            dep_key: "x".to_string(),
            affected_routes: vec![],
        });
    }

    #[test]
    fn multiple_subscribers_all_receive_event() {
        let broadcast = LiveBroadcast::new(64);
        let mut rx1 = broadcast.subscribe();
        let mut rx2 = broadcast.subscribe();
        broadcast.send(InvalidationEvent {
            dep_key: "orders:id=99".to_string(),
            affected_routes: vec!["/orders/:id".to_string()],
        });
        assert_eq!(rx1.try_recv().unwrap().dep_key, "orders:id=99");
        assert_eq!(rx2.try_recv().unwrap().dep_key, "orders:id=99");
    }

    #[test]
    fn clone_shares_same_channel() {
        let broadcast = LiveBroadcast::new(64);
        let broadcast2 = broadcast.clone();
        let mut rx = broadcast.subscribe();
        broadcast2.send(InvalidationEvent {
            dep_key: "clone-test".to_string(),
            affected_routes: vec![],
        });
        assert_eq!(rx.try_recv().unwrap().dep_key, "clone-test");
    }
}
