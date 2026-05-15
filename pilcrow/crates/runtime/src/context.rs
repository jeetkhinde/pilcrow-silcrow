use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use axum::{
    async_trait,
    extract::{Form, FromRequest, FromRequestParts, Path, Request},
    http::request::Parts,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use cookie::time::Duration;
use headers::HeaderMapExt;
use pilcrow_core::AppError;
use serde::Serialize;

use crate::i18n::{CurrentLocale, FmtHelper, I18nBundles};
use crate::isr::IsrHandle;
use crate::response::headers::*;
use crate::response::response::{ActionResult, BaseResponse, FormErrors, ToastLevel};

// ── Locals ────────────────────────────────────────────────────

/// Per-request typed store shared across all `load()` calls in the same request.
///
/// `Locals` uses `Arc<RwLock<...>>` so that cloning `Req` (which the framework
/// does when passing it to layout and page loads) still refers to the same underlying
/// map.  A layout's `load()` can write to `req.locals`, and the page's `load()` will
/// see it without re-fetching.
///
/// ```rust,ignore
/// // _layout.rs
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = auth::verify(&req.cookies).await?;
///     req.locals.set(user);
///     Ok(Props { ... })
/// }
///
/// // products.rs
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = req.locals.require::<User>()?;
///     Ok(Props { ... })
/// }
/// ```
#[derive(Clone, Default)]
pub struct Locals(Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>);

impl std::fmt::Debug for Locals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locals").finish_non_exhaustive()
    }
}

impl Locals {
    /// Store a value of type `T`. Overwrites any previous value of the same type.
    pub fn set<T: Send + Sync + 'static>(&self, value: T) {
        self.0
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieve a clone of the stored value of type `T`, or `None` if not set.
    pub fn get<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Like `get`, but returns `Err(AppError::Unauthorized)` when absent.
    ///
    /// ```rust,ignore
    /// let user = req.locals.require::<User>()?;
    /// ```
    pub fn require<T: Clone + Send + Sync + 'static>(&self) -> Result<T, AppError> {
        self.get::<T>().ok_or(AppError::Unauthorized)
    }

    /// `true` if a value of type `T` has been set.
    pub fn has<T: 'static>(&self) -> bool {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&TypeId::of::<T>())
    }
}

// ── Res ──────────────────────────────────────────────────────

/// Per-request response modifier available inside `load()` and `actions()`.
///
/// Lets handlers set response headers, cookies, and toasts without changing the
/// return type.  The framework applies accumulated modifications to the final
/// rendered `Response` after all handlers complete.
///
/// ```rust,ignore
/// pub async fn load(req: Req) -> AppResult<Props> {
///     req.res.no_cache();
///     req.res.with_toast("Welcome back!", ToastLevel::Info);
///     Ok(Props { ... })
/// }
/// ```
#[derive(Clone, Default)]
pub struct Res(Arc<Mutex<BaseResponse>>);

impl std::fmt::Debug for Res {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Res").finish_non_exhaustive()
    }
}

impl Res {
    fn state(&self) -> std::sync::MutexGuard<'_, BaseResponse> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Set an explicit HTTP status code on the rendered page response.
    pub fn with_status(&self, status: StatusCode) -> &Self {
        self.state().set_status(status);
        self
    }

    /// Append a raw response header.
    pub fn with_header(&self, key: &'static str, value: impl Into<String>) -> &Self {
        self.state().set_header(key, value);
        self
    }

    /// Add `silcrow-cache: no-cache` so silcrow.js skips the response cache.
    pub fn no_cache(&self) -> &Self {
        self.state().set_no_cache();
        self
    }

    /// Add a `Set-Cookie` header to the response.
    pub fn with_cookie(&self, cookie: Cookie<'static>) -> &Self {
        self.state().add_cookie(cookie);
        self
    }

    /// Queue a toast notification. The client reads this from the `silcrow_toasts` cookie.
    pub fn with_toast(&self, message: impl Into<String>, level: ToastLevel) -> &Self {
        self.state().add_toast(message, level);
        self
    }

    /// Fire a custom DOM event on the client via `silcrow-trigger`.
    /// Multiple calls accumulate — all named events are sent in a single header.
    pub fn trigger_event(&self, event_name: &str) -> &Self {
        self.state().add_trigger_event(event_name);
        self
    }

