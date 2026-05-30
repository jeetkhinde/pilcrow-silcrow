use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_core::Stream;
use serde::Serialize;
use tokio::sync::watch;
use tokio::time::Duration;

// ── Live props SSE helper ────────────────────────────────────────────────────

/// Build an SSE response for pages with `LiveProp<T>` fields.
///
/// Merges all field streams and emits `live` SSE events containing a flat
/// `{"field_name": value}` JSON object. The injected client script listens for
/// `event: live` and forwards the parsed object directly to `__pilcrow_live_patch`.
/// Called by generated SSE route handlers — not part of the public API.
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub fn __live_props_response(
    streams: Vec<Pin<Box<dyn Stream<Item = (&'static str, String)> + Send + 'static>>>,
) -> impl axum::response::IntoResponse {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use futures_util::StreamExt as _;
    use std::convert::Infallible;

    let merged = futures_util::stream::select_all(streams);

    let event_stream = merged.map(|(field, json_str)| {
        let value: serde_json::Value =
            serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);
        let mut map = serde_json::Map::new();
        map.insert(field.to_string(), value);
        let data = serde_json::to_string(&serde_json::Value::Object(map))
            .unwrap_or_else(|_| "{}".to_string());
        Ok::<Event, Infallible>(Event::default().event("live").data(data))
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

// ── LiveProp<T> ──────────────────────────────────────────────────────────────

/// Controls where live prop updates are delivered for a `LiveProp<T>` field.
///
/// Set via `.target(LiveTarget::...)` on the field constructor.
#[derive(Clone, Debug, Default)]
pub enum LiveTarget {
    /// Patch `[data-pilcrow-live-field]` text nodes in the DOM only. Default.
    #[default]
    Dom,
    /// Patch DOM bindings **and** publish to a `stream:` atom so React/Solid/island
    /// adapters can subscribe via `Silcrow.subscribe(...)`.
    DomAndStore,
    /// Publish to `stream:` atom only — no DOM `:text` binding is generated.
    Store,
}

pub(crate) enum LiveProducer<T: 'static> {
    Watch(watch::Receiver<T>),
    Poll {
        interval: Duration,
        factory: Arc<dyn Fn() -> Pin<Box<dyn Future<Output = T> + Send>> + Send + Sync>,
    },
    Stream(Pin<Box<dyn Stream<Item = T> + Send + 'static>>),
    Static,
}

/// A live-updating prop field. Renders the initial value `T` in the shell HTML, then
/// subscribes the client to a server-pushed SSE channel that patches DOM bindings (and
/// optionally a Silcrow stream atom) whenever the value changes.
///
/// ```rust,ignore
/// pub struct Props {
///     pub post_content: String,
///     pub live_count: LiveProp<u64>,
/// }
///
/// pub async fn load(req: Req) -> AppResult<Props> {
///     Ok(Props {
///         post_content: get_post().await?,
///         live_count: LiveProp::watch(COUNT.subscribe())
///             .target(LiveTarget::Dom),
///     })
/// }
/// ```
pub struct LiveProp<T: 'static> {
    pub(crate) initial: T,
    pub(crate) producer: LiveProducer<T>,
    pub(crate) target: LiveTarget,
}

impl<T: Clone + Send + 'static> LiveProp<T> {
    /// Create a live prop backed by a `tokio::sync::watch` receiver.
    /// Initial value is cloned from the current receiver state.
    pub fn watch(rx: watch::Receiver<T>) -> Self {
        let initial = rx.borrow().clone();
        Self {
            initial,
            producer: LiveProducer::Watch(rx),
            target: LiveTarget::Dom,
        }
    }
}

impl<T: Send + 'static> LiveProp<T> {
    /// Create a live prop that re-runs an async factory on a fixed interval.
    /// `initial` is rendered in the shell; the factory is called each interval tick.
    pub fn poll<Fut>(
        initial: T,
        interval: Duration,
        factory: impl Fn() -> Fut + Send + Sync + 'static,
    ) -> Self
    where
        Fut: Future<Output = T> + Send + 'static,
    {
        Self {
            initial,
            producer: LiveProducer::Poll {
                interval,
                factory: Arc::new(move || Box::pin(factory())),
            },
            target: LiveTarget::Dom,
        }
    }

    /// Create a live prop backed by an arbitrary stream.
    /// `initial` is rendered in the shell; stream items are pushed as live updates.
    pub fn stream(initial: T, stream: impl Stream<Item = T> + Send + 'static) -> Self {
        Self {
            initial,
            producer: LiveProducer::Stream(Box::pin(stream)),
            target: LiveTarget::Dom,
        }
    }

    /// Create a static live prop: initial value rendered, no live updates sent.
    /// Useful when a `live()` function provides producers separately.
    pub fn initial(value: T) -> Self {
        Self {
            initial: value,
            producer: LiveProducer::Static,
            target: LiveTarget::Dom,
        }
    }

    /// Set the delivery target for this field (default: `Dom`).
    pub fn target(mut self, target: LiveTarget) -> Self {
        self.target = target;
        self
    }

    /// Return the delivery target.
    pub fn live_target(&self) -> &LiveTarget {
        &self.target
    }
}

impl<T: Clone + Serialize + Send + Sync + 'static> LiveProp<T> {
    /// Called by generated SSE route code. Converts this prop into a stream of
    /// `(field_name, json_string)` pairs for SSE transmission. Yields on each
    /// value change; never yields the initial value (already in the shell HTML).
    #[doc(hidden)]
    pub fn __into_sse_stream(
        self,
        field: &'static str,
    ) -> Pin<Box<dyn Stream<Item = (&'static str, String)> + Send + 'static>> {
        use futures_util::StreamExt as _;

        match self.producer {
            LiveProducer::Watch(rx) => {
                Box::pin(futures_util::stream::unfold(rx, move |mut rx| async move {
                    rx.changed().await.ok()?;
                    let v = rx.borrow().clone();
                    let json = serde_json::to_string(&v).unwrap_or_default();
                    Some(((field, json), rx))
                }))
            }
            LiveProducer::Poll { interval, factory } => {
                let interval = interval.max(Duration::from_millis(1));
                let start = tokio::time::Instant::now() + interval;
                let ticker = tokio::time::interval_at(start, interval);
                Box::pin(futures_util::stream::unfold(
                    (ticker, factory),
                    move |(mut ticker, factory)| async move {
                        ticker.tick().await;
                        let v = (factory)().await;
                        let json = serde_json::to_string(&v).unwrap_or_default();
                        Some(((field, json), (ticker, factory)))
                    },
                ))
            }
            LiveProducer::Stream(s) => Box::pin(s.map(move |v| {
                let json = serde_json::to_string(&v).unwrap_or_default();
                (field, json)
            })),
            LiveProducer::Static => Box::pin(futures_util::stream::empty()),
        }
    }
}

impl<T: fmt::Display> fmt::Display for LiveProp<T> {
    /// Renders the initial value. Askama calls this for `{{ field }}` in the shell template.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.initial.fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for LiveProp<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LiveProp({:?})", self.initial)
    }
}

impl<T: Serialize> serde::Serialize for LiveProp<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.initial.serialize(serializer)
    }
}

// ── DependencyKey ─────────────────────────────────────────────────────────────

/// A domain-owned key linking a live field to the data that can invalidate it.
///
/// Keys use the shape `"table:column=value"` by convention, matching the
/// `depends_on @> ARRAY['table:column=value']` Postgres invalidation query.
/// Construct with the [`dep!`](crate::dep) macro or `DependencyKey::new("…")`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct DependencyKey(String);

impl DependencyKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DependencyKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DependencyKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
