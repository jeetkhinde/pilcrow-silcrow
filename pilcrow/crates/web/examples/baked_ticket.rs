//! Demonstrates auto-prebake on hit threshold, cache hits, and JSON-key patching
//! when a dependency key fires (signal-driven invalidation without re-render).
//!
//! Run with:
//! cargo run -p pilcrow-web --features experimental-baked-pages --example baked_ticket

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
};
use http_body_util::BodyExt;
use pilcrow_web::experimental::baked_pages::{
    BakedPage, BakedPagePaths, BakedPageStore, BakedPatchRegistry, BakedRenderedOutput, BakedRoute,
    BakedSlot, DependencyConfig,
};
use serde_json::json;
use std::{
    fs, io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

const TICKET_PATTERN: &str = "/tickets/:id";
const TICKET_PATH: &str = "/tickets/123";
const DEP_KEY: &str = "TicketStatus:ticket_id=123";

fn shell_html() -> &'static str {
    r#"<html><body><h1>Ticket 123</h1><span data-pilcrow-slot="status">Loading</span></body></html>"#
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "pilcrow-baked-ticket-example-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let store = BakedPageStore::new(&root);
    let route = BakedRoute::new(store.clone());
    let status = Arc::new(Mutex::new(String::from("Open")));
    let render_count = Arc::new(AtomicUsize::new(0));

    // Register a recompute function for the "status" field under our dep key.
    let mut registry = BakedPatchRegistry::new(store.clone());
    registry.register_field_recompute(DEP_KEY, "status", {
        let status = status.clone();
        move |_key| Ok(json!(status.lock().unwrap().clone()))
    });
    let registry = Arc::new(registry);

    // Seed the page metadata with threshold=1 so the first request bakes it.
    let seed = make_page(&store, TICKET_PATH, Some(1));
    store.write_page(&seed)?;
    store.upsert_reverse_index_page(&seed)?;

    let app = Router::new()
        .route(
            TICKET_PATH,
            get({
                let route = route.clone();
                let store = store.clone();
                let status = status.clone();
                let render_count = render_count.clone();
                move || {
                    let route = route.clone();
                    let store = store.clone();
                    let status = status.clone();
                    let render_count = render_count.clone();
                    async move {
                        let page = store.read_page(TICKET_PATH).unwrap().unwrap();
                        route
                            .serve(page, move || {
                                render_count.fetch_add(1, Ordering::SeqCst);
                                let s = status.lock().unwrap().clone();
                                Ok(BakedRenderedOutput::new(
                                    shell_html(),
                                    json!({ "status": s }),
                                    "v1",
                                ))
                            })
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/tickets/123/close",
            post({
                let status = status.clone();
                let registry = registry.clone();
                move || {
                    let status = status.clone();
                    let registry = registry.clone();
                    async move {
                        *status.lock().unwrap() = String::from("Closed");
                        let outcome = registry.patch_dependency(DEP_KEY).unwrap();
                        assert!(outcome.stale_paths.is_empty());
                        assert_eq!(outcome.patched_paths.len(), 1);
                        StatusCode::OK
                    }
                }
            }),
        );

    // First request: bakes the JSON artifact (hit_count reaches threshold=1).
    let first = get_text(app.clone(), TICKET_PATH).await?;
    assert!(first.contains("Open"), "first response should contain Open");
    assert_eq!(render_count.load(Ordering::SeqCst), 1);

    // Second request: served from baked JSON + shell (no render call).
    let second = get_text(app.clone(), TICKET_PATH).await?;
    assert!(
        second.contains("Open"),
        "second response should contain Open"
    );
    assert_eq!(
        render_count.load(Ordering::SeqCst),
        1,
        "render should not be called again"
    );

    // Fire the dep key — patches the JSON artifact in place.
    let close = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tickets/123/close")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .map_err(io::Error::other)?;
    assert_eq!(close.status(), StatusCode::OK);

    // After patch: hit response but now contains "Closed" — no render involved.
    let patched = get_text(app.clone(), TICKET_PATH).await?;
    assert!(
        patched.contains("Closed"),
        "patched response should contain Closed"
    );
    assert_eq!(
        render_count.load(Ordering::SeqCst),
        1,
        "render still not called after patch"
    );

    println!("lazy baked ticket example served, hit, and patched at {root:?}");
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

fn make_page(store: &BakedPageStore, concrete_path: &str, threshold: Option<u32>) -> BakedPage {
    BakedPage::new(
        BakedPagePaths::new(
            TICKET_PATTERN,
            concrete_path,
            store
                .shell_path(TICKET_PATTERN)
                .to_string_lossy()
                .to_string(),
            store.json_path(concrete_path).to_string_lossy().to_string(),
            store
                .metadata_path(concrete_path)
                .to_string_lossy()
                .to_string(),
        ),
        vec![BakedSlot::text("status")],
        vec![DependencyConfig::immediate(DEP_KEY, "status")],
        threshold,
        "v1",
    )
}

async fn get_text(app: Router, path: &str) -> io::Result<String> {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .map_err(io::Error::other)?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .map_err(io::Error::other)?
            .to_bytes()
            .to_vec(),
    )
    .unwrap())
}