    /// Override the swap target selector via `silcrow-retarget`.
    pub fn retarget(&self, selector: &str) -> &Self {
        self.state().set_retarget(selector);
        self
    }

    /// Push a URL to the browser history via `silcrow-push`.
    pub fn push_history(&self, url: &str) -> &Self {
        self.state().set_push_history(url);
        self
    }

    /// Patch a secondary DOM target via `silcrow-patch`.
    /// Multiple calls accumulate — each `{target, data}` entry is carried in
    /// a single JSON-array header and applied in call order on the client.
    pub fn patch_target(&self, selector: &str, data: &impl Serialize) -> &Self {
        self.state().add_patch_target(selector, data);
        self
    }

    /// Like `patch_target` but tags the patch entry with a mutation id.
    ///
    /// Use when an optimistic mutation is in flight: pass `req.mutation_id()`
    /// here so silcrow.js confirms the pending mutation when the response arrives.
    pub fn patch_target_with_mutation(
        &self,
        selector: &str,
        data: &impl Serialize,
        mutation_id: &str,
    ) -> &Self {
        self.state()
            .add_patch_target_with_mutation(selector, data, mutation_id);
        self
    }

    /// Invalidate a DOM target's binding cache via `silcrow-invalidate`.
    /// Multiple calls accumulate — all selectors are carried in a single
    /// JSON-array header and invalidated in call order on the client.
    pub fn invalidate_target(&self, selector: &str) -> &Self {
        self.state().add_invalidate_target(selector);
        self
    }

    /// Trigger a client-side navigation via `silcrow-navigate`.
    pub fn client_navigate(&self, path: &str) -> &Self {
        self.state().set_client_navigate(path);
        self
    }

    /// Open an SSE connection on the client via `silcrow-sse`.
    pub fn sse(&self, path: impl AsRef<str>) -> &Self {
        self.state().set_sse(path.as_ref());
        self
    }

    /// Open a WebSocket connection on the client via `silcrow-ws`.
    pub fn ws(&self, path: impl AsRef<str>) -> &Self {
        self.state().set_ws(path.as_ref());
        self
    }

    /// Prevent the ISR middleware from writing the rendered HTML to the cache.
    ///
    /// Call this inside `load()` to serve a non-cacheable response (e.g. preview mode
    /// or admin views). The render still happens normally; only the cache write is skipped.
    ///
    /// ```rust,ignore
    /// pub async fn load(req: Req) -> AppResult<Props> {
    ///     if req.query.contains("preview") {
    ///         req.res.bypass_cache();
    ///     }
    ///     // ...
    /// }
    /// ```
    pub fn bypass_cache(&self) -> &Self {
        self.state().set_bypass_cache();
        self
    }

    /// Check the bypass-cache flag. Used by generated ISR handler code.
    #[doc(hidden)]
    pub fn __is_bypass_cache(&self) -> bool {
        self.state().is_bypass_cache()
    }

    /// Apply all accumulated modifications to an existing response.
    ///
    /// Called by generated handler code after rendering is complete.
    pub fn apply_to(&self, response: &mut Response) {
        self.state().apply_to_response(response);
    }
}

// ── Action parsing ───────────────────────────────────────────

/// Extract the named action from a raw URL query string (`?/<name>`).
///
/// Scans the query string for the first key that starts with `/` and returns
/// the URL-decoded remainder as the action name. Returns `None` when no such
/// key is present.
fn extract_action_from_query(raw_query: Option<&str>) -> Option<String> {
    let query = raw_query?;
    for pair in query.split('&') {
        let key = pair.split('=').next()?;
        if let Some(stripped) = key.strip_prefix('/') {
            return match urlencoding::decode(stripped) {
                Ok(decoded) => Some(decoded.into_owned()),
                Err(_) => Some(stripped.to_string()),
            };
        }
    }
    None
}

// ── FormMap ───────────────────────────────────────────────────

/// Multi-value URL-encoded map. Used for both the form body (`req.form`) and
/// the query string (`req.query`); both share the same repeated-key semantics
/// (e.g. `?tag=a&tag=b` or `<input name="tag" …>` twice).
#[derive(Debug, Default, Clone)]
pub struct FormMap(pub HashMap<String, Vec<String>>);

