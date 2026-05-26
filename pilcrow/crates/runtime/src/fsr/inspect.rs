use axum::http::{StatusCode, header};
use axum::{Extension, response::IntoResponse};
use std::sync::Arc;

use super::hub::FsrConnectionCounter;
use super::store::{FsrStore, InspectRow};

/// Dev-only handler for `GET /__pilcrow/fsr/inspect`.
///
/// Renders an HTML table of every row in `pilcrow_fsr`.
/// Returns 503 when no `FsrStore` extension is present (DB not configured).
/// Only registered when `PILCROW_DEV` is set.
pub async fn fsr_inspect_handler(
    store: Option<Extension<Arc<FsrStore>>>,
    counter: Option<Extension<FsrConnectionCounter>>,
) -> axum::response::Response {
    let content_type = [(header::CONTENT_TYPE, "text/html; charset=utf-8")];

    let conn_count = counter
        .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(0);

    let Some(Extension(store)) = store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            content_type,
            not_configured_html(conn_count),
        )
            .into_response();
    };

    match store.fetch_all_for_inspect().await {
        Ok(rows) => (content_type, build_html(&rows, conn_count)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            content_type,
            format!("<!doctype html><html><body><pre>DB error: {e}</pre></body></html>"),
        )
            .into_response(),
    }
}

fn not_configured_html(conn_count: usize) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>FSR Inspect</title></head>\
         <body><h1>FSR Inspect</h1>\
         <p><strong>Live SSE connections:</strong> {conn_count}</p>\
         <p>FSR store unavailable — <code>DATABASE_URL</code> not configured or connection failed.</p>\
         </body></html>"
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_html(rows: &[InspectRow], conn_count: usize) -> String {
    let mut out = String::from(
        "<!doctype html>\n<html>\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <title>FSR Inspect — Pilcrow</title>\n\
         <style>\n\
         body{font-family:monospace;font-size:13px;padding:1rem 2rem;}\n\
         h1{font-size:1.1rem;margin-bottom:.75rem;}\n\
         p.count{color:#555;margin-bottom:.5rem;}\n\
         table{border-collapse:collapse;width:100%;}\n\
         th,td{border:1px solid #ccc;padding:4px 8px;text-align:left;white-space:nowrap;}\n\
         th{background:#f0f0f0;}\n\
         tr.route-row>td{background:#f7f7f7;font-style:italic;color:#555;}\n\
         .yes-stale{color:#c00;font-weight:bold;}\n\
         .yes-promoted{color:#060;font-weight:bold;}\n\
         </style>\n\
         </head>\n<body>\n\
         <h1>FSR Inspect</h1>\n",
    );

    out.push_str(&format!(
        "<p><strong>Live SSE connections:</strong> {conn_count}</p>\n"
    ));

    if rows.is_empty() {
        out.push_str("<p>No rows in <code>pilcrow_fsr</code>.</p>\n");
    } else {
        out.push_str(&format!("<p class=\"count\">{} row(s)</p>\n", rows.len()));
        out.push_str(
            "<table>\n<thead>\n<tr>\
             <th>route</th><th>slot</th><th>depends_on</th>\
             <th>stale</th><th>version</th><th>hit_count</th>\
             <th>promoted</th><th>html_path</th><th>json_path</th><th>last_hit</th>\
             </tr>\n</thead>\n<tbody>\n",
        );

        for row in rows {
            let is_route_row = row.slot.is_empty();
            let row_class = if is_route_row {
                " class=\"route-row\""
            } else {
                ""
            };
            let slot_cell = if is_route_row {
                "(route)".to_string()
            } else {
                escape_html(&row.slot)
            };
            let deps = row
                .depends_on
                .iter()
                .map(|d| escape_html(d))
                .collect::<Vec<_>>()
                .join(", ");
            let stale_cell = if row.stale {
                "<span class=\"yes-stale\">stale</span>"
            } else {
                "fresh"
            };
            let promoted_cell = if row.promoted {
                "<span class=\"yes-promoted\">yes</span>"
            } else {
                "no"
            };
            let html_path = row
                .html_path
                .as_deref()
                .map(escape_html)
                .unwrap_or_else(|| "—".to_string());
            let json_path = row
                .json_path
                .as_deref()
                .map(escape_html)
                .unwrap_or_else(|| "—".to_string());
            let last_hit = row.last_hit.as_deref().unwrap_or("—");

            out.push_str(&format!(
                "<tr{row_class}><td>{route}</td><td>{slot_cell}</td><td>{deps}</td>\
                 <td>{stale_cell}</td><td>{version}</td><td>{hit_count}</td>\
                 <td>{promoted_cell}</td><td>{html_path}</td><td>{json_path}</td>\
                 <td>{last_hit}</td></tr>\n",
                route = escape_html(&row.route),
                version = row.version,
                hit_count = row.hit_count,
                last_hit = escape_html(last_hit),
            ));
        }

        out.push_str("</tbody>\n</table>\n");
    }

    out.push_str("</body>\n</html>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use axum::{Router, body::Body, routing::get};
    use tower::ServiceExt as _;

    /// Handler returns 503 when no FsrStore extension is present.
    #[tokio::test]
    async fn handler_returns_503_without_store() {
        let resp = fsr_inspect_handler(None, None).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    /// Without the route registered, the endpoint returns 404 (production mode).
    #[tokio::test]
    async fn route_absent_returns_404() {
        let app = Router::new();
        let req = Request::builder()
            .uri("/__pilcrow/fsr/inspect")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// With the route registered (dev mode), the endpoint is reachable.
    /// Without a DB extension the handler returns 503 — not 404.
    #[tokio::test]
    async fn route_present_returns_non_404_in_dev_mode() {
        let app = Router::new().route("/__pilcrow/fsr/inspect", get(fsr_inspect_handler));
        let req = Request::builder()
            .uri("/__pilcrow/fsr/inspect")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn inspect_shows_connection_count() {
        use crate::fsr::hub::FsrConnectionCounter;
        use axum::body::to_bytes;
        use std::sync::atomic::AtomicUsize;

        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(42));
        let resp = fsr_inspect_handler(None, Some(Extension(counter))).await;
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(
            html.contains("42"),
            "expected connection count 42 in HTML, got: {html}"
        );
    }

    #[tokio::test]
    async fn inspect_shows_zero_without_counter() {
        use axum::body::to_bytes;
        let resp = fsr_inspect_handler(None, None).await;
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains('0'), "expected 0 connections in HTML");
    }
}
