use std::convert::Infallible;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

// ── Dev events ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) enum DevEvent {
    CssReload { path: String },
}

impl DevEvent {
    fn into_sse(self) -> Event {
        match self {
            DevEvent::CssReload { path } => Event::default().event("custom").data(
                serde_json::json!({
                    "event": "css-reload",
                    "data": { "path": path }
                })
                .to_string(),
            ),
        }
    }
}

// ── DevState (shared across SSE connections) ───────────────────

/// Shared dev-mode state. Injected as an axum `Extension` in dev builds.
/// Each SSE connection subscribes to `tx` to receive live events.
#[derive(Clone)]
pub(crate) struct DevState {
    tx: broadcast::Sender<DevEvent>,
}

impl DevState {
    pub(crate) fn new() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self { tx }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<DevEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn sender(&self) -> broadcast::Sender<DevEvent> {
        self.tx.clone()
    }
}

// ── SSE stream ─────────────────────────────────────────────────

/// Stream for `GET /__pilcrow/dev-reload`.
///
/// Yields a `custom/reload` event on first poll (for the reconnect-reload pattern),
/// then forwards any [`DevEvent`]s broadcast by the CSS file watcher.
/// `BroadcastStream` / `BroadcastStreamRecvError` lags are silently skipped.
pub(crate) struct DevSseStream {
    /// `true` until the initial reconnect-reload event has been yielded.
    initial: bool,
    inner: BroadcastStream<DevEvent>,
}

impl Stream for DevSseStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.initial {
            this.initial = false;
            return Poll::Ready(Some(Ok(Event::default()
                .event("custom")
                .data(r#"{"event":"reload","data":{}}"#))));
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(ev))) => return Poll::Ready(Some(Ok(ev.into_sse()))),
                Poll::Ready(Some(Err(_lagged))) => continue,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub async fn dev_reload_handler(
    axum::Extension(state): axum::Extension<DevState>,
) -> Sse<DevSseStream> {
    Sse::new(DevSseStream {
        initial: true,
        inner: BroadcastStream::new(state.subscribe()),
    })
    .keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

// ── CSS file watcher ───────────────────────────────────────────

/// Spawns a background OS thread that watches `src_dir` for CSS file changes
/// and broadcasts [`DevEvent::CssReload`] to all connected SSE clients.
///
/// Runs as a plain OS thread (not tokio) to avoid async watcher complexity.
/// The broadcast sender is `Send + Clone` so crossing the thread boundary is safe.
pub(crate) fn spawn_css_watcher(tx: broadcast::Sender<DevEvent>, src_dir: PathBuf) {
    std::thread::spawn(move || {
        use notify::{EventKind, RecursiveMode, Watcher};

        let (sync_tx, sync_rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(sync_tx) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("dev: CSS watcher init failed: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&src_dir, RecursiveMode::Recursive) {
            tracing::warn!("dev: CSS watcher watch({}) failed: {e}", src_dir.display());
            return;
        }

        tracing::debug!("dev: CSS watcher active on {}", src_dir.display());

        for result in sync_rx {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("dev: CSS watch error: {e}");
                    continue;
                }
            };

            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                continue;
            }

            for path in &event.paths {
                if path.extension().map(|e| e == "css").unwrap_or(false) {
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "styles.css".to_string());

                    tracing::debug!("dev: CSS changed — {filename}");
                    let _ = tx.send(DevEvent::CssReload { path: filename });
                }
            }
        }
    });
}

// ── HTML injection middleware ───────────────────────────────────

/// Injected before `</body>` in every `text/html` response in dev mode.
///
/// - The hidden `<div s-sse>` opens the silcrow SSE connection. silcrow reconnects
///   automatically with exponential backoff after a server restart.
/// - `silcrow:sse:reload` — first connect sets `c=true` (no-op); reconnect calls `location.reload()`.
/// - `silcrow:sse:css-reload` — reinjects the matching `<link>` by filename with `?t=` cache-bust.
/// - `silcrow:live:disconnect` on the element — shows a "building…" banner after 2 s;
///   `silcrow:live:connect` clears it. These two events act as build-start / build-end signals:
///   while the server is recompiling the connection is down, and the banner communicates that.
const DEV_INJECTION: &str = concat!(
    r#"<div id="__pilcrow_dev" s-sse="/__pilcrow/dev-reload" style="display:none"></div>"#,
    "<script>(function(){",
    "var c=false,banner=null,timer=null;",
    // reload on reconnect
    "document.addEventListener('silcrow:sse:reload',function(){",
    "if(c){location.reload();}else{c=true;}",
    "});",
    // css hot swap — match by filename
    "document.addEventListener('silcrow:sse:css-reload',function(e){",
    "var f=e.detail&&e.detail.path;if(!f)return;",
    "document.querySelectorAll('link[rel=\"stylesheet\"]').forEach(function(l){",
    "var lf=l.href.split('?')[0].split('/').pop();",
    "if(lf===f){var n=l.cloneNode();n.href=l.href.split('?')[0]+'?t='+Date.now();",
    "l.after(n);n.onload=function(){l.remove();};}",
    "});",
    "});",
    // build status banner via live:connect / live:disconnect on the element
    "var el=document.getElementById('__pilcrow_dev');",
    "if(el){",
    "el.addEventListener('silcrow:live:disconnect',function(){",
    "timer=setTimeout(function(){",
    "if(banner)return;",
    "banner=document.createElement('div');",
    "Object.assign(banner.style,{position:'fixed',bottom:'1rem',right:'1rem',",
    "background:'#1a1a1a',color:'#e5e5e5',padding:'0.5rem 1rem',",
    "borderRadius:'0.375rem',fontFamily:'monospace',fontSize:'0.8125rem',",
    "zIndex:'9999',boxShadow:'0 2px 8px rgba(0,0,0,0.4)'});",
    "banner.textContent='pilcrow: building…';",
    "document.body.appendChild(banner);",
    "},2000);",
    "});",
    "el.addEventListener('silcrow:live:connect',function(){",
    "clearTimeout(timer);timer=null;",
    "if(banner){banner.remove();banner=null;}",
    "});",
    "}",
    "})();</script>",
);

pub async fn dev_inject_layer(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;

    let is_html = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/html"))
        .unwrap_or(false);

    if !is_html {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return axum::response::Response::from_parts(parts, axum::body::Body::empty()),
    };

    let mut html = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, DEV_INJECTION);
    } else {
        html.push_str(DEV_INJECTION);
    }

    parts.headers.remove(CONTENT_LENGTH);
    axum::response::Response::from_parts(parts, axum::body::Body::from(html))
}
