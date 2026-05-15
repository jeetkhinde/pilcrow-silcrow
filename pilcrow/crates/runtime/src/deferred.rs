use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use futures_core::Stream;
use futures_util::stream::FuturesUnordered;
use serde::Serialize;
use tokio::sync::{mpsc, watch};
use tokio::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

fn html_stream_response(body: Body) -> Response {
    match axum::http::Response::builder()
        .header("content-type", "text/html; charset=utf-8")
        .header("silcrow-full-reload", "true")
        .body(body)
    {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(error = %err, "failed to build deferred response");
            let mut response = Response::new(Body::from("internal server error"));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
    }
}

// ── AsyncHtml ────────────────────────────────────────────────────────────────

/// A lazily-resolved HTML fragment that streams into a keyed slot after the shell renders.
///
/// Use `AsyncHtml` when you need to stream complex markup (lists, cards, nested HTML) rather
/// than scalar values. In the template, `{{ field }}` renders as a `<span data-pilcrow-async-html>`
/// placeholder; the resolved HTML replaces it once the future completes.
///
/// ```rust,ignore
/// pub struct Props {
///     pub title: String,
///     pub products: AsyncHtml,
/// }
///
/// pub async fn load(_req: Req) -> AppResult<Props> {
///     Ok(Props {
///         title: "Products".into(),
///         products: AsyncHtml::spawn(async {
///             let items = db::get_products().await;
///             render_product_list(items)
///         }),
///     })
/// }
/// ```
///
/// Add a loading skeleton shown while the future resolves:
/// ```rust,ignore
/// AsyncHtml::spawn(async { ... }).with_loading("<div class='skeleton'></div>")
/// ```
pub struct AsyncHtml {
    inner: AsyncHtmlInner,
    pub loading: String,
}

enum AsyncHtmlInner {
    Future(Pin<Box<dyn Future<Output = String> + Send + 'static>>),
    Slot(String),
}

impl AsyncHtml {
    /// Create an async HTML slot backed by a future that resolves to an HTML string.
    pub fn spawn(fut: impl Future<Output = String> + Send + 'static) -> Self {
        Self {
            inner: AsyncHtmlInner::Future(Box::pin(fut)),
            loading: String::new(),
        }
    }

    /// Set the loading skeleton shown in the slot while the future is resolving.
    pub fn with_loading(mut self, html: impl Into<String>) -> Self {
        self.loading = html.into();
        self
    }

    /// Called by generated code: extract the future and loading HTML for streaming.
    #[doc(hidden)]
    pub fn __into_parts(
        self,
    ) -> (
        Pin<Box<dyn Future<Output = String> + Send + 'static>>,
        String,
    ) {
        match self.inner {
            AsyncHtmlInner::Future(f) => (f, self.loading),
            AsyncHtmlInner::Slot(_) => {
                tracing::error!("AsyncHtml::__into_parts called on a slot placeholder");
                (Box::pin(async { String::new() }), self.loading)
            }
        }
    }

    /// Called by generated code: create a placeholder used during shell rendering.
    #[doc(hidden)]
    pub fn __slot(name: impl Into<String>, loading: String) -> Self {
        Self {
            inner: AsyncHtmlInner::Slot(name.into()),
            loading,
        }
    }
}

/// Renders as a unique text marker that generated code replaces with the actual slot span.
///
/// The marker `__pilcrow_html_slot_{name}__` contains no HTML-special characters so Askama
/// will not escape it, making a simple string-replace safe.
impl fmt::Display for AsyncHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            AsyncHtmlInner::Slot(name) => write!(f, "__pilcrow_html_slot_{name}__"),
            AsyncHtmlInner::Future(_) => Ok(()),
        }
    }
}

impl fmt::Debug for AsyncHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            AsyncHtmlInner::Slot(n) => write!(f, "AsyncHtml::Slot({n:?})"),
            AsyncHtmlInner::Future(_) => write!(f, "AsyncHtml::Future(...)"),
        }
    }
}

impl serde::Serialize for AsyncHtml {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("")
    }
}

/// A resolved HTML patch streamed into a named slot.
pub struct AsyncHtmlPatch {
    pub slot: &'static str,
    pub html: String,
}

