use axum::{extract::Request, response::Response};

/// Continuation passed to the `middleware()` function in `src/middleware.rs`.
///
/// Call `next.run().await` to proceed to the route handler.  You can short-circuit
/// by returning a `Response` directly without calling `next.run()`.
///
/// ```rust,ignore
/// // src/middleware.rs
/// pub async fn middleware(req: Req, next: Next) -> Response {
///     let token = req.cookies.get("session").map(|c| c.value().to_string());
///     match auth::verify(token).await {
///         Ok(user) => req.locals.set(user),
///         Err(_) if req.path.starts_with("/admin") => {
///             return AppError::Unauthorized.into_response();
///         }
///         _ => {}
///     }
///     next.run().await
/// }
/// ```
pub struct Next {
    inner: axum::middleware::Next,
    request: Request,
}

impl Next {
    /// Construct a `Next` from axum's internal next handle and the forwarded request.
    ///
    /// Called by framework-generated glue code — not intended for direct use.
    #[doc(hidden)]
    pub fn new(inner: axum::middleware::Next, request: Request) -> Self {
        Self { inner, request }
    }

    /// Continue to the next handler (layout loads, page load, or actions).
    pub async fn run(self) -> Response {
        self.inner.run(self.request).await
    }
}
