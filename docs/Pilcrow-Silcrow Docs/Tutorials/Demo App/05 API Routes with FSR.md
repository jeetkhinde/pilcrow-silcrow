# 05 — API Routes with FSR

The demo has two API routes under `api/`. They use raw Axum instead of Pilcrow page conventions, and they access the FSR store directly to trigger invalidations.

## File layout

```text
api/
  tickets.rs   ← PUT /api/tickets/:id
  products.rs  ← GET /api/products
```

API routes export a `pub fn router() -> axum::Router` and are auto-discovered by routekit.

## `api/tickets.rs`

```rust
use pilcrow_web::axum::{self, extract::{Extension, Json, Path}, response::IntoResponse, routing, Router};
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

    // Invalidate the specific ticket detail route by dep key
    fsr_store
        .invalidate_dep_key(&format!("tickets:id={id}"))
        .await
        .ok();

    // Invalidate the dashboard counters (all six slots on /tickets)
    fsr_store.invalidate_route("/tickets").await.ok();

    axum::response::Json(serde_json::json!({ "ok": true }))
}

pub fn router() -> Router {
    Router::new().route("/:id", routing::put(update))
}
```

**`Extension(fsr_store): Extension<Arc<FsrStore>>`** — the FSR store is registered as an Axum extension by `pilcrow_start()`. API handlers extract it the same way as any other extension.

**`invalidate_dep_key("tickets:id={id}")`** — marks stale every slot whose `depends_on` contains `tickets:id={id}`. The ticket detail page's `Live` struct has `#[pilcrow::depends_on_route(tickets, id)]` which generates exactly this dep key, so both `status` and `priority` slots on `/tickets/{id}` go stale.

**`invalidate_route("/tickets")`** — marks all six counter slots on the dashboard stale.

Result: one PUT triggers SSE patches to:
- Every browser tab showing `/tickets/{id}` — status badge + priority badge update
- Every browser tab showing `/tickets` — all six counters update

## `api/products.rs`

A simple GET handler that returns HTML fragments (not JSON):

```rust
async fn get(Query(params): Query<Params>) -> Html<String> {
    let url = /* build dummyjson URL from params.category */;
    let resp: ApiResponse = reqwest::get(&url).await?.json().await?;
    Html(render_cards(&resp.products))
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", routing::get(get))
}
```

This handler returns raw HTML. A fragment on the page calls this endpoint and swaps the result into the product grid. No FSR needed — the data lives on an external service.

## Calling the API from the browser

The ticket detail template uses a `fetch()` call for the Save button:

```js
var res = await fetch('/api/tickets/{{ id }}', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ status: status, priority: priority }),
});
```

After the PUT succeeds, the watcher picks up the stale slots and pushes SSE events. The `status` and `priority` badges on every connected tab update without any additional client-side work.

## When to use API routes vs named actions

| | Named action (`?/name`) | API route (`api/`) |
|---|---|---|
| Convention | Pilcrow page action | Raw Axum handler |
| Returns | `redirect()`, `ok()`, `json()` | Any `impl IntoResponse` |
| FSR | `req.fsr.invalidate_route()` | `Extension(fsr_store).invalidate_*()` |
| Use when | Browser form submits | `fetch()` from JS, mobile clients, other services |

---

Next: [[06 React Islands]]
