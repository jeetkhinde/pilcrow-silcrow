use pilcrow_web::axum::{
    self,
    extract::{Extension, Json, Path},
    response::IntoResponse,
    routing, Router,
};
use pilcrow_runtime::fsr::FsrStore;
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct TicketUpdate {
    status: String,
    priority: String,
}

async fn update(
    Path(id): Path<i64>,
    Extension(fsr_store): Extension<Arc<FsrStore>>,
    Json(body): Json<TicketUpdate>,
) -> impl IntoResponse {
    sqlx::query("UPDATE tickets SET status = $1, priority = $2 WHERE id = $3")
        .bind(&body.status)
        .bind(&body.priority)
        .bind(id)
        .execute(crate::db::pool())
        .await
        .ok();

    fsr_store
        .invalidate_dep_key(&format!("tickets:id={id}"))
        .await
        .ok();
    fsr_store.invalidate_route("/tickets").await.ok();

    axum::response::Json(serde_json::json!({ "ok": true }))
}

pub fn router() -> Router {
    Router::new().route("/:id", routing::put(update))
}
