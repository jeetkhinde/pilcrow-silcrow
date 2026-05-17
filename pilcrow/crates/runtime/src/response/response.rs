use crate::response::headers::*;
use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration;
use headers::HeaderMapExt;
use pilcrow_core::AppError;
use serde::{Deserialize, Serialize};

/// Return type for `actions()` handlers.
///
/// A type alias for `Result<Response, AppError>`.  Use [`redirect`] for
/// successful navigation and [`Req::fail`](crate::context::Req::fail) for form
/// validation errors.  The `?` operator propagates infrastructure `AppError`s
/// (database failures, auth errors, etc.) automatically.
pub type ActionResult = Result<Response, AppError>;

pub type ErrorResponse = Response;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "success" => Self::Success,
            "warning" | "warn" => Self::Warning,
            "error" | "danger" => Self::Error,
            _ => Self::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
}

#[derive(Default)]
pub struct BaseResponse {
    pub headers: HeaderMap,
    pub cookies: CookieJar,
    pub toasts: Vec<Toast>,         // Future-proof: multiple toasts
    pub status: Option<StatusCode>, // Optional explicit status code
    pub bypass_cache: bool,         // Set by req.res.bypass_cache() inside load()
}

impl BaseResponse {
    // ── Shared header-manipulation helpers ────────────────────────
    // These are the single source of truth for response modifications.
    // Both `Res` (interior-mutable wrapper) and `ResponseExt` (owned-builder
    // trait) delegate here so there is exactly one implementation.

    /// Set an explicit HTTP status code.
    pub fn set_status(&mut self, status: StatusCode) {
        self.status = Some(status);
    }

    /// Append a raw response header.
    pub fn set_header(&mut self, key: &'static str, value: impl Into<String>) {
        if let Ok(val) = HeaderValue::from_str(&value.into()) {
            self.headers.insert(key, val);
        }
    }

    /// Add `silcrow-cache: no-cache` so silcrow.js skips the response cache.
    pub fn set_no_cache(&mut self) {
        self.headers
            .typed_insert(SilcrowCache("no-cache".to_string()));
    }

    /// Mark this response as bypassing the SSG cache — the result will not be
    /// written back to the cache for PRERENDER routes.
    pub fn set_bypass_cache(&mut self) {
        self.bypass_cache = true;
    }

    pub fn is_bypass_cache(&self) -> bool {
        self.bypass_cache
    }

    /// Add a `Set-Cookie` header to the response.
    pub fn add_cookie(&mut self, cookie: Cookie<'static>) {
        self.cookies = std::mem::take(&mut self.cookies).add(cookie);
    }