/// Build a `Stream<Item = AsyncHtmlPatch>` from (slot, future → html) pairs.
/// Resolves concurrently and yields in completion order.
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub fn __async_html_patch_stream(
    pairs: Vec<(&'static str, Pin<Box<dyn Future<Output = String> + Send>>)>,
) -> impl Stream<Item = AsyncHtmlPatch> + Send {
    pairs
        .into_iter()
        .map(|(slot, fut)| async move {
            let html = fut.await;
            AsyncHtmlPatch { slot, html }
        })
        .collect::<FuturesUnordered<_>>()
}

/// Build a streaming `Response` for pages with both `AsyncValue<T>` (JSON) and `AsyncHtml` fields.
///
/// Resolves all futures concurrently; JSON patches call `window.__pilcrow_async_value`,
/// HTML patches call `window.__pilcrow_async_html` to update the async HTML target.
pub fn async_response_combined(
    shell_html: String,
    json_patches: impl Stream<Item = AsyncValuePatch> + Send + 'static,
    html_patches: impl Stream<Item = AsyncHtmlPatch> + Send + 'static,
) -> Response {
    use futures_util::StreamExt as _;

    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(8);

    tokio::spawn(async move {
        if tx.send(Ok(bytes::Bytes::from(shell_html))).await.is_err() {
            return;
        }

        let json_tx = tx.clone();
        let json_task = tokio::spawn(async move {
            tokio::pin!(json_patches);
            while let Some(p) = json_patches.next().await {
                let chunk = async_value_patch_script(p.field, &p.json);
                if json_tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                    break;
                }
            }
        });

        let html_tx = tx;
        let html_task = tokio::spawn(async move {
            tokio::pin!(html_patches);
            while let Some(p) = html_patches.next().await {
                let chunk = format!(
                    "<script>window.__pilcrow_async_html({slot},{html})</script>",
                    slot = serde_json::to_string(p.slot).unwrap_or_default(),
                    html = serde_json::to_string(&p.html).unwrap_or_default(),
                );
                if html_tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                    break;
                }
            }
        });

        let _ = tokio::join!(json_task, html_task);
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    html_stream_response(body)
}

// ── AsyncValue<T> ────────────────────────────────────────────────────────────

/// A lazily-resolved value that the framework can stream to the client after the shell renders.
///
/// Declare `AsyncValue<T>` fields on a `Props` struct to enable streaming:
///
/// ```rust,ignore
/// pub struct Props {
///     pub title: String,
///     pub count: AsyncValue<i32>,
/// }
///
/// pub async fn load(_req: Req) -> AppResult<Props> {
///     Ok(Props {
///         title: "Hello".to_string(),
///         count: AsyncValue::spawn(async {
///             tokio::time::sleep(std::time::Duration::from_millis(200)).await;
///             42
///         }),
///     })
/// }
/// ```
///
/// The framework renders the shell HTML immediately (with `AsyncValue` fields as `""`), then
/// streams `<script>window.__pilcrow_deferred('field_name', value)</script>` patches as each
/// future resolves.
pub struct AsyncValue<T> {
    inner: AsyncValueInner<T>,
}

enum AsyncValueInner<T> {
    Future(Pin<Box<dyn Future<Output = T> + Send + 'static>>),
    Ready(T),
}

impl<T: Serialize + Send + 'static> AsyncValue<T> {
    /// Create an async value backed by a future. Starts resolving when awaited.
    pub fn spawn(fut: impl Future<Output = T> + Send + 'static) -> Self {
        Self {
            inner: AsyncValueInner::Future(Box::pin(fut)),
        }
    }

    /// Create an already-resolved async value.
    pub fn ready(value: T) -> Self {
        Self {
            inner: AsyncValueInner::Ready(value),
        }
    }

    /// Resolve the async value. Consumes self.
    pub async fn resolve(self) -> T {
        match self.inner {
            AsyncValueInner::Future(fut) => fut.await,
            AsyncValueInner::Ready(v) => v,
        }
    }
}

/// Renders as an empty string — the loading placeholder in the shell template.
impl<T> fmt::Display for AsyncValue<T> {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<T: fmt::Debug> fmt::Debug for AsyncValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            AsyncValueInner::Ready(v) => write!(f, "AsyncValue::Ready({v:?})"),
            AsyncValueInner::Future(_) => write!(f, "AsyncValue::Future(...)"),
        }
    }
}

impl<T: Serialize> serde::Serialize for AsyncValue<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.inner {
            AsyncValueInner::Ready(v) => v.serialize(serializer),
            AsyncValueInner::Future(_) => serializer.serialize_str(""),
        }
    }
}

/// A (field_name, serialized_json_value) pair streamed after the shell renders.
pub struct AsyncValuePatch {
    pub field: &'static str,
    pub json: String,
}

