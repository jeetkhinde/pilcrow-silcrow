use pilcrow_web::{HookError, Next, Req, Response};

pub async fn handle(_req: Req, next: Next) -> Response {
    next.run().await
}

pub async fn handle_error(error: &HookError, _req: &Req) -> Option<Response> {
    pilcrow_web::tracing::error!(status = error.status, "server error: {}", error.message);
    None
}

pub async fn init() {
    pilcrow_web::tracing::info!("pilcrow-demos starting");
}
