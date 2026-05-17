//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use runtime::response::response::{
    ActionResult, ActionResultExt, ErrorResponse, FormErrorItem, FormErrors, JsonResponse,
    NavigateResponse, ResponseExt, ToastLevel,
};
pub use runtime::response::response::{form_errors, json, navigate, ok, redirect, status};

// ── Request handling ─────────────────────────────────────────
pub use runtime::{FormMap, Locals, Next, Page, Req, Res};
pub use runtime::SilcrowMutationId;

// ── Status & response primitives ─────────────────────────────
pub use runtime::Response;
pub use runtime::StatusCode;

// ── SSE ──────────────────────────────────────────────────────
pub use runtime::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, sse_raw, sse_stream,
    watch,
};

// ── WebSocket ────────────────────────────────────────────────
pub use runtime::{WsEvent, WsRoute, WsStream};

// ── Generated routes ─────────────────────────────────────────
pub use runtime::{
    GeneratedApiRoute, GeneratedPageRoute, generated_api_routes, generated_routes, pilcrow_router,
    register_generated_api_routes, register_generated_routes,
};

// ── Assets ───────────────────────────────────────────────────
pub use runtime::assets;

// ── Domain primitives (from pilcrow-core) ────────────────────
pub use pilcrow_core::{
    ApiEnvelope, AppError, AppResult, BackendConfig, HookError, Meta, PilcrowConfig, WebConfig,
};

pub use pilcrow_client::PilcrowClient;
pub use pilcrow_macros::handler;
pub use runtime::island_ssr::IslandSsrWorker;
pub use runtime::{AdapterFuture, PilcrowAdapter, TokioAdapter};
pub use runtime::{start, start_with_adapter, start_with_prerender};

/// FSR (Field-Selective Rendering) developer-facing surface.
///
/// Import all live types and macros with: `use pilcrow::live::*;`
#[cfg(feature = "live-props")]
pub mod live;

/// Experimental APIs that may change before stabilization.
#[cfg(feature = "experimental-baked-pages")]
pub mod experimental {
    /// Developer-facing helpers for opt-in baked page serving.
    pub mod baked_pages {
        pub use runtime::baked_pages::*;

        use axum::{
            http::header::{HeaderName, HeaderValue},
            response::{Html, IntoResponse, Response},
        };
        use std::io;

        const BAKED_HEADER: HeaderName = HeaderName::from_static("x-pilcrow-baked");
        const SSR_LOAD_HEADER: HeaderName = HeaderName::from_static("x-pilcrow-ssr-load");

        /// Tiny route-local wrapper for opting a handler into baked serving.
        ///
        /// The route still owns its source-of-truth render/load function. This helper
        /// only checks the baked store first, writes lazy artifacts on misses, and
        /// returns a normal Axum response.
        #[derive(Debug, Clone)]
        pub struct BakedRoute {
            store: BakedPageStore,
        }

        impl BakedRoute {
            pub fn new(store: BakedPageStore) -> Self {
                Self { store }
            }

            pub fn store(&self) -> &BakedPageStore {
                &self.store
            }

            pub fn serve<F>(&self, page: BakedPage, render: F) -> io::Result<Response>
            where
                F: FnOnce() -> io::Result<BakedRenderedOutput>,
            {
                let outcome = self.store.get_or_render(page, render)?;
                Ok(baked_html_response(outcome))
            }
        }

        /// Register all `BakedField`s produced by `BakedProp` into a patch registry.
        ///
        /// Called at app startup (generated code) to wire dep-key → recompute functions.
        /// Each field's `BakedProducer` supplies the value; the registry stores it for
        /// `patch_dependency()` calls when a dep key fires.
        pub fn register_baked_fields(_registry: &mut BakedPatchRegistry, _fields: Vec<BakedField>) {
            // Codegen-driven: field producers are registered by the generated baked-route
            // infrastructure in app_module.rs. This function is the stable public signature.
        }