impl FormMap {
    /// First value for a key, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.first().map(String::as_str)
    }

    /// All values for a key (e.g. multiple checkboxes with the same name, or
    /// repeated query params like `?tag=a&tag=b`).
    pub fn get_all(&self, key: &str) -> &[String] {
        self.0.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `true` if the key appears at least once.
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterator over distinct keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Deserialize the map into a typed struct using `serde`.
    ///
    /// Repeated keys (e.g. `<select multiple>`) are represented as multiple
    /// values per key; `serde_urlencoded` handles this with `Vec<T>` fields.
    /// Absent keys return the field default when `#[serde(default)]` is applied.
    ///
    /// ```rust,ignore
    /// #[derive(serde::Deserialize)]
    /// struct CreateInput {
    ///     name: String,
    ///     price: f64,
    ///     #[serde(default)]
    ///     active: bool,
    /// }
    ///
    /// pub async fn create(req: Req) -> ActionResult {
    ///     let input: CreateInput = req.form.parse()?;
    ///     // ...
    ///     redirect("/items")
    /// }
    /// ```
    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<T, AppError> {
        let mut parts: Vec<String> = Vec::with_capacity(self.0.len());
        for (k, vs) in &self.0 {
            for v in vs {
                parts.push(format!(
                    "{}={}",
                    urlencoding::encode(k),
                    urlencoding::encode(v)
                ));
            }
        }
        let encoded = parts.join("&");
        serde_urlencoded::from_str::<T>(&encoded).map_err(|e| AppError::Validation(e.to_string()))
    }
}

/// Parse a raw query string into a multi-value [`FormMap`], stripping any
/// `?/<name>` action markers (those are read via [`Req::action`]).
///
/// Handles standard URL-encoded form semantics: `+` is decoded as space and
/// `%XX` sequences are percent-decoded. Empty pairs are ignored.
fn parse_query_multi(raw_query: Option<&str>) -> FormMap {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let Some(q) = raw_query else {
        return FormMap(out);
    };
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_val) = pair.split_once('=').unwrap_or((pair, ""));
        // `?/<name>` keys are action markers; readable via req.action().
        if raw_key.starts_with('/') {
            continue;
        }
        out.entry(decode_form_component(raw_key))
            .or_default()
            .push(decode_form_component(raw_val));
    }
    FormMap(out)
}

fn decode_form_component(s: &str) -> String {
    // `+` → space is form-urlencoded-specific (not covered by percent-decoding).
    let with_spaces = s.replace('+', " ");
    urlencoding::decode(&with_spaces)
        .map(|c| c.into_owned())
        .unwrap_or(with_spaces)
}

// ── Req ──────────────────────────────────────────────────────

/// Unified request context for both `load()` and named action handlers.
///
/// Replaces the old `PageContext` / `ActionContext` split — one type, same mental
/// model everywhere.  `req.form` is empty on GET requests; `req.res` accumulates
/// response side-effects (headers, cookies, toasts) that the framework applies
/// after the handler returns.
///
/// ```rust,ignore
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = req.locals.require::<User>()?;
///     req.res.no_cache();
///     Ok(Props { user })
/// }
///
/// // Named action — invoked when the client POSTs `?/create`.
/// pub async fn create(req: Req) -> ActionResult {
///     let name = req.form.get("name").unwrap_or("");
///     redirect("/items")
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Req {
    /// Named path capture groups. `/posts/[id]` → `{"id": "42"}`.
    pub params: HashMap<String, String>,
    /// Query string as a multi-value map (`?category=shoes&tag=a&tag=b` →
    /// `{"category": ["shoes"], "tag": ["a", "b"]}`). Use `.get(key)` for the
    /// first value, `.get_all(key)` for every value.
    ///
    /// The `?/name` action marker is stripped out of this map — read it via
    /// [`Req::action`] instead.
    pub query: FormMap,
    /// Parsed URL-encoded form body. Empty on GET requests.
    pub form: FormMap,
    /// Request cookies.
    pub cookies: CookieJar,
    /// Raw request headers.
    pub headers: HeaderMap,
    /// The current request path, e.g. `/products` or `/posts/42`.
    pub path: String,
    /// `true` when the request was sent by silcrow.js (has a `silcrow-target` header).
    ///
    /// Use `req.fail(form_errors)` instead of checking this directly — it handles
    /// the JSON-vs-flash decision automatically.
    pub is_enhanced: bool,
    /// Per-request typed store shared across all loads in this request.
    pub locals: Locals,
    /// Response modifier: set headers, cookies, toasts from inside any handler.
    pub res: Res,
    /// ISR cache handle. Use `req.cache.revalidate(path)` or
    /// `req.cache.revalidate_tag(tag)` to bust the cache from within an action.
    pub cache: IsrHandle,
    /// The detected locale for this request (e.g. `"en"`, `"de"`).
    ///
    /// Populated by the i18n middleware from the URL prefix. When i18n is not
    /// configured, this is an empty string. Use `t::key(&req, ...)` (generated typed
    /// helpers) or `req.__t("key", &[("arg", &val)])` for direct translation.
    pub locale: String,
    /// The named action, parsed from `?/<name>`. `None` when the URL has no
    /// action marker (default POST / GET). See [`Req::action`].
    action: Option<String>,
    /// Pre-loaded Fluent bundles. `None` when i18n is not configured.
    i18n: Option<I18nBundles>,
    /// Client-provided mutation id from `silcrow-mutation-id` header. `None` when absent.
    mutation_id: Option<String>,
}

