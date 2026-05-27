use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use pilcrow_core::PilcrowConfig;

use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::adapter::{PilcrowAdapter, TokioAdapter};
use crate::assets::assets::{
    react_islands_js_path, serve_react_islands_js, serve_silcrow_js, serve_solid_islands_js,
    silcrow_js_path, solid_islands_js_path,
};
use crate::dev::{DevState, dev_inject_layer, dev_reload_handler, spawn_css_watcher};
use crate::i18n::{I18nBundles, locale_middleware_impl};
use crate::image::handler::{ImageState, image_handler};
use crate::island_ssr::{IslandSsrWorker, replace_ssr_placeholders};
use crate::sw::{sw_handler, sw_inject_layer};

const REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum HTML body size buffered for SSR placeholder replacement (10 MiB).
/// Responses larger than this skip SSR processing rather than risking OOM.
const SSR_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;
/// Hard cap on Node SSR worker round-trip time. Prevents a hung Node process
/// from occupying a spawn_blocking thread slot indefinitely.
const SSR_WORKER_TIMEOUT: Duration = Duration::from_secs(10);
// local utility
fn web_bind_addr(config: &PilcrowConfig) -> String {
    format!("{}:{}", config.web.host, config.web.port)
}

pub async fn start(app: Router) {
    start_with_adapter(app, TokioAdapter).await;
}