fn async_value_patch_script(field: &'static str, json: &str) -> String {
    format!(
        "<script>window.__pilcrow_async_value({name},{value})</script>",
        name = serde_json::to_string(field).unwrap_or_default(),
        value = json,
    )
}

/// Build a streaming `Response` for pages with async value fields.
///
/// - `shell_html`: the fully-rendered shell (async fields show as empty strings).
/// - `patches`: an async iterator of `AsyncValuePatch` values that will be streamed
///   as inline `<script>` chunks after the shell.
pub fn async_value_response(
    shell_html: String,
    patches: impl Stream<Item = AsyncValuePatch> + Send + 'static,
) -> Response {
    use futures_util::StreamExt as _;

    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(8);

    tokio::spawn(async move {
        let shell_bytes = bytes::Bytes::from(shell_html);
        if tx.send(Ok(shell_bytes)).await.is_err() {
            return;
        }
        tokio::pin!(patches);
        while let Some(patch) = patches.next().await {
            let chunk = async_value_patch_script(patch.field, &patch.json);
            if tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                return;
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    html_stream_response(body)
}

/// Serialize a resolved async value to JSON string, or `"null"` on error.
#[doc(hidden)]
pub fn __serialize_async_value<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
}

/// Build a streaming Response for `STREAMING = true` pages.
///
/// Sends the shell HTML immediately, then awaits the props JSON future and streams
/// a single `window.__ps(json)` call that lets silcrow.js patch all page bindings at once.
/// If the future returns an empty string (load errored or task panicked), no patch is emitted.
#[doc(hidden)]
pub fn __streaming_props_response(
    shell_html: String,
    props_json_future: impl Future<Output = String> + Send + 'static,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(2);

    tokio::spawn(async move {
        if tx.send(Ok(bytes::Bytes::from(shell_html))).await.is_err() {
            return;
        }
        let json = props_json_future.await;
        if !json.is_empty() {
            let chunk = format!("<script>window.__ps({})</script>", json);
            let _ = tx.send(Ok(bytes::Bytes::from(chunk))).await;
        }
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    html_stream_response(body)
}

/// Serialize page Props to a JSON object string for STREAMING patches.
///
/// Returns an empty string on serialization failure (treated as "no patch" by
/// `__streaming_props_response`).
#[doc(hidden)]
pub fn __serialize_page_props<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_default()
}

/// Build a `Stream<Item = AsyncValuePatch>` from a vec of `(field, future → String)` pairs.
/// Resolves patches concurrently and yields them in completion order.
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub fn __async_value_patch_stream(
    pairs: Vec<(&'static str, Pin<Box<dyn Future<Output = String> + Send>>)>,
) -> impl Stream<Item = AsyncValuePatch> + Send {
    pairs
        .into_iter()
        .map(|(field, fut)| async move {
            let json = fut.await;
            AsyncValuePatch { field, json }
        })
        .collect::<FuturesUnordered<_>>()
}

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

// ── BakedProp<T> ─────────────────────────────────────────────────────────────

/// Controls how quickly a baked JSON artifact is updated when a dependency key fires.
///
/// Stored in route metadata so external patchers (written in any language) can honour
/// the same delay without Pilcrow being involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatchDelay {
    /// Write the JSON artifact immediately when the dep key fires.
    Immediate,
    /// Batch rapid-fire signals: wait `millis` ms after the last one before writing.
    Debounced { millis: u64 },
    /// Mark the artifact stale; re-render from source on the next request miss.
    LazyOnNextMiss,
}

impl PatchDelay {
    pub fn debounced(duration: Duration) -> Self {
        Self::Debounced {
            millis: duration.as_millis() as u64,
        }
    }

    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            Self::Debounced { millis } => Some(Duration::from_millis(*millis)),
            _ => None,
        }
    }
}

#[allow(dead_code)]
pub(crate) enum BakedProducer<T: 'static> {
    Watch(watch::Receiver<T>),
    Poll {
        interval: Duration,
        factory: Arc<dyn Fn() -> Pin<Box<dyn Future<Output = T> + Send>> + Send + Sync>,
    },
    Stream(Pin<Box<dyn Stream<Item = T> + Send + 'static>>),
    Static,
}

/// A baked prop field. Renders the initial value `T` in the shell HTML, writes it to
/// a JSON artifact on first hit (or at threshold), and patches the artifact when the
/// dependency key fires — without re-running `load()` or querying the database.
///
/// ```rust,ignore
/// pub struct Props {
///     pub status: BakedProp<String>,
/// }
///
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let id = req.param("id")?;
///     let rx = ticket_status_channel(&id).subscribe();
///     Ok(Props {
///         status: BakedProp::watch(format!("TicketStatus:ticket_id={id}"), rx),
///     })
/// }
/// ```
pub struct BakedProp<T: 'static> {
    pub(crate) initial: T,
    pub(crate) dep_key: String,
    #[allow(dead_code)]
    pub(crate) producer: BakedProducer<T>,
    pub(crate) patch_delay: PatchDelay,
}