/// Typed page context passed to `load(ctx: Page)` for dynamic file routes.
///
/// Routekit generates a page-local `Params` type from the route file path and a
/// `type Page = pilcrow_web::Page<Params>` alias inside the generated page module.
/// The underlying [`Req`] remains available through `ctx.req` and via deref.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub req: Req,
    pub params: T,
}

impl<T> Page<T>
where
    T: TryFrom<HashMap<String, String>, Error = AppError>,
{
    pub fn try_from_req(req: Req) -> Result<Self, AppError> {
        let params = T::try_from(req.params.clone())?;
        Ok(Self { req, params })
    }

    pub fn from_req(req: Req) -> Self {
        Self::try_from_req(req).expect("route params should be validated before load()")
    }
}

impl<T> std::ops::Deref for Page<T> {
    type Target = Req;

    fn deref(&self) -> &Self::Target {
        &self.req
    }
}

impl Req {
    /// Return the named action from the current request URL.
    ///
    /// The server parses `?/<name>` from the query string — matching the
    /// SvelteKit convention. Returns `""` when no action marker is present.
    ///
    /// ```rust,ignore
    /// // HTML: <form s-post="?/create">…</form>
    /// // Code-behind:
    /// pub async fn create(req: Req) -> ActionResult {
    ///     let name = req.form.get("name").unwrap_or("");
    ///     redirect("/items")
    /// }
    /// ```
    pub fn action(&self) -> &str {
        self.action.as_deref().unwrap_or("")
    }

    /// The client-side mutation id sent with this request, if any.
    ///
    /// Silcrow sets `silcrow-mutation-id` when an optimistic mutation is in flight.
    /// Pass this to `SilcrowEvent::patch(...).with_mutation_id(id)` so the client
    /// can confirm and retire the pending mutation.
    pub fn mutation_id(&self) -> Option<&str> {
        self.mutation_id.as_deref()
    }

    /// Return the smart form-error response for the current request context.
    ///
    /// - **Enhanced** (`req.is_enhanced == true`, i.e. silcrow.js sent this): returns
    ///   JSON that silcrow.js patches into the form via `:text`/`:show`/`:value` bindings.
    /// - **Plain browser POST**: serialises the errors into a short-lived
    ///   `silcrow_form_flash` cookie and issues a `303 → req.path` redirect.
    ///   The page's `load()` reads the flash with [`Req::take_form_flash`].
    ///
    /// ```rust,ignore
    /// pub async fn signup(req: Req) -> ActionResult {
    ///     let email = req.form.get("email").unwrap_or("");
    ///     if email.is_empty() {
    ///         return req.fail(form_errors()
    ///             .error("email", "Email is required")
    ///             .value("email", email));
    ///     }
    ///     redirect("/dashboard")
    /// }
    /// ```
    pub fn fail(&self, errors: FormErrors) -> ActionResult {
        if self.is_enhanced {
            Ok(axum::Json(errors).into_response())
        } else {
            let json = serde_json::to_string(&errors).unwrap_or_default();
            let encoded = urlencoding::encode(&json).into_owned();
            let flash_cookie = Cookie::build(("silcrow_form_flash", encoded))
                .path("/")
                .same_site(SameSite::Lax)
                .max_age(Duration::seconds(30))
                .build();
            let mut response = Redirect::to(&self.path).into_response();
            *response.status_mut() = StatusCode::SEE_OTHER;
            if let Ok(header_value) = HeaderValue::from_str(&flash_cookie.to_string()) {
                response
                    .headers_mut()
                    .append(header::SET_COOKIE, header_value);
            }
            Ok(response)
        }
    }