/// Start the Pilcrow web server with a custom deployment [`PilcrowAdapter`].
///
/// The adapter receives the fully-wired `Router` and the bind address from
/// `Pilcrow.toml`. Use this when targeting a non-standard runtime (Lambda, etc.).
///
/// ```rust,ignore
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, MyAdapter).await;
/// ```
pub async fn start_with_adapter<A>(app: Router, adapter: A)
where
    A: PilcrowAdapter,
{
    let config = Arc::new(load_config_or_exit());
    let bind_addr = web_bind_addr(&config);
    let request_body_limit_bytes = config.web.request_body_limit_bytes;
    let http = reqwest::Client::new();

    // Apply [client] settings before any template render can call script_tag().
    crate::assets::assets::set_inline_runtime(config.client.inline_runtime);

    // Load i18n bundles when [i18n] is configured in Pilcrow.toml.
    let i18n_bundles: Option<I18nBundles> = if !config.i18n.locales.is_empty() {
        let locales_dir = std::env::current_dir()
            .unwrap_or_default()
            .join(&config.i18n.locales_dir);
        let bundles = I18nBundles::load(
            &locales_dir,
            &config.i18n.locales,
            &config.i18n.default_locale,
        );
        tracing::info!(
            "i18n: {} locale(s) loaded (default: {})",
            config.i18n.locales.len(),
            config.i18n.default_locale
        );
        Some(bundles)
    } else {
        None
    };

    let dev_mode = std::env::var("PILCROW_DEV").is_ok();
    let sw_enabled = config.service_worker.enabled && !dev_mode;
    let sw_strategy_dbg = format!("{:?}", config.service_worker.strategy);

    let silcrow_path = silcrow_js_path();
    let react_islands_path = react_islands_js_path();
    let solid_islands_path = solid_islands_js_path();
    let mut app = app
        .route(&silcrow_path, axum::routing::get(serve_silcrow_js))
        .route(
            &react_islands_path,
            axum::routing::get(serve_react_islands_js),
        )
        .route(
            &solid_islands_path,
            axum::routing::get(serve_solid_islands_js),
        );

    if sw_enabled {
        app = app.route("/sw.js", axum::routing::get(sw_handler));
    }

    if config.images.enabled {
        let image_state = ImageState {
            config: Arc::new(config.images.clone()),
            semaphore: Arc::new(tokio::sync::Semaphore::new(config.images.concurrency)),
        };
        app = app.route(
            "/_image",
            axum::routing::get(image_handler).with_state(image_state),
        );
        tracing::info!(
            "image optimization: enabled (cache: {})",
            config.images.cache_dir
        );
    }

    let dev_state = if dev_mode {
        let state = DevState::new();
        let src_dir = std::env::current_dir().unwrap_or_default();
        spawn_css_watcher(state.sender(), src_dir);
        app = app.route(
            "/__pilcrow/dev-reload",
            axum::routing::get(dev_reload_handler),
        );
        Some(state)
    } else {
        None
    };

    // Capture fsr config before config is moved into extension.
    #[cfg(feature = "live-props")]
    let fsr_config = config.fsr.clone();

    #[allow(unused_mut)]
    let mut app = app
        .layer(axum::Extension(config))
        .layer(axum::Extension(http));

    // live-props: register broadcast channel + optional DB store.
    #[cfg(feature = "live-props")]
    {
        use crate::live_props::{LiveBroadcast, LivePageStore};
        let live_broadcast = LiveBroadcast::new(256);
        app = app.layer(axum::Extension(live_broadcast));

        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            match sqlx::PgPool::connect(&db_url).await {
                Ok(pool) => {
                    let live_store = Arc::new(LivePageStore::new(pool));
                    app = app.layer(axum::Extension(live_store));
                    tracing::info!("live-props: connected to DATABASE_URL");
                }
                Err(err) => {
                    tracing::warn!("live-props: failed to connect to DATABASE_URL: {err}");
                }
            }
        }
    }

    // FSR: register broadcast channel, optional DB store, and embedded watcher.
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
                        // Merge field-level timers (explicit #[revalidate(N)]) with default timers
                        // (routes whose fields have no explicit revalidation — use global or 24h).
                        let default_secs = fsr_config.revalidate_seconds.unwrap_or(86_400);
                        let extra = crate::fsr::codegen_default_revalidate_routes()
                            .into_iter()
                            .map(|route| ScheduledInvalidation::new(
                                format!("{route}::__revalidate_default"),
                                std::time::Duration::from_secs(default_secs),
                            ));
                        let all_invalidations: Vec<ScheduledInvalidation> =
                            crate::fsr::codegen_scheduled_invalidations()
                                .into_iter()
                                .chain(extra)
                                .collect();
                        let watcher_cfg = WatcherConfig {
                            poll_interval_ms: fsr_config.poll_interval_ms,
                            promote_after_hits: fsr_config.promote_after_hits,
                            patch_debounce_secs: fsr_config.patch_debounce_secs,
                            purge_after_seconds: fsr_config.purge_after_seconds,
                            scheduled_invalidations: all_invalidations,
                            idle_evict_secs: fsr_config.idle_evict_secs,
                            idle_threshold_secs: fsr_config.idle_threshold_secs,
                        };

                        // If Redis is configured, use the pub/sub-driven watcher and
                        // spawn a bridge that forwards pilcrow:patch events to the
                        // in-process broadcast channel (for single-pod SSE clients).
                        #[cfg(feature = "live-props-redis")]
                        let redis_started = {
                            use crate::fsr::cache::RedisCache;
                            use crate::fsr::watcher::spawn_embedded_watcher_redis;

                            if let Some(ref redis_url) = fsr_config.redis_url {
                                match RedisCache::connect(redis_url).await {
                                    Ok(cache) => {
                                        let redis = Arc::new(
                                            cache.with_artifact_ttl(fsr_config.artifact_ttl_secs),
                                        );
                                        app = app.layer(axum::Extension(Arc::clone(&redis)));

                                        // Rebuild fsr_store with Redis attached so that
                                        // invalidate_dep_key / invalidate_route publish
                                        // to pilcrow:invalidate immediately.
                                        let fsr_store = Arc::new(
                                            fsr_store.with_redis_attached(Arc::clone(&redis)),
                                        );
                                        // Re-register the upgraded store as an Extension.
                                        app = app.layer(axum::Extension(Arc::clone(&fsr_store)));

                                        // Bridge: Redis pilcrow:patch → in-process broadcast.
                                        spawn_redis_patch_bridge(
                                            Arc::clone(&redis),
                                            (*fsr_tx).clone(),
                                        );

                                        spawn_embedded_watcher_redis(
                                            Arc::clone(&fsr_store),
                                            watcher_cfg.clone(),
                                            Some((*fsr_tx).clone()),
                                            Arc::clone(&redis),
                                        );
                                        tracing::info!(
                                            "FSR: embedded watcher started (Redis pub/sub, max_connections: {}, ttl: {}s)",
                                            fsr_config.max_sse_connections,
                                            fsr_config.connection_ttl_secs,
                                        );
                                        true
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            error = %e,
                                            url = redis_url,
                                            "FSR: failed to connect to Redis — falling back to polling"
                                        );
                                        false
                                    }
                                }
                            } else {
                                false
                            }
                        };

                        #[cfg(not(feature = "live-props-redis"))]
                        let redis_started = false;

                        if !redis_started {
                            spawn_embedded_watcher(
                                Arc::clone(&fsr_store),
                                watcher_cfg,
                                Some((*fsr_tx).clone()),
                            );
                            tracing::info!(
                                "FSR: embedded watcher started (polling {}ms, max_connections: {}, ttl: {}s)",
                                fsr_config.poll_interval_ms,
                                fsr_config.max_sse_connections,
                                fsr_config.connection_ttl_secs,
                            );
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("FSR: failed to connect to DATABASE_URL for FsrStore: {err}");
                }
            }
        }
    }

    let mut app = app
        .layer(axum::extract::DefaultBodyLimit::max(request_body_limit_bytes))
        .layer(axum::middleware::from_fn(request_timeout_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new());

    // i18n: inject bundles as extension (for handler access via Req.i18n) and add the
    // locale-prefix rewrite middleware (outermost, so it runs before routing).
    if let Some(bundles) = i18n_bundles {
        let b = bundles.clone();
        app = app
            .layer(axum::Extension(bundles))
            .layer(axum::middleware::from_fn(move |req, next| {
                locale_middleware_impl(req, next, b.clone())
            }));
    }

    let app = app;

    let app = if let Some(state) = dev_state {
        tracing::info!("dev mode: live reload + CSS hot swap active");
        app.layer(axum::Extension(state))
            .layer(axum::middleware::from_fn(dev_inject_layer))
    } else {
        app
    };

    let app = if sw_enabled {
        tracing::info!("service worker: enabled (strategy: {sw_strategy_dbg})");
        app.layer(axum::middleware::from_fn(sw_inject_layer))
    } else {
        app
    };

    // SSR placeholder middleware — no-op when no worker Extension is present.
    let app = app.layer(axum::middleware::from_fn(island_ssr_middleware));

    adapter.serve(&bind_addr, app, Box::new(|actual| {
        // Normalize 0.0.0.0 (all-interfaces bind) to loopback for local prebake requests.
        let base = if let Some(port) = actual.strip_prefix("0.0.0.0:") {
            format!("http://127.0.0.1:{port}")
        } else {
            format!("http://{actual}")
        };
        crate::prebake::set_local_base(base);
    })).await;
}

