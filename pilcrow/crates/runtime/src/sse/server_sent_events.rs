use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;
use std::convert::Infallible;
use std::future::Future;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

crate::define_route!(SseRoute, "SSE", "/events/feed", "FEED");

#[derive(Debug)]
pub struct SilcrowEvent {
    kind: EventKind,
    id: Option<String>,
}

#[derive(Debug)]
pub(crate) enum EventKind {
    Patch {
        data: Result<serde_json::Value, String>,
        target: String,
        mutation_id: Option<String>,
    },
    Html {
        markup: String,
        target: String,
    },
    Invalidate {
        target: String,
    },
    Navigate {
        path: String,
    },
    Custom {
        event: String,
        data: Result<serde_json::Value, String>,
    },
    ListPatch {
        list: String,
        key: String,
        data: Result<serde_json::Value, String>,
    },
}

impl SilcrowEvent {
    /// Sends JSON data to `Silcrow.patch(data, target)`.
    pub fn patch(data: impl serde::Serialize, target: &str) -> Self {
        Self {
            kind: EventKind::Patch {
                data: serde_json::to_value(data).map_err(|e| e.to_string()),
                target: target.to_owned(),
                mutation_id: None,
            },
            id: None,
        }
    }

    /// Sends HTML markup to `safeSetHTML(element, markup)`.
    pub fn html(markup: impl Into<String>, target: &str) -> Self {
        Self {
            kind: EventKind::Html {
                markup: markup.into(),
                target: target.to_owned(),
            },
            id: None,
        }
    }

    /// Wire-identical to `patch`. Semantic alias.
    pub fn json(data: impl serde::Serialize, target: &str) -> Self {
        Self::patch(data, target)
    }

    /// Tells the client to re-fetch `target` from the server.
    pub fn invalidate(target: &str) -> Self {
        Self {
            kind: EventKind::Invalidate {
                target: target.to_owned(),
            },
            id: None,
        }
    }

    /// Tells the client to navigate to `path`.
    pub fn navigate(path: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Navigate { path: path.into() },
            id: None,
        }
    }

    /// Dispatches a named custom event on the client as `silcrow:sse:custom`.
    pub fn custom(event: impl Into<String>, data: impl serde::Serialize) -> Self {
        Self {
            kind: EventKind::Custom {
                event: event.into(),
                data: serde_json::to_value(data).map_err(|e| e.to_string()),
            },
            id: None,
        }
    }

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

    /// Attach a `Last-Event-ID` so reconnecting clients can resume from this event.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Tag a `patch` event with a client-side mutation id so the client can confirm it.
    pub fn with_mutation_id(mut self, mutation_id: impl Into<String>) -> Self {
        if let EventKind::Patch {
            mutation_id: ref mut mid,
            ..
        } = self.kind
        {
            *mid = Some(mutation_id.into());
        }
        self
    }

    fn serialize_check(&self) -> Result<(), String> {
        match &self.kind {
            EventKind::Patch { data, .. }
            | EventKind::Custom { data, .. }
            | EventKind::ListPatch { data, .. } => data.as_ref().map(|_| ()).map_err(Clone::clone),
            _ => Ok(()),
        }
    }
}

fn apply_id(event: Event, id: Option<String>) -> Event {
    match id {
        Some(id) => event.id(id),
        None => event,
    }
}

impl From<SilcrowEvent> for Event {
    fn from(evt: SilcrowEvent) -> Event {
        let id = evt.id;
        match evt.kind {
            EventKind::Patch {
                data,
                target,
                mutation_id,
            } => match data {
                Err(e) => {
                    tracing::warn!("SilcrowEvent::patch dropped — serialization failed: {e}");
                    Event::default().comment("pilcrow:serialize_error")
                }
                Ok(data) => {
                    let mut payload = serde_json::json!({ "target": target, "data": data });
                    if let Some(mid) = mutation_id {
                        payload["mutation_id"] = serde_json::Value::String(mid);
                    }
                    apply_id(
                        Event::default()
                            .event("patch")
                            .json_data(payload)
                            .unwrap_or_else(|_| Event::default().comment("pilcrow:encode_error")),
                        id,
                    )
                }
            },
            EventKind::Html { markup, target } => apply_id(
                Event::default()
                    .event("html")
                    .json_data(serde_json::json!({ "target": target, "html": markup }))
                    .unwrap_or_else(|_| Event::default().comment("pilcrow:encode_error")),
                id,
            ),
            EventKind::Invalidate { target } => {
                apply_id(Event::default().event("invalidate").data(target), id)
            }
            EventKind::Navigate { path } => {
                apply_id(Event::default().event("navigate").data(path), id)
            }
            EventKind::Custom { event, data } => match data {
                Err(e) => {
                    tracing::warn!("SilcrowEvent::custom dropped — serialization failed: {e}");
                    Event::default().comment("pilcrow:serialize_error")
                }
                Ok(data) => apply_id(
                    Event::default()
                        .event("custom")
                        .json_data(serde_json::json!({ "event": event, "data": data }))
                        .unwrap_or_else(|_| Event::default().comment("pilcrow:encode_error")),
                    id,
                ),
            },
            EventKind::ListPatch { list, key, data } => match data {
                Err(e) => {
                    tracing::warn!("SilcrowEvent::list_patch dropped — serialization failed: {e}");
                    Event::default().comment("pilcrow:serialize_error")
                }
                Ok(mut payload) => {
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
        }
    }
}

#[must_use = "SSE errors must be handled — use ? to propagate"]
#[derive(Debug)]
pub enum EmitError {
    /// The client disconnected. Use `?` to exit the stream loop cleanly.
    Disconnected,
    /// Serialization of the event payload failed before transmission.
    Serialize(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "SSE client disconnected"),
            Self::Serialize(e) => write!(f, "SSE event serialization failed: {e}"),
        }
    }
}

impl std::error::Error for EmitError {}

#[derive(Clone)]
pub struct SseEmitter {
    tx: mpsc::Sender<SilcrowEvent>,
}

impl SseEmitter {
    pub async fn send(&self, event: SilcrowEvent) -> Result<(), EmitError> {
        if let Err(e) = event.serialize_check() {
            tracing::warn!("SilcrowEvent dropped — serialization failed: {e}");
            return Err(EmitError::Serialize(e));
        }
        self.tx
            .send(event)
            .await
            .map_err(|_| EmitError::Disconnected)
    }
    /// Convenience for sending serializable data to a DOM target.
    pub async fn json(&self, target: &str, data: &impl serde::Serialize) -> Result<(), EmitError> {
        self.send(SilcrowEvent::json(data, target)).await
    }
}

pub fn sse_stream<F, Fut>(
    handler: F,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send + 'static>
where
    F: FnOnce(SseEmitter) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), EmitError>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<SilcrowEvent>(32);
    let emitter = SseEmitter { tx };

    tokio::spawn(async move {
        let _ = handler(emitter).await;
    });

    let stream = ReceiverStream::new(rx).map(|event| Ok::<Event, Infallible>(event.into()));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub fn sse_raw<S>(stream: S) -> Sse<S>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_to_string(e: SilcrowEvent) -> String {
        let axum_event: axum::response::sse::Event = e.into();
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
    fn list_patch_numeric_key_serialises() {
        let data = serde_json::json!({"count": 5});
        let evt = SilcrowEvent::list_patch("items", "99", data);
        let rendered = event_to_string(evt);
        assert!(rendered.contains("list-patch"));
        assert!(rendered.contains("99"));
    }
}
