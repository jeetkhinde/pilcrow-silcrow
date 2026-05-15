use super::{BakedPage, BakedPageStore, inject::inject_slots};
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakedServeState {
    /// Served from JSON artifact + shell — no `load()` call.
    Hit,
    /// Cache miss or stale; rendered by `load()`, artifact written (or promoted).
    MissRendered,
    /// `NeverBake` or below threshold; rendered live, no artifact written.
    RenderedUnbaked,
}

impl BakedServeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::MissRendered => "miss-rendered",
            Self::RenderedUnbaked => "never-bake-rendered",
        }
    }

    pub fn render_state(self) -> &'static str {
        match self {
            Self::Hit => "skipped",
            Self::MissRendered | Self::RenderedUnbaked => "ran",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedServeOutcome {
    pub html: String,
    pub state: BakedServeState,
    pub page: Option<BakedPage>,
}

/// What a `render` closure must return.
pub struct BakedRenderedOutput {
    /// The static shell HTML — written once per route pattern (idempotent).
    pub shell_html: String,
    /// The full load response as JSON — the primary artifact, written per concrete path.
    pub json: serde_json::Value,
    pub render_load_version: String,
}

impl BakedRenderedOutput {
    pub fn new(
        shell_html: impl Into<String>,
        json: serde_json::Value,
        render_load_version: impl Into<String>,
    ) -> Self {
        Self {
            shell_html: shell_html.into(),
            json,
            render_load_version: render_load_version.into(),
        }
    }
}

impl BakedPageStore {
    /// Main request entry point for baked pages.
    ///
    /// 1. Increments `hit_count` on every request.
    /// 2. If the JSON artifact is fresh, reads shell + JSON → injects slots → returns `Hit`.
    /// 3. Otherwise calls `render`, writes shell (idempotent) and conditionally writes JSON
    ///    (when `hit_count >= auto_prebake_threshold` or the page is `BuildTime`).
    pub fn get_or_render<F>(&self, mut page: BakedPage, render: F) -> io::Result<BakedServeOutcome>
    where
        F: FnOnce() -> io::Result<BakedRenderedOutput>,
    {
        // Always increment hit count.
        page.hit_count = page.hit_count.saturating_add(1);
        page.last_accessed_at = unix_timestamp();
        self.write_page(&page)?;

        // Serve from cache if fresh.
        if page.is_baked && !page.stale_state.stale
            && let Some(outcome) = self.try_serve_from_cache(&page)?
        {
            return Ok(outcome);
        }

        // Render.
        let output = render()?;

        // Write shell (per pattern, idempotent).
        self.write_shell(&page.route_pattern, &output.shell_html)?;

        // Decide whether to bake the JSON artifact.
        let should_bake = should_bake(&page);

        let html = inject_slots(&output.shell_html, &output.json, &page.slots);

        if should_bake {
            self.write_json(&page.concrete_path, &output.json)?;
            page.is_baked = true;
            page.baked_at = Some(unix_timestamp());
            page.render_load_version = output.render_load_version;
            page.stale_state = super::model::StaleState::fresh();
            self.write_page(&page)?;
            self.upsert_reverse_index_page(&page)?;

            Ok(BakedServeOutcome {
                html,
                state: BakedServeState::MissRendered,
                page: Some(page),
            })
        } else {
            Ok(BakedServeOutcome {
                html,
                state: BakedServeState::RenderedUnbaked,
                page: None,
            })
        }
    }

    fn try_serve_from_cache(&self, page: &BakedPage) -> io::Result<Option<BakedServeOutcome>> {
        let Some(shell) = self.read_shell(&page.route_pattern)? else {
            return Ok(None);
        };
        let Some(json) = self.read_json(&page.concrete_path)? else {
            return Ok(None);
        };
        let html = inject_slots(&shell, &json, &page.slots);
        Ok(Some(BakedServeOutcome {
            html,
            state: BakedServeState::Hit,
            page: Some(page.clone()),
        }))
    }
}

fn should_bake(page: &BakedPage) -> bool {
    // Already baked and stale → re-bake.
    if page.is_baked {
        return true;
    }
    // Not yet baked: check threshold.
    match page.auto_prebake_threshold {
        Some(threshold) => page.hit_count >= threshold as u64,
        None => false,
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baked_pages::model::{BakedPage, BakedPagePaths, BakedSlot, DependencyConfig};
    use serde_json::json;

    fn shell_html() -> &'static str {
        r#"<p>Status: <span data-pilcrow-slot="status">Loading</span></p>"#
    }

    fn make_page(store: &BakedPageStore, concrete_path: &str, threshold: Option<u32>) -> BakedPage {
        BakedPage::new(
            BakedPagePaths::new(
                "/tickets/:id",
                concrete_path,
                store
                    .shell_path("/tickets/:id")
                    .to_string_lossy()
                    .to_string(),
                store.json_path(concrete_path).to_string_lossy().to_string(),
                store
                    .metadata_path(concrete_path)
                    .to_string_lossy()
                    .to_string(),
            ),
            vec![BakedSlot::text("status")],
            vec![DependencyConfig::immediate("TicketStatus:123", "status")],
            threshold,
            "v1",
        )
    }

    fn render_output(status: &str) -> io::Result<BakedRenderedOutput> {
        Ok(BakedRenderedOutput::new(
            shell_html(),
            json!({ "status": status }),
            "v1",
        ))
    }

    #[test]
    fn first_request_below_threshold_is_rendered_unbaked() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123", Some(5));
        store.write_page(&page).unwrap();

        let outcome = store.get_or_render(page, || render_output("Open")).unwrap();

        assert_eq!(outcome.state, BakedServeState::RenderedUnbaked);
        assert!(outcome.page.is_none());
        assert!(store.read_json("/tickets/123").unwrap().is_none());
    }

    #[test]
    fn request_at_threshold_bakes_json_and_returns_miss_rendered() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let mut page = make_page(&store, "/tickets/123", Some(1));
        page.hit_count = 0; // threshold is 1, first hit increments to 1 → bake
        store.write_page(&page).unwrap();

        let outcome = store.get_or_render(page, || render_output("Open")).unwrap();

        assert_eq!(outcome.state, BakedServeState::MissRendered);
        assert!(outcome.page.as_ref().unwrap().is_baked);
        let json = store.read_json("/tickets/123").unwrap().unwrap();
        assert_eq!(json["status"], "Open");
    }

    #[test]
    fn second_request_after_bake_hits_cache() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let mut page = make_page(&store, "/tickets/123", Some(1));
        page.hit_count = 0;
        store.write_page(&page).unwrap();

        // First request bakes.
        let first = store
            .get_or_render(page.clone(), || render_output("Open"))
            .unwrap();
        assert_eq!(first.state, BakedServeState::MissRendered);

        // Second request should hit cache.
        let baked_page = store.read_page("/tickets/123").unwrap().unwrap();
        let second = store
            .get_or_render(baked_page, || {
                panic!("render should not be called on cache hit")
            })
            .unwrap();

        assert_eq!(second.state, BakedServeState::Hit);
        assert!(second.html.contains("Open"));
    }

    #[test]
    fn injected_html_reflects_json_value() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let mut page = make_page(&store, "/tickets/123", Some(1));
        page.hit_count = 0;
        store.write_page(&page).unwrap();

        let outcome = store
            .get_or_render(page, || render_output("Resolved"))
            .unwrap();

        assert!(outcome.html.contains("Resolved"));
        assert!(!outcome.html.contains("Loading"));
    }

    #[test]
    fn no_threshold_never_bakes() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123", None);
        store.write_page(&page).unwrap();

        let outcome = store.get_or_render(page, || render_output("Open")).unwrap();

        assert_eq!(outcome.state, BakedServeState::RenderedUnbaked);
        assert!(store.read_json("/tickets/123").unwrap().is_none());
    }
}