    /// Queue a toast notification.
    pub fn add_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        self.toasts.push(Toast {
            message: message.into(),
            level,
        });
    }

    /// Fire a custom DOM event on the client via `silcrow-trigger`.
    /// Multiple calls accumulate.
    pub fn add_trigger_event(&mut self, event_name: &str) {
        let mut map = self
            .headers
            .typed_get::<SilcrowTrigger>()
            .and_then(|h| {
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&h.0).ok()
            })
            .unwrap_or_default();
        map.insert(event_name.to_string(), serde_json::json!({}));
        self.headers
            .typed_insert(SilcrowTrigger(serde_json::Value::Object(map).to_string()));
    }

    /// Override the swap target selector via `silcrow-retarget`.
    pub fn set_retarget(&mut self, selector: &str) {
        self.headers
            .typed_insert(SilcrowRetarget(selector.to_string()));
    }

    /// Push a URL to the browser history via `silcrow-push`.
    pub fn set_push_history(&mut self, url: &str) {
        self.headers.typed_insert(SilcrowPush(url.to_string()));
    }

    /// Patch a secondary DOM target via `silcrow-patch`.
    /// Multiple calls accumulate.
    pub fn add_patch_target(&mut self, selector: &str, data: &impl Serialize) {
        self.add_patch_target_inner(selector, data, None);
    }

    /// Like `add_patch_target` but tags the entry with a mutation id so the
    /// client can confirm and retire the pending optimistic mutation.
    pub fn add_patch_target_with_mutation(
        &mut self,
        selector: &str,
        data: &impl Serialize,
        mutation_id: &str,
    ) {
        self.add_patch_target_inner(selector, data, Some(mutation_id));
    }

    fn add_patch_target_inner(
        &mut self,
        selector: &str,
        data: &impl Serialize,
        mutation_id: Option<&str>,
    ) {
        let mut list = self
            .headers
            .typed_get::<SilcrowPatch>()
            .and_then(|h| serde_json::from_str::<Vec<serde_json::Value>>(&h.0).ok())
            .unwrap_or_default();
        let mut entry = serde_json::json!({ "data": data, "target": selector });
        if let Some(mid) = mutation_id {
            entry["mutation_id"] = serde_json::Value::String(mid.to_owned());
        }
        list.push(entry);
        self.headers
            .typed_insert(SilcrowPatch(serde_json::Value::Array(list).to_string()));
    }

    /// Invalidate a DOM target's binding cache via `silcrow-invalidate`.
    /// Multiple calls accumulate.
    pub fn add_invalidate_target(&mut self, selector: &str) {
        let mut list = self
            .headers
            .typed_get::<SilcrowInvalidate>()
            .and_then(|h| serde_json::from_str::<Vec<String>>(&h.0).ok())
            .unwrap_or_default();
        list.push(selector.to_string());
        self.headers.typed_insert(SilcrowInvalidate(
            serde_json::to_string(&list).unwrap_or_default(),
        ));
    }

    /// Trigger a client-side navigation via `silcrow-navigate`.
    pub fn set_client_navigate(&mut self, path: &str) {
        self.headers.typed_insert(SilcrowNavigate(path.to_string()));
    }

    /// Open an SSE connection on the client via `silcrow-sse`.
    pub fn set_sse(&mut self, path: &str) {
        self.headers.typed_insert(SilcrowSse(path.to_string()));
    }

    /// Open a WebSocket connection on the client via `silcrow-ws`.
    pub fn set_ws(&mut self, path: &str) {
        self.headers.typed_insert(SilcrowWs(path.to_string()));
    }

    // ── Apply accumulated modifications ──────────────────────────

    pub fn apply_to_response(&self, response: &mut Response) {
        self.headers.iter().for_each(|(name, value)| {
            response.headers_mut().insert(name.clone(), value.clone());
        });
        if let Some(code) = self.status {
            *response.status_mut() = code;
        }
        if self.toasts.is_empty() {
            for cookie in self.cookies.iter() {
                if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
                    response
                        .headers_mut()
                        .append(axum::http::header::SET_COOKIE, header_value);
                }
            }
        } else {
            let mut final_jar = self.cookies.clone();
            if let Ok(json_string) = serde_json::to_string(&self.toasts) {
                let encoded = urlencoding::encode(&json_string).into_owned();
                let toast_cookie = Cookie::build(("silcrow_toasts", encoded))
                    .path("/")
                    .same_site(SameSite::Lax)
                    .max_age(Duration::seconds(5))
                    .build();
                final_jar = final_jar.add(toast_cookie);
            }
            for cookie in final_jar.iter() {
                if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
                    response
                        .headers_mut()
                        .append(axum::http::header::SET_COOKIE, header_value);
                }
            }
        }
    }
}

pub trait ResponseExt: Sized {
    fn base_mut(&mut self) -> &mut BaseResponse;