    /// Read and clear the form flash cookie set by [`Req::fail`] on a
    /// non-enhanced (native browser) POST that failed validation.
    ///
    /// Call this at the top of your page `load()` to repopulate the form UI:
    ///
    /// ```rust,ignore
    /// pub async fn load(req: Req) -> AppResult<Props> {
    ///     let flash = req.take_form_flash();
    ///     Ok(Props { errors: flash, ..Default::default() })
    /// }
    /// ```
    ///
    /// The cookie is automatically cleared so the errors don't persist on refresh.
    pub fn take_form_flash(&self) -> Option<FormErrors> {
        let cookie = self.cookies.get("silcrow_form_flash")?;
        let decoded = urlencoding::decode(cookie.value()).ok()?;
        let flash: FormErrors = serde_json::from_str(&decoded).ok()?;
        let removal = Cookie::build(("silcrow_form_flash", ""))
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(Duration::seconds(0))
            .build();
        self.res.with_cookie(removal);
        Some(flash)
    }

    /// Extract a `Req` from request `Parts` only — no body consumed.
    ///
    /// Used by the generated middleware glue so the original `Request` can be
    /// reconstructed (with body intact) and forwarded through `Next`.
    ///
    /// - `form` is always empty (body not consumed)
    /// - `Locals` and `Res` are inserted into `parts.extensions` so they are
    ///   shared with the downstream page/action handlers
    ///
    /// This is `#[doc(hidden)]` — it is not part of the public API.
    #[doc(hidden)]
    pub async fn __from_middleware_parts<S: Send + Sync>(parts: &mut Parts, state: &S) -> Self {
        let common = extract_common_parts(parts, state).await;
        Req {
            params: common.params,
            query: common.query,
            form: FormMap::default(),
            cookies: common.cookies,
            headers: common.headers,
            path: common.path,
            is_enhanced: common.is_enhanced,
            locals: common.locals,
            res: common.res,
            cache: common.cache,
            locale: common.locale,
            i18n: common.i18n,
            action: common.action,
            mutation_id: common.mutation_id,
        }
    }

    /// Construct a read-only `Req` snapshot from request `Parts` for the error handler hook.
    ///
    /// Called by the generated `__pilcrow_error_handler` shim before forwarding the request
    /// to the route handler. The snapshot is passed to the user's `handle_error` hook if a
    /// 5xx response is observed.
    ///
    /// - `form` is always empty (body not available at this point)
    /// - `params` is empty (path extraction is async)
    /// - `Locals` and `Res` are **read** from `parts.extensions` so that values set by the
    ///   `handle` hook are visible inside `handle_error`
    #[doc(hidden)]
    pub fn __from_error_parts(parts: &Parts) -> Self {
        let locals = parts
            .extensions
            .get::<Locals>()
            .cloned()
            .unwrap_or_default();
        let res = parts.extensions.get::<Res>().cloned().unwrap_or_default();
        let path = parts.uri.path().to_owned();
        let headers = parts.headers.clone();
        let is_enhanced = parts.headers.typed_get::<SilcrowTarget>().is_some();
        let mutation_id = parts
            .headers
            .typed_get::<SilcrowMutationId>()
            .map(|h| h.0);
        let action = extract_action_from_query(parts.uri.query());
        let query = parse_query_multi(parts.uri.query());
        let locale = parts
            .extensions
            .get::<CurrentLocale>()
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let i18n = parts.extensions.get::<I18nBundles>().cloned();
        Req {
            params: HashMap::new(),
            query,
            form: FormMap::default(),
            cookies: CookieJar::new(),
            headers,
            path,
            is_enhanced,
            locals,
            res,
            cache: IsrHandle::default(),
            locale,
            i18n,
            action,
            mutation_id,
        }
    }

