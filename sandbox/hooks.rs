use pilcrow_web::{HookError, Next, Req, Response};

/// Global request handler — runs for every request before route handlers.
///
/// Use this for cross-cutting concerns: auth, logging, request IDs, locale detection.
/// Values set on `req.locals` are visible to every downstream `load()` and action fn.
pub async fn handle(_req: Req, next: Next) -> Response {
    // Example: stash the authenticated user in locals so pages can read it.
    // let token = _req.cookies.get("session").map(|c| c.value().to_string());
    // if let Ok(user) = auth::verify(token).await {
    //     _req.locals.set(user);
    // }
    next.run().await
}

/// Error hook — called whenever a route handler returns a 5xx response.
///
/// Return `Some(response)` to replace the error response (e.g. a custom 500 page).
/// Return `None` to let Pilcrow's default error rendering take over.
pub async fn handle_error(error: &HookError, _req: &Req) -> Option<Response> {
    pilcrow_web::tracing::error!(status = error.status, "server error: {}", error.message);
    None
}

/// Server startup hook — runs once before the server begins accepting connections.
///
/// Initialise shared resources here: database connection pools, caches, background jobs.
pub async fn init() {
    pilcrow_web::tracing::info!("server initialising");
}