    fn with_header(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.base_mut().set_header(key, value);
        self
    }
    fn with_status(mut self, status: StatusCode) -> Self {
        self.base_mut().set_status(status);
        self
    }
    fn no_cache(mut self) -> Self {
        self.base_mut().set_no_cache();
        self
    }
    fn with_toast(mut self, message: impl Into<String>, level: ToastLevel) -> Self {
        self.base_mut().add_toast(message, level);
        self
    }
    fn trigger_event(mut self, event_name: &str) -> Self {
        self.base_mut().add_trigger_event(event_name);
        self
    }
    fn retarget(mut self, selector: &str) -> Self {
        self.base_mut().set_retarget(selector);
        self
    }
    fn push_history(mut self, url: &str) -> Self {
        self.base_mut().set_push_history(url);
        self
    }
    fn patch_target(mut self, selector: &str, data: &impl serde::Serialize) -> Self {
        self.base_mut().add_patch_target(selector, data);
        self
    }
    fn patch_target_with_mutation(
        mut self,
        selector: &str,
        data: &impl serde::Serialize,
        mutation_id: &str,
    ) -> Self {
        self.base_mut()
            .add_patch_target_with_mutation(selector, data, mutation_id);
        self
    }
    fn invalidate_target(mut self, selector: &str) -> Self {
        self.base_mut().add_invalidate_target(selector);
        self
    }
    fn client_navigate(mut self, path: &str) -> Self {
        self.base_mut().set_client_navigate(path);
        self
    }
    fn sse(mut self, path: impl AsRef<str>) -> Self {
        self.base_mut().set_sse(path.as_ref());
        self
    }
    fn ws(mut self, path: impl AsRef<str>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().set_ws(path.as_ref());
        self
    }
}

pub struct HtmlResponse {
    pub data: String,
    pub base: BaseResponse,
}
impl From<String> for HtmlResponse {
    fn from(s: String) -> Self {
        html(s)
    }
}

impl From<&str> for HtmlResponse {
    fn from(s: &str) -> Self {
        html(s.to_owned())
    }
}
impl IntoResponse for HtmlResponse {
    fn into_response(self) -> Response {
        let mut response = axum::response::Html(self.data).into_response();
        self.base.apply_to_response(&mut response);
        response
    }
}

pub struct JsonResponse<T> {
    pub data: T,
    pub base: BaseResponse,
}

