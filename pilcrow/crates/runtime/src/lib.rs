// ./src/lib.rs

pub mod adapter;
pub mod adapters;
pub mod assets;
#[cfg(feature = "experimental-baked-pages")]
pub mod baked_pages;
pub mod context;
pub mod csrf;
pub mod deferred;
pub(crate) mod dev;
#[cfg(feature = "live-props")]
pub mod fsr;
pub mod generated_routes;
pub mod i18n;
pub mod image;
pub mod island_ssr;
pub mod isr;
#[cfg(feature = "live-props")]
pub mod live_props;
pub mod middleware;
pub mod response;
pub mod sse;
pub mod start;
pub(crate) mod sw;
pub mod validator;
pub mod ws;
pub use adapter::{AdapterFuture, PilcrowAdapter, TokioAdapter};
pub use start::{export, start, start_with_adapter, start_with_prerender};
// ── Core API re-exports ──────────────────────────────────────
pub use axum::http::StatusCode;
pub use axum::response::Response;
pub use context::{FormMap, Locals, Page, Req, Res};
pub use csrf::csrf_middleware;
pub use generated_routes::{
    GeneratedApiRoute, GeneratedPageRoute, generated_api_routes, generated_routes, pilcrow_router,
    register_generated_api_routes, register_generated_routes,
};
pub use middleware::Next;
pub use pilcrow_core::HookError;
pub use pilcrow_macros::sse;
pub use response::headers::SilcrowMutationId;
pub use response::response::ToastLevel;
pub use response::response::{
    ActionResult, ActionResultExt, ErrorResponse, FormErrorItem, FormErrors, JsonResponse,
    NavigateResponse, ResponseExt, form_errors, json, navigate, ok, redirect, status,
};
pub use sse::watch;
pub use sse::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, sse_raw, sse_stream,
};
pub use ws::ws::{WsEvent, WsRoute, WsStream};

// ── Available but not primary API ────────────────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use response::response::html;

pub use deferred::{
    __async_html_patch_stream, __async_value_patch_stream, __live_props_response,
    __serialize_async_value, __serialize_page_props, __streaming_props_response, AsyncHtml,
    AsyncHtmlPatch, AsyncValue, AsyncValuePatch, LiveProp, LiveTarget, async_response_combined,
    async_value_response,
};
// ── ISR ──────────────────────────────────────────────────────
pub use isr::{__isr_cache_key, CacheEntrySnapshot, IsrCache, IsrCacheState, IsrHandle};
// ── Validation ───────────────────────────────────────────────
pub use context::ReqBuilder;
pub use validator::Validator;
// ── i18n ─────────────────────────────────────────────────────
pub use i18n::{FmtHelper, I18nBundles};

// ── Internal helpers (used by ws.rs, macros, generated code) ─
pub(crate) use sse::serialize_or_null;
#[doc(hidden)]
pub use tokio;
