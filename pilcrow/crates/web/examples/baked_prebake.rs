//! Demonstrates manually writing shell + JSON artifacts before the server starts
//! and serving them via BakedRoute — simulating "build-time" prebaking.
//!
//! Run with:
//! cargo run -p pilcrow-web --features experimental-baked-pages --example baked_prebake

use pilcrow_web::experimental::baked_pages::{
    BakedPage, BakedPagePaths, BakedPageStore, BakedRoute, BakedSlot, DependencyConfig,
};
use serde_json::json;
use std::{fs, io};

fn main() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "pilcrow-baked-prebake-example-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let store = BakedPageStore::new(&root);
    let route = BakedRoute::new(store.clone());

    // Write shell + JSON before the server starts (simulating build-time baking).
    let shell = r#"<html><body><span data-pilcrow-slot="status">Loading</span></body></html>"#;
    store.write_shell("/tickets/:id", shell)?;
    store.write_json("/tickets/123", &json!({ "status": "Open" }))?;

    // Mark the page as already baked so `serve()` hits the artifact.
    let mut page = BakedPage::new(
        BakedPagePaths::new(
            "/tickets/:id",
            "/tickets/123",
            store
                .shell_path("/tickets/:id")
                .to_string_lossy()
                .to_string(),
            store
                .json_path("/tickets/123")
                .to_string_lossy()
                .to_string(),
            store
                .metadata_path("/tickets/123")
                .to_string_lossy()
                .to_string(),
        ),
        vec![BakedSlot::text("status")],
        vec![DependencyConfig::immediate("ticket:123", "status")],
        None,
        "render-v1",
    );
    page.is_baked = true;
    store.write_page(&page)?;

    // Read back so we have the persisted state.
    let page = store.read_page("/tickets/123")?.expect("page must exist");

    let response = route.serve(page, || {
        Err(io::Error::other("should not render — artifact is prebaked"))
    })?;

    assert_eq!(response.headers()["x-pilcrow-baked"], "hit");
    assert_eq!(response.headers()["x-pilcrow-ssr-load"], "skipped");

    println!("prebaked page served as hit/skipped from {root:?}");
    let _ = fs::remove_dir_all(&root);
    Ok(())
}