    /// Translate a message key using the request locale.
    ///
    /// This is the low-level translation primitive. Prefer the generated `t::` functions
    /// (e.g. `t::greeting(&req, &user.name)`) which are compile-time-safe and typed.
    ///
    /// Returns the key verbatim when i18n is not configured or the key is missing.
    ///
    /// ```rust,ignore
    /// let msg = req.__t("greeting", &[("name", &user.name)]);
    /// ```
    #[doc(hidden)]
    pub fn __t(&self, key: &str, args: &[(&str, &str)]) -> String {
        match &self.i18n {
            Some(bundles) => bundles.translate(&self.locale, key, args),
            None => key.to_string(),
        }
    }

    /// Return a locale-aware formatting helper for this request.
    ///
    /// ```rust,ignore
    /// let formatted = req.fmt().number(1_234_567); // "1,234,567" (en) or "1.234.567" (de)
    /// let price    = req.fmt().float(19.99, 2);
    /// ```
    pub fn fmt(&self) -> FmtHelper<'_> {
        FmtHelper {
            locale: &self.locale,
        }
    }

    /// Return a builder for constructing a synthetic `Req` in unit tests.
    ///
    /// ```rust,ignore
    /// let req = Req::for_test()
    ///     .param("id", "42")
    ///     .query("page", "1")
    ///     .form_field("name", "Widget")
    ///     .build();
    /// let props = load(req).await.unwrap();
    /// ```
    pub fn for_test() -> ReqBuilder {
        ReqBuilder::new()
    }

    /// Construct a synthetic `Req` from captured parts for ISR background revalidation tasks.
    ///
    /// The synthetic request has an empty form body, `is_enhanced = false`, a fresh `Res`,
    /// and a no-op `IsrHandle` (to prevent recursive cache writes inside the spawned task).
    #[doc(hidden)]
    pub fn __synthetic(
        path: String,
        params: HashMap<String, String>,
        query: FormMap,
        headers: HeaderMap,
        cookies: CookieJar,
        locals: Locals,
    ) -> Self {
        Req {
            params,
            query,
            form: FormMap::default(),
            cookies,
            headers,
            path,
            is_enhanced: false,
            locals,
            res: Res::default(),
            cache: IsrHandle::default(),
            locale: String::new(),
            i18n: None,
            action: None,
            mutation_id: None,
        }
    }
}

// ── ReqBuilder ───────────────────────────────────────────────

/// Builder for a synthetic [`Req`] used in unit tests. Obtain via [`Req::for_test()`].
pub struct ReqBuilder {
    params: HashMap<String, String>,
    query: FormMap,
    form: FormMap,
    cookies: CookieJar,
    headers: HeaderMap,
    path: String,
    is_enhanced: bool,
    locale: String,
}

impl ReqBuilder {
    fn new() -> Self {
        Self {
            params: HashMap::new(),
            query: FormMap::default(),
            form: FormMap::default(),
            cookies: CookieJar::default(),
            headers: HeaderMap::new(),
            path: "/".to_string(),
            is_enhanced: false,
            locale: String::new(),
        }
    }

    /// Set the locale for this test request (e.g. `"de"` to test German translations).
    pub fn locale(mut self, locale: &str) -> Self {
        self.locale = locale.to_string();
        self
    }

    /// Set a URL path param (e.g. `:id`).
    pub fn param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    /// Append a query string value.
    pub fn query(mut self, key: &str, value: &str) -> Self {
        self.query
            .0
            .entry(key.to_string())
            .or_default()
            .push(value.to_string());
        self
    }

    /// Append a form body field.
    pub fn form_field(mut self, key: &str, value: &str) -> Self {
        self.form
            .0
            .entry(key.to_string())
            .or_default()
            .push(value.to_string());
        self
    }

    /// Add a cookie.
    pub fn cookie(mut self, name: &str, value: &str) -> Self {
        self.cookies = self
            .cookies
            .add(Cookie::new(name.to_string(), value.to_string()));
        self
    }

