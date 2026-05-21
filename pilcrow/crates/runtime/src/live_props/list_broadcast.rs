use tokio::sync::broadcast;

use super::list_row::ListRow;

// ── ListPatchEvent ────────────────────────────────────────────────────────────

/// A row change event fanned out by [`ListBroadcast`].
///
/// SSE handlers receive these via `ListBroadcast::subscribe()` and convert them
/// to `SilcrowEvent::list_patch(ev.list_name, ev.key, ev.fields)`.
#[derive(Debug, Clone)]
pub struct ListPatchEvent {
    /// Name of the list field in Props (e.g. `"tickets"`). Matches `data-pilcrow-list` in HTML.
    pub list_name: String,
    /// Row key (e.g. `"42"`). Matches `data-pilcrow-key` in HTML.
    pub key: String,
    /// JSON object containing only the changed live fields, e.g. `{"status": "closed"}`.
    pub fields: serde_json::Value,
}

// ── ListBroadcast ─────────────────────────────────────────────────────────────

/// Shared broadcast channel for keyed list row changes.
///
/// Register one as an Axum `Extension` at startup. Mutation handlers call
/// `send_row` or `send_patch` when a row changes; SSE route handlers subscribe
/// and relay the events to connected browsers as `list-patch` SSE events.
///
/// # Example
///
/// ```rust,no_run
/// # use runtime::live_props::{ListBroadcast, ListRow};
/// # use runtime::sse::{sse_stream, SilcrowEvent, EmitError};
/// # use axum::Extension;
/// # use tokio::sync::broadcast;
/// #
/// async fn live_tickets(
///     Extension(lb): Extension<ListBroadcast>,
/// ) -> impl axum::response::IntoResponse {
///     sse_stream(move |emitter| async move {
///         let mut rx = lb.subscribe();
///         loop {
///             match rx.recv().await {
///                 Ok(ev) => {
///                     emitter
///                         .send(SilcrowEvent::list_patch(ev.list_name, ev.key, ev.fields))
///                         .await?;
///                 }
///                 Err(broadcast::error::RecvError::Closed) => break Ok(()),
///                 Err(broadcast::error::RecvError::Lagged(_)) => continue,
///             }
///         }
///     })
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ListBroadcast {
    sender: broadcast::Sender<ListPatchEvent>,
}

impl ListBroadcast {
    /// Create a new `ListBroadcast` with the given channel capacity.
    /// A capacity of 256 is reasonable for most workloads.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Send a list-patch event derived from a typed row value.
    ///
    /// Extracts the key via `T::pilcrow_key()` and the live field snapshot via
    /// `T::pilcrow_live_fields()`, then broadcasts a [`ListPatchEvent`].
    /// No-op if there are no active subscribers.
    pub fn send_row<T: ListRow>(&self, list_name: &str, row: &T) {
        let key = row.pilcrow_key();
        let mut map = serde_json::Map::new();
        for (name, value) in row.pilcrow_live_fields() {
            map.insert(name.to_string(), value);
        }
        let fields = serde_json::Value::Object(map);
        let _ = self.sender.send(ListPatchEvent {
            list_name: list_name.to_string(),
            key,
            fields,
        });
    }

    /// Send a raw list-patch event without a typed row.
    ///
    /// Use this when you already have the key and changed fields as JSON (e.g. from
    /// a Redis pub/sub message or webhook payload).
    pub fn send_patch(
        &self,
        list_name: &str,
        key: impl Into<String>,
        fields: impl serde::Serialize,
    ) {
        let fields = serde_json::to_value(fields).unwrap_or(serde_json::Value::Null);
        let _ = self.sender.send(ListPatchEvent {
            list_name: list_name.to_string(),
            key: key.into(),
            fields,
        });
    }

    /// Subscribe to list-patch events. Each subscriber receives every event sent
    /// after it subscribes. Events may be lost if the subscriber falls behind by
    /// more than the channel capacity (`Lagged` error); the caller should
    /// `continue` on `Lagged` rather than treating it as fatal.
    pub fn subscribe(&self) -> broadcast::Receiver<ListPatchEvent> {
        self.sender.subscribe()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct TicketRow {
        id: i64,
        status: String,
    }

    impl ListRow for TicketRow {
        fn pilcrow_key(&self) -> String {
            self.id.to_string()
        }

        fn pilcrow_live_fields(&self) -> Vec<(&'static str, serde_json::Value)> {
            vec![("status", serde_json::json!(self.status))]
        }
    }

    #[test]
    fn send_row_broadcasts_key_and_live_fields() {
        let lb = ListBroadcast::new(16);
        let mut rx = lb.subscribe();
        let row = TicketRow { id: 42, status: "open".to_string() };
        lb.send_row("tickets", &row);
        let ev = rx.try_recv().unwrap();
        assert_eq!(ev.list_name, "tickets");
        assert_eq!(ev.key, "42");
        assert_eq!(ev.fields["status"], serde_json::json!("open"));
    }

    #[test]
    fn send_patch_broadcasts_raw_fields() {
        let lb = ListBroadcast::new(16);
        let mut rx = lb.subscribe();
        lb.send_patch("tickets", "99", serde_json::json!({"status": "closed"}));
        let ev = rx.try_recv().unwrap();
        assert_eq!(ev.list_name, "tickets");
        assert_eq!(ev.key, "99");
        assert_eq!(ev.fields["status"], serde_json::json!("closed"));
    }

    #[test]
    fn no_subscribers_send_does_not_panic() {
        let lb = ListBroadcast::new(16);
        let row = TicketRow { id: 1, status: "pending".to_string() };
        lb.send_row("tasks", &row);
    }

    #[test]
    fn multiple_subscribers_all_receive() {
        let lb = ListBroadcast::new(16);
        let mut rx1 = lb.subscribe();
        let mut rx2 = lb.subscribe();
        lb.send_patch("orders", "7", serde_json::json!({"total": 99}));
        assert_eq!(rx1.try_recv().unwrap().key, "7");
        assert_eq!(rx2.try_recv().unwrap().key, "7");
    }

    #[test]
    fn clone_shares_same_channel() {
        let lb = ListBroadcast::new(16);
        let lb2 = lb.clone();
        let mut rx = lb.subscribe();
        lb2.send_patch("items", "5", serde_json::json!({"count": 3}));
        assert_eq!(rx.try_recv().unwrap().list_name, "items");
    }
}