impl<T: Clone + Send + 'static> BakedProp<T> {
    /// Create a baked prop backed by a `tokio::sync::watch` receiver.
    /// Initial value is cloned from the current receiver state.
    pub fn watch(dep_key: impl Into<String>, rx: watch::Receiver<T>) -> Self {
        let initial = rx.borrow().clone();
        Self {
            initial,
            dep_key: dep_key.into(),
            producer: BakedProducer::Watch(rx),
            patch_delay: PatchDelay::Immediate,
        }
    }
}

impl<T: Send + 'static> BakedProp<T> {
    /// Create a baked prop that re-runs an async factory on a fixed interval.
    /// `initial` is rendered in the shell; the factory is called each interval tick.
    pub fn poll<Fut>(
        dep_key: impl Into<String>,
        initial: T,
        interval: Duration,
        factory: impl Fn() -> Fut + Send + Sync + 'static,
    ) -> Self
    where
        Fut: Future<Output = T> + Send + 'static,
    {
        Self {
            initial,
            dep_key: dep_key.into(),
            producer: BakedProducer::Poll {
                interval,
                factory: Arc::new(move || Box::pin(factory())),
            },
            patch_delay: PatchDelay::Immediate,
        }
    }

    /// Create a baked prop backed by an arbitrary stream.
    /// `initial` is rendered in the shell; stream items patch the JSON artifact.
    pub fn stream(
        dep_key: impl Into<String>,
        initial: T,
        stream: impl Stream<Item = T> + Send + 'static,
    ) -> Self {
        Self {
            initial,
            dep_key: dep_key.into(),
            producer: BakedProducer::Stream(Box::pin(stream)),
            patch_delay: PatchDelay::Immediate,
        }
    }

    /// Create a static baked prop: initial value baked once, no automatic re-patching.
    pub fn static_value(dep_key: impl Into<String>, value: T) -> Self {
        Self {
            initial: value,
            dep_key: dep_key.into(),
            producer: BakedProducer::Static,
            patch_delay: PatchDelay::Immediate,
        }
    }

    /// Set the patch delay for this field (default: `Immediate`).
    pub fn patch_delay(mut self, delay: PatchDelay) -> Self {
        self.patch_delay = delay;
        self
    }
}

impl<T: Serialize + 'static> BakedProp<T> {
    /// Called by generated baked-route code. Extracts this field into a `BakedField`
    /// for JSON artifact construction and reverse-index registration.
    #[doc(hidden)]
    pub fn __into_baked_field(self, field_name: &'static str) -> BakedField {
        BakedField {
            field_name,
            dep_key: self.dep_key,
            patch_delay: self.patch_delay,
            initial_json: serde_json::to_value(&self.initial).unwrap_or_default(),
        }
    }
}

impl<T: fmt::Display> fmt::Display for BakedProp<T> {
    /// Renders the initial value. Askama calls this for `{{ field }}` in the shell template.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.initial.fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for BakedProp<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BakedProp({:?})", self.initial)
    }
}

impl<T: Serialize> serde::Serialize for BakedProp<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.initial.serialize(serializer)
    }
}

/// Carries the extracted baked field info from a `BakedProp<T>`.
/// Produced by `BakedProp::__into_baked_field` in generated route code.
pub struct BakedField {
    pub field_name: &'static str,
    pub dep_key: String,
    pub patch_delay: PatchDelay,
    pub initial_json: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::task::{Context, Poll};

    struct PendingUntilDropped {
        dropped: Arc<AtomicBool>,
    }

    impl Future for PendingUntilDropped {
        type Output = String;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    impl Drop for PendingUntilDropped {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_async_value_patch_stream_drops_pending_future() {
        let dropped = Arc::new(AtomicBool::new(false));
        let stream = __async_value_patch_stream(vec![(
            "count",
            Box::pin(PendingUntilDropped {
                dropped: Arc::clone(&dropped),
            }),
        )]);

        drop(stream);

        assert!(dropped.load(Ordering::SeqCst));
    }
}