        pub fn baked_html_response(outcome: BakedServeOutcome) -> Response {
            let baked_state = outcome.state.as_str();
            let render_state = outcome.state.render_state();
            let mut response = Html(outcome.html).into_response();
            response
                .headers_mut()
                .insert(BAKED_HEADER, HeaderValue::from_static(baked_state));
            response
                .headers_mut()
                .insert(SSR_LOAD_HEADER, HeaderValue::from_static(render_state));
            response
        }
    }
}

#[cfg(all(test, feature = "experimental-baked-pages"))]
mod baked_page_tests {
    use super::experimental::baked_pages::{
        BakedPage, BakedPagePaths, BakedPageStore, BakedPatchRegistry, BakedRenderedOutput,
        BakedRoute, BakedSlot, DependencyConfig,
    };
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use http_body_util::BodyExt;
    use serde_json::json;
    use std::{
        io,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use tower::ServiceExt;

    fn shell_html() -> &'static str {
        r#"<main><span data-pilcrow-slot="status">Loading</span></main>"#
    }

    fn make_page(store: &BakedPageStore, concrete_path: &str, threshold: Option<u32>) -> BakedPage {
        BakedPage::new(
            BakedPagePaths::new(
                "/tickets/:id",
                concrete_path,
                store
                    .shell_path("/tickets/:id")
                    .to_string_lossy()
                    .to_string(),
                store.json_path(concrete_path).to_string_lossy().to_string(),
                store
                    .metadata_path(concrete_path)
                    .to_string_lossy()
                    .to_string(),
            ),
            vec![BakedSlot::text("status")],
            vec![DependencyConfig::immediate("TicketStatus:123", "status")],
            threshold,
            "v1",
        )
    }

    fn render_output(status: &str) -> io::Result<BakedRenderedOutput> {
        Ok(BakedRenderedOutput::new(
            shell_html(),
            json!({ "status": status }),
            "v1",
        ))
    }

    #[test]
    fn below_threshold_renders_unbaked_with_correct_headers() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let route = BakedRoute::new(store.clone());
        let page = make_page(&store, "/tickets/123", Some(5));
        store.write_page(&page).unwrap();

        let response = route.serve(page, || render_output("Open")).unwrap();

