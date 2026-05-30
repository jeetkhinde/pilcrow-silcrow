//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use runtime::response::response::{
    ActionResult, ActionResultExt, ErrorResponse, FormErrorItem, FormErrors, JsonResponse,
    NavigateResponse, ResponseExt, ToastLevel,
};
pub use runtime::response::response::{form_errors, json, navigate, ok, redirect, status};

// ── Request handling ─────────────────────────────────────────
pub use runtime::SilcrowMutationId;
pub use runtime::{FormMap, Locals, Next, Page, Req, Res};

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

// ── Layout-aware navigation ───────────────────────────────────
pub use runtime::extract_ps_fragment;

// ── Domain primitives (from pilcrow-core) ────────────────────
pub use pilcrow_core::{
    ApiEnvelope, AppError, AppResult, BackendConfig, HookError, Meta, PilcrowConfig, StartupError,
    WebConfig,
};

pub use pilcrow_client::PilcrowClient;
pub use pilcrow_macros::handler;
pub use runtime::island_ssr::IslandSsrWorker;
pub use runtime::{AdapterFuture, PilcrowAdapter, TokioAdapter};
pub use runtime::{start, start_with_adapter, try_start, try_start_with_adapter};

/// FSR (Field-Selective Rendering) developer-facing surface.
///
/// Import all live types and macros with: `use pilcrow::live::*;`
#[cfg(feature = "live-props")]
pub mod live;

#[cfg(feature = "live-props")]
pub use pilcrow_macros::PilcrowListRow;

// ── FSR codegen internals (used by generated app module) ─────
#[doc(hidden)]
#[cfg(feature = "live-props")]
pub use runtime::fsr::__register_codegen_scheduled_invalidations;
#[doc(hidden)]
#[cfg(feature = "live-props")]
pub use runtime::fsr::__register_codegen_default_revalidate_routes;
#[cfg(feature = "live-props")]
pub use runtime::fsr::ScheduledInvalidation;

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

// ── i18n ─────────────────────────────────────────────────────
pub use runtime::{FmtHelper, I18nBundles};

// ── Doc-hidden re-exports for generated code ────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use pilcrow_client;
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
            ::pilcrow_web::start(router).await;
        }
    };
}