    /// Set the request path (default: `"/"`).
    pub fn path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }

    /// Mark as an enhanced (silcrow.js) request (`is_enhanced = true`).
    pub fn enhanced(mut self) -> Self {
        self.is_enhanced = true;
        self
    }

    /// Finalise and return the synthetic [`Req`].
    pub fn build(self) -> Req {
        Req {
            params: self.params,
            query: self.query,
            form: self.form,
            cookies: self.cookies,
            headers: self.headers,
            path: self.path,
            is_enhanced: self.is_enhanced,
            locals: Locals::default(),
            res: Res::default(),
            cache: IsrHandle::default(),
            locale: self.locale,
            i18n: None,
            action: None,
            mutation_id: None,
        }
    }
}

/// Fields shared between full `FromRequest` extraction and the parts-only
/// middleware extraction. Avoids duplicating ~30 lines across both paths.
struct CommonParts {
    params: HashMap<String, String>,
    query: FormMap,
    cookies: CookieJar,
    headers: HeaderMap,
    path: String,
    is_enhanced: bool,
    locals: Locals,
    res: Res,
    cache: IsrHandle,
    locale: String,
    i18n: Option<I18nBundles>,
    action: Option<String>,
    mutation_id: Option<String>,
}

async fn extract_common_parts<S: Send + Sync>(parts: &mut Parts, state: &S) -> CommonParts {
    let params = Path::<HashMap<String, String>>::from_request_parts(parts, state)
        .await
        .map(|p| p.0)
        .unwrap_or_default();

    let action = extract_action_from_query(parts.uri.query());
    let query = parse_query_multi(parts.uri.query());

    let cookies = CookieJar::from_request_parts(parts, state)
        .await
        .unwrap_or_default();

    let headers = parts.headers.clone();
    let path = parts.uri.path().to_owned();
    let is_enhanced = parts.headers.typed_get::<SilcrowTarget>().is_some();
    let mutation_id = parts
        .headers
        .typed_get::<SilcrowMutationId>()
        .map(|h| h.0);

    // Shared per-request Locals: first extraction creates and inserts;
    // subsequent ones share the same Arc.
    let locals = parts
        .extensions
        .get::<Locals>()
        .cloned()
        .unwrap_or_else(|| {
            let l = Locals::default();
            parts.extensions.insert(l.clone());
            l
        });

    // Shared per-request Res: same pattern.
    let res = parts.extensions.get::<Res>().cloned().unwrap_or_else(|| {
        let r = Res::default();
        parts.extensions.insert(r.clone());
        r
    });

    // ISR cache handle — injected by start() when the cache is initialised.
    // Pages without REVALIDATE will have a no-op IsrHandle (inner = None).
    let cache = parts
        .extensions
        .get::<IsrHandle>()
        .cloned()
        .unwrap_or_default();

    // Locale — set by the i18n locale-rewrite middleware; empty when i18n is not configured.
    let locale = parts
        .extensions
        .get::<CurrentLocale>()
        .map(|c| c.0.clone())
        .unwrap_or_default();

    // i18n bundles — injected by start() when [i18n] is configured.
    let i18n = parts.extensions.get::<I18nBundles>().cloned();

    CommonParts {
        params,
        query,
        cookies,
        headers,
        path,
        is_enhanced,
        locals,
        res,
        cache,
        locale,
        i18n,
        action,
        mutation_id,
    }
}

#[async_trait]
impl<S: Send + Sync> FromRequest<S> for Req {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();

        let common = extract_common_parts(&mut parts, state).await;

        // Reconstruct the request so Form can consume the body.
        let req = Request::from_parts(parts, body);
        let pairs: Vec<(String, String)> = Form::<Vec<(String, String)>>::from_request(req, state)
            .await
            .map(|f| f.0)
            .unwrap_or_default();

        let mut raw_map: HashMap<String, Vec<String>> = HashMap::new();
        for (k, v) in pairs {
            raw_map.entry(k).or_default().push(v);
        }
        let form = FormMap(raw_map);

        Ok(Req {
            params: common.params,
            query: common.query,
            form,
            cookies: common.cookies,
            headers: common.headers,
            path: common.path,
            is_enhanced: common.is_enhanced,
            locals: common.locals,
            res: common.res,
            cache: common.cache,
            locale: common.locale,
            i18n: common.i18n,
            action: common.action,
            mutation_id: common.mutation_id,
        })
    }
}