        assert_eq!(response.headers()["x-pilcrow-baked"], "never-bake-rendered");
        assert_eq!(response.headers()["x-pilcrow-ssr-load"], "ran");
        assert!(store.read_json("/tickets/123").unwrap().is_none());
    }

    #[tokio::test]
    async fn at_threshold_bakes_then_second_request_hits_cache() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let route = BakedRoute::new(store.clone());
        let render_count = Arc::new(AtomicUsize::new(0));

        let app = Router::new().route(
            "/tickets/123",
            get({
                let route = route.clone();
                let store = store.clone();
                let render_count = render_count.clone();
                move || {
                    let route = route.clone();
                    let store = store.clone();
                    let render_count = render_count.clone();
                    async move {
                        let page = store.read_page("/tickets/123").unwrap().unwrap_or_else(|| {
                            let p = make_page(&store, "/tickets/123", Some(1));
                            store.write_page(&p).unwrap();
                            p
                        });
                        route
                            .serve(page, || {
                                render_count.fetch_add(1, Ordering::SeqCst);
                                render_output("Open")
                            })
                            .unwrap()
                    }
                }
            }),
        );

        // Seed page so it exists before first request.
        let seed = make_page(&store, "/tickets/123", Some(1));
        store.write_page(&seed).unwrap();

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/tickets/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()["x-pilcrow-baked"], "miss-rendered");
        assert_eq!(first.headers()["x-pilcrow-ssr-load"], "ran");
        assert_eq!(render_count.load(Ordering::SeqCst), 1);

        let second = app
            .oneshot(
                Request::builder()
                    .uri("/tickets/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.headers()["x-pilcrow-baked"], "hit");
        assert_eq!(second.headers()["x-pilcrow-ssr-load"], "skipped");
        let body = response_text(second).await;
        assert!(body.contains("Open"));
        assert_eq!(render_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ticket_flow_patch_updates_json_without_re_render() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let route = BakedRoute::new(store.clone());
        let status = Arc::new(std::sync::Mutex::new(String::from("Open")));
        let render_count = Arc::new(AtomicUsize::new(0));

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_field_recompute("TicketStatus:123", "status", {
            let status = status.clone();
            move |_key| Ok(json!(status.lock().unwrap().clone()))
        });
        let registry = Arc::new(registry);

        // Seed page with threshold=1 so first request bakes.
        let mut seed = make_page(&store, "/tickets/123", Some(1));
        seed.hit_count = 0;
        store.write_page(&seed).unwrap();
        store.upsert_reverse_index_page(&seed).unwrap();

        let app = Router::new()
            .route(
                "/tickets/123",
                get({
                    let route = route.clone();
                    let store = store.clone();
                    let status = status.clone();
                    let render_count = render_count.clone();
                    move || {
                        let route = route.clone();
                        let store = store.clone();
                        let status = status.clone();
                        let render_count = render_count.clone();
                        async move {
                            let page = store.read_page("/tickets/123").unwrap().unwrap();
                            route
                                .serve(page, || {
                                    render_count.fetch_add(1, Ordering::SeqCst);
                                    let s = status.lock().unwrap().clone();
                                    Ok(BakedRenderedOutput::new(
                                        shell_html(),
                                        json!({ "status": s }),
                                        "v1",
                                    ))
                                })
                                .unwrap()
                        }
                    }
                }),
            )
            .route(
                "/tickets/123/close",
                axum::routing::post({
                    let status = status.clone();
                    let registry = registry.clone();
                    move || {
                        let status = status.clone();
                        let registry = registry.clone();
                        async move {
                            *status.lock().unwrap() = String::from("Closed");
                            registry.patch_dependency("TicketStatus:123").unwrap();
                            StatusCode::OK
                        }
                    }
                }),
            );

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/tickets/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.headers()["x-pilcrow-baked"], "miss-rendered");
        assert!(response_text(first).await.contains("Open"));
        assert_eq!(render_count.load(Ordering::SeqCst), 1);

        let second = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/tickets/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.headers()["x-pilcrow-baked"], "hit");
        assert!(response_text(second).await.contains("Open"));
        assert_eq!(render_count.load(Ordering::SeqCst), 1);

        let close = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tickets/123/close")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(close.status(), StatusCode::OK);

        let after_patch = app
            .oneshot(
                Request::builder()
                    .uri("/tickets/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after_patch.headers()["x-pilcrow-baked"], "hit");
        assert!(response_text(after_patch).await.contains("Closed"));
        assert_eq!(render_count.load(Ordering::SeqCst), 1);
    }

    async fn response_text(response: axum::response::Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}

/// Platform deployment adapters.
///
/// Each adapter implements [`PilcrowAdapter`] and is passed to
/// [`start_with_adapter`] to target a specific hosting platform.
///
/// # Available adapters
///
/// | Adapter | Platform | Feature flag |
/// |---|---|---|
/// | [`TokioAdapter`] | Local / VPS / bare metal | *(default)* |
/// | [`adapters::PortEnvAdapter`] | Any `PORT`-env platform | *(default)* |
/// | [`adapters::FlyAdapter`] | Fly.io | *(default)* |
/// | [`adapters::RailwayAdapter`] | Railway | *(default)* |
/// | [`adapters::CloudRunAdapter`] | Google Cloud Run | *(default)* |
/// | [`adapters::RenderAdapter`] | Render | *(default)* |
/// | [`adapters::VercelAdapter`] | Vercel (long-running) | *(default)* |
/// | [`adapters::LambdaAdapter`] | AWS Lambda / Vercel Functions / Netlify | `lambda` |
pub mod adapters {
    pub use runtime::adapters::{
        CloudRunAdapter, FlyAdapter, PortEnvAdapter, RailwayAdapter, RenderAdapter, VercelAdapter,
    };

    #[cfg(feature = "lambda")]
    pub use runtime::adapters::LambdaAdapter;
}

// ── Live props (old per-route SSE system) ────────────────────
pub use runtime::{__live_props_response, LiveProp, LiveTarget};

// ── SSG cache ────────────────────────────────────────────────
pub use runtime::{IsrCache, IsrCacheState, IsrHandle};
// ── i18n ─────────────────────────────────────────────────────
pub use runtime::{FmtHelper, I18nBundles};

// ── Doc-hidden re-exports for generated code ────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use pilcrow_client;
#[doc(hidden)]
pub use runtime::__isr_cache_key;
#[doc(hidden)]
pub use runtime::csrf_middleware as __csrf_middleware;
#[doc(hidden)]
pub use runtime::tokio;
#[doc(hidden)]
pub use tracing;

/// Include the auto-generated Pilcrow app module and expose `pilcrow_router()`.
///
/// This macro eliminates all manual route wiring. Place it at the top of your
/// `main.rs` and use `pilcrow_router()` to get a fully-wired `axum::Router`:
///
/// ```ignore
/// pilcrow_web::pilcrow_app!();
///
/// #[tokio::main]
/// async fn main() {
///     let app = pilcrow_router();
///     pilcrow_web::start(app).await
/// }
/// ```
#[macro_export]
macro_rules! pilcrow_app {
    () => {
        // API mod tree at crate root so `mod api { pub mod health; }` resolves to src/api/health.rs
        include!(concat!(env!("OUT_DIR"), "/generated_api_mods.rs"));

        mod __pilcrow_app {
            include!(concat!(env!("OUT_DIR"), "/generated_app.rs"));
        }

        // Typed route helpers: `routes::products_id("42")` → `"/products/42"`
        include!(concat!(env!("OUT_DIR"), "/generated_typed_routes.rs"));

        // Typed env structs: `env::Private::load()?` → `env::Private { database_url: .. }`
        include!(concat!(env!("OUT_DIR"), "/generated_env.rs"));

        // Typed i18n helpers: `t::greeting(&req, &name)` → `String`
        include!(concat!(env!("OUT_DIR"), "/generated_i18n.rs"));

        fn pilcrow_router() -> ::pilcrow_web::axum::Router {
            __pilcrow_app::build_router()
        }

        /// Start the Pilcrow server, pre-rendering any SSG pages before accepting connections.
        ///
        /// Use `pilcrow_start(pilcrow_router()).await` in place of
        /// `pilcrow_web::start(pilcrow_router()).await` when your app has pages with
        /// `pub const PRERENDER: bool = true`.
        async fn pilcrow_start(router: ::pilcrow_web::axum::Router) {
            // Wire up the React SSR Node worker when [client.react] ssr = true.
            let config =
                ::pilcrow_web::PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml");
            let router = {
                let bundles = __pilcrow_app::__pilcrow_ssr_bundles();
                if config.client.react.ssr && !bundles.is_empty() {
                    match ::pilcrow_web::IslandSsrWorker::spawn_with_sources(
                        bundles,
                        &config.client.react.node_bin,
                    ) {
                        Ok(worker) => router.layer(::pilcrow_web::axum::Extension(
                            ::std::sync::Arc::new(::std::sync::Mutex::new(worker)),
                        )),
                        Err(e) => {
                            eprintln!("[pilcrow] failed to spawn React SSR worker: {e}");
                            router
                        }
                    }
                } else {
                    router
                }
            };
            __pilcrow_app::__pilcrow_init().await;
            ::pilcrow_web::start_with_prerender(router, |cache| async move {
                __pilcrow_app::__pilcrow_prerender_all(&cache).await
            })
            .await;
        }

    };
}