/// Enforce a per-request timeout and log the path when it fires.
async fn request_timeout_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_owned();
    match tokio::time::timeout(
        Duration::from_secs(REQUEST_TIMEOUT_SECS),
        next.run(req),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!(path = %path, timeout_secs = REQUEST_TIMEOUT_SECS, "request timed out");
            StatusCode::REQUEST_TIMEOUT.into_response()
        }
    }
}

/// Replace `__PILCROW_REACT_SSR_{id}__` placeholders in HTML responses.
/// Reads props from the surrounding `data-prop-*` attributes and sends them to
/// the persistent Node worker for rendering. No-op when no worker is registered.
async fn island_ssr_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let worker = req
        .extensions()
        .get::<Arc<std::sync::Mutex<IslandSsrWorker>>>()
        .cloned();

    let response = next.run(req).await;

    let Some(worker) = worker else {
        return response;
    };

    let is_html = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/html"))
        .unwrap_or(false);

    if !is_html {
        return response;
    }

    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, SSR_BODY_LIMIT_BYTES).await {
        Ok(b) => b,
        Err(_) => {
            tracing::warn!("island_ssr_middleware: response body exceeded SSR_BODY_LIMIT_BYTES or read failed; SSR skipped");
            return axum::response::Response::from_parts(parts, axum::body::Body::empty());
        }
    };

    let html = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => {
            return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes));
        }
    };

    if !html.contains("__PILCROW_REACT_SSR_") {
        return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes));
    }

    // `replace_ssr_placeholders` communicates with a Node.js process over
    // stdin/stdout (blocking I/O). Run it on the blocking thread pool to
    // avoid stalling Tokio workers and serialising all SSR requests.
    let html_owned = html.to_owned();
    let task = tokio::task::spawn_blocking(move || {
        replace_ssr_placeholders(&html_owned, &worker)
    });
    let replaced = match tokio::time::timeout(SSR_WORKER_TIMEOUT, task).await {
        Ok(Ok(html)) => html,
        Ok(Err(panic)) => {
            tracing::error!("island SSR worker panicked: {:?}", panic);
            return axum::response::Response::from_parts(parts, axum::body::Body::empty());
        }
        Err(_elapsed) => {
            tracing::error!("island SSR worker timed out after {}s", SSR_WORKER_TIMEOUT.as_secs());
            return axum::response::Response::from_parts(parts, axum::body::Body::empty());
        }
    };
    axum::response::Response::from_parts(parts, axum::body::Body::from(replaced))
}

/// Subscribe to Redis `pilcrow:patch` and forward each event to the in-process
/// broadcast channel so SSE clients on this pod receive multi-pod patch events.
#[cfg(feature = "live-props-redis")]
fn spawn_redis_patch_bridge(
    redis: Arc<crate::fsr::cache::RedisCache>,
    tx: crate::fsr::WatcherEventTx,
) {
    use futures_util::StreamExt as _;
    tokio::spawn(async move {
        loop {
            match redis.client().get_async_pubsub().await {
                Ok(mut pubsub) => {
                    if let Err(e) = pubsub.subscribe("pilcrow:patch").await {
                        tracing::warn!(error = %e, "FSR patch bridge: subscribe failed, retrying in 1s");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    let stream = pubsub.on_message();
                    tokio::pin!(stream);
                    while let Some(msg) = stream.next().await {
                        let payload: String = match msg.get_payload() {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        if let Ok(patch) =
                            serde_json::from_str::<crate::fsr::cache::PatchPayload>(&payload)
                        {
                            let _ = tx.send(crate::fsr::watcher::SlotPatch {
                                route: patch.route,
                                slot: patch.slot,
                                value: patch.value,
                            });
                        }
                    }
                    tracing::warn!("FSR patch bridge: Redis connection closed, reconnecting");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "FSR patch bridge: Redis connection failed, retrying in 1s");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}

fn load_config_or_exit() -> PilcrowConfig {
    match PilcrowConfig::load_from_current_dir() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "failed to load Pilcrow configuration");
            eprintln!("pilcrow: failed to load Pilcrow configuration: {err}");
            std::process::exit(1);
        }
    }
}