impl<T: serde::Serialize> IntoResponse for JsonResponse<T> {
    fn into_response(self) -> Response {
        serde_json::to_value(&self.data)
            .map_err(|e| {
                tracing::error!("JsonResponse serialization failed: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
            .map(|json_payload| {
                if self.base.toasts.is_empty() {
                    json_payload
                } else {
                    let toasts_json = serde_json::json!(self.base.toasts);
                    match json_payload {
                        serde_json::Value::Object(mut map) => {
                            map.insert("_toasts".to_string(), toasts_json);
                            serde_json::Value::Object(map)
                        }
                        other => serde_json::json!({
                            "data": other,
                            "_toasts": toasts_json
                        }),
                    }
                }
            })
            .map(|final_payload| {
                let mut response = Json(final_payload).into_response();
                self.base.apply_to_response(&mut response);
                response
            })
            .unwrap_or_else(std::convert::identity)
    }
}

pub struct NavigateResponse {
    pub path: String,
    pub base: BaseResponse,
}

impl IntoResponse for NavigateResponse {
    fn into_response(self) -> Response {
        let mut response = Redirect::to(&self.path).into_response();

        // Ensure the status is explicitly 303 (Axum defaults to 303 for Redirect::to, but this guarantees it)
        *response.status_mut() = StatusCode::SEE_OTHER;

        self.base.apply_to_response(&mut response);
        response
    }
}

// ── FormErrors ────────────────────────────────────────────────

/// JSON response type for returning form validation errors from `action()` functions.
///
/// silcrow.js receives this as JSON and calls `patch(data, targetEl)`, updating elements
/// with `:text="errors.field"`, `:show="errors.field"`, and `:value="values.field"` bindings
/// in the submitted form — no full page re-render needed.
///
/// # Example
///
/// ```rust,ignore
/// pub async fn action_create(ctx: ActionContext) -> Result<impl IntoResponse, AppError> {
///     let email = ctx.form.get("email").unwrap_or("");
///     if email.is_empty() {
///         return Ok(form_errors()
///             .error("email", "Email is required")
///             .value("email", email)
///             .into_response());
///     }
///     Ok(navigate("/dashboard"))
/// }
/// ```
///
/// Matching template:
/// ```html
/// <form s-action="?/create" s-target="#my-form" id="my-form">
///   <input name="email" :value="values.email" />
///   <span class="error" :text="errors.email" :show="errors.email"></span>
/// </form>
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormErrors {
    /// Per-field error messages. Keys match form field names.
    pub errors: std::collections::HashMap<String, String>,
    /// Echoed field values to restore input state on error.
    pub values: std::collections::HashMap<String, String>,
    /// `true` when any error is present — bind to `:show="has_errors"` on an error summary.
    pub has_errors: bool,
    /// Flat list for `template[s-for]` error summaries.
    pub error_list: Vec<FormErrorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormErrorItem {
    pub field: String,
    pub message: String,
}

impl FormErrors {
    /// Add a field-level error message.
    pub fn error(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        let field = field.into();
        let message = message.into();
        self.error_list.push(FormErrorItem {
            field: field.clone(),
            message: message.clone(),
        });
        self.errors.insert(field, message);
        self.has_errors = true;
        self
    }

    /// Echo a field value back so the input is repopulated on error.
    pub fn value(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(field.into(), value.into());
        self
    }

    /// `true` if no errors have been added.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl IntoResponse for FormErrors {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

/// Construct a new [`FormErrors`] builder.
pub fn form_errors() -> FormErrors {
    FormErrors::default()
}

pub fn html(data: impl Into<String>) -> HtmlResponse {
    HtmlResponse {
        data: data.into(),
        base: BaseResponse::default(),
    }
}
pub fn status(code: StatusCode) -> Response {
    code.into_response()
}
pub fn json<T>(data: T) -> JsonResponse<T> {
    JsonResponse {
        data,
        base: BaseResponse::default(),
    }
}

pub fn navigate(path: impl Into<String>) -> NavigateResponse {
    NavigateResponse {
        path: path.into(),
        base: BaseResponse::default(),
    }
}

/// Return a plain `200 OK` from an `actions()` handler. Chain modifiers via [`ActionResultExt`]:
///
/// ```rust,ignore
/// pub async fn action_toggle(_req: Req) -> ActionResult {
///     toggle_state();
///     ok().with_toast("Done!", ToastLevel::Success)
/// }
/// ```
pub fn ok() -> ActionResult {
    Ok(StatusCode::OK.into_response())
}

/// Issue a `303 See Other` redirect from an `actions()` handler. Chain modifiers via [`ActionResultExt`]:
///
/// ```rust,ignore
/// pub async fn action_create(req: Req) -> ActionResult {
///     db::create_item(&req.form).await?;
///     redirect("/items").with_toast("Created!", ToastLevel::Success)
/// }
/// ```
pub fn redirect(path: impl Into<String>) -> ActionResult {
    Ok(NavigateResponse {
        path: path.into(),
        base: BaseResponse::default(),
    }
    .into_response())
}

/// Chainable modifiers for [`ActionResult`].
///
/// Imported automatically in every page module by Pilcrow's codegen.
pub trait ActionResultExt: Sized {
    fn with_toast(self, message: impl Into<String>, level: ToastLevel) -> Self;
    fn with_header(self, key: &'static str, value: impl Into<String>) -> Self;
    fn no_cache(self) -> Self;
    fn trigger_event(self, event_name: &str) -> Self;
    fn retarget(self, selector: &str) -> Self;
    fn push_history(self, url: &str) -> Self;
}

impl ActionResultExt for ActionResult {
    fn with_toast(self, message: impl Into<String>, level: ToastLevel) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.add_toast(message, level);
            base.apply_to_response(&mut r);
            r
        })
    }
    fn with_header(self, key: &'static str, value: impl Into<String>) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.set_header(key, value);
            base.apply_to_response(&mut r);
            r
        })
    }
    fn no_cache(self) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.set_no_cache();
            base.apply_to_response(&mut r);
            r
        })
    }
    fn trigger_event(self, event_name: &str) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.add_trigger_event(event_name);
            base.apply_to_response(&mut r);
            r
        })
    }
    fn retarget(self, selector: &str) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.set_retarget(selector);
            base.apply_to_response(&mut r);
            r
        })
    }
    fn push_history(self, url: &str) -> Self {
        self.map(|mut r| {
            let mut base = BaseResponse::default();
            base.set_push_history(url);
            base.apply_to_response(&mut r);
            r
        })
    }
}

impl ResponseExt for HtmlResponse {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}
impl<T> ResponseExt for JsonResponse<T> {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}
impl ResponseExt for NavigateResponse {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}
