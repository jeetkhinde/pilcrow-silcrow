use serde::{Deserialize, Serialize};

use crate::deferred::PatchDelay;

// ── DependencyKey ─────────────────────────────────────────────────────────────

/// A domain-owned key linking a baked prop to the data that can update it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct DependencyKey(String);

impl DependencyKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DependencyKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DependencyKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

// ── BakedSlotKind ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BakedSlotKind {
    Text,
    TrustedHtml,
}

// ── BakedSlot ─────────────────────────────────────────────────────────────────

/// Declares a slot anchor in the shell HTML.
///
/// The slot `name` matches both `data-pilcrow-slot="name"` in the shell HTML and the
/// key in the baked JSON artifact that holds the value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedSlot {
    pub name: String,
    pub kind: BakedSlotKind,
}

impl BakedSlot {
    pub fn text(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::Text,
        }
    }

    pub fn trusted_html(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::TrustedHtml,
        }
    }
}

// ── DependencyConfig ──────────────────────────────────────────────────────────

/// Per-dependency configuration stored in page metadata.
///
/// External patchers (written in any language) read this to know which JSON key to
/// update and how long to wait before writing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyConfig {
    pub key: DependencyKey,
    pub patch_delay: PatchDelay,
    /// The JSON key in the data artifact this dep updates (e.g. `"status"`).
    pub field_name: String,
}

impl DependencyConfig {
    pub fn new(
        key: impl Into<DependencyKey>,
        field_name: impl Into<String>,
        patch_delay: PatchDelay,
    ) -> Self {
        Self {
            key: key.into(),
            field_name: field_name.into(),
            patch_delay,
        }
    }

    pub fn immediate(key: impl Into<DependencyKey>, field_name: impl Into<String>) -> Self {
        Self::new(key, field_name, PatchDelay::Immediate)
    }
}

// ── BakeEligibility ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BakeEligibility {
    /// Bake at startup; error on request if artifact is missing.
    BuildTime,
    /// Render on first miss; serve JSON artifact on subsequent hits.
    LazyOnFirstHit,
    /// Never bake; always render live from `load()`.
    NeverBake,
}

// ── StaleState ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaleState {
    pub stale: bool,
    pub reason: Option<String>,
}

impl StaleState {
    pub fn fresh() -> Self {
        Self {
            stale: false,
            reason: None,
        }
    }

    pub fn stale(reason: impl Into<String>) -> Self {
        Self {
            stale: true,
            reason: Some(reason.into()),
        }
    }
}

// ── BakedPagePaths ────────────────────────────────────────────────────────────

/// The five file-system paths that identify a baked page artifact.
#[derive(Debug, Clone)]
pub struct BakedPagePaths {
    pub route_pattern: String,
    pub concrete_path: String,
    pub shell_path: String,
    pub json_path: String,
    pub metadata_path: String,
}

impl BakedPagePaths {
    pub fn new(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        shell_path: impl Into<String>,
        json_path: impl Into<String>,
        metadata_path: impl Into<String>,
    ) -> Self {
        Self {
            route_pattern: route_pattern.into(),
            concrete_path: concrete_path.into(),
            shell_path: shell_path.into(),
            json_path: json_path.into(),
            metadata_path: metadata_path.into(),
        }
    }
}

// ── BakedPage ─────────────────────────────────────────────────────────────────

/// Metadata for a baked page artifact. Written to `metadata/{path}.json`.
///
/// This is the shared contract between Pilcrow, the serving layer, and any external
/// patcher or server written in another language. All fields are stable and versioned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedPage {
    pub route_pattern: String,
    pub concrete_path: String,
    /// Path to the static shell HTML — `shells/{pattern_normalized}.html`.
    /// One file per route pattern; shared across all concrete paths of that pattern.
    pub shell_path: String,
    /// Path to the baked JSON data artifact — `data/{path_normalized}.json`.
    pub json_path: String,
    pub metadata_path: String,
    pub slots: Vec<BakedSlot>,
    pub dependency_configs: Vec<DependencyConfig>,
    /// Aggregated dep keys for fast reverse-index lookups.
    pub dependency_keys: Vec<DependencyKey>,
    pub hit_count: u64,
    pub is_baked: bool,
    pub auto_prebake_threshold: Option<u32>,
    /// `None` until the page is first baked.
    pub baked_at: Option<u64>,
    pub last_accessed_at: u64,
    pub render_load_version: String,
    pub stale_state: StaleState,
}

impl BakedPage {
    pub fn new(
        paths: BakedPagePaths,
        slots: Vec<BakedSlot>,
        dependency_configs: Vec<DependencyConfig>,
        auto_prebake_threshold: Option<u32>,
        render_load_version: impl Into<String>,
    ) -> Self {
        let dependency_keys = dependency_configs.iter().map(|c| c.key.clone()).collect();
        Self {
            route_pattern: paths.route_pattern,
            concrete_path: paths.concrete_path,
            shell_path: paths.shell_path,
            json_path: paths.json_path,
            metadata_path: paths.metadata_path,
            slots,
            dependency_configs,
            dependency_keys,
            hit_count: 0,
            is_baked: false,
            auto_prebake_threshold,
            baked_at: None,
            last_accessed_at: unix_timestamp(),
            render_load_version: render_load_version.into(),
            stale_state: StaleState::fresh(),
        }
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page() -> BakedPage {
        BakedPage::new(
            BakedPagePaths::new(
                "/tickets/:id",
                "/tickets/123",
                "shells/tickets__id.html",
                "data/tickets/123.json",
                "metadata/tickets/123.json",
            ),
            vec![BakedSlot::text("status")],
            vec![DependencyConfig::immediate("TicketStatus:123", "status")],
            Some(10),
            "v1",
        )
    }

    #[test]
    fn new_page_starts_unbaked_with_zero_hits() {
        let page = make_page();
        assert!(!page.is_baked);
        assert_eq!(page.hit_count, 0);
        assert!(page.baked_at.is_none());
    }

    #[test]
    fn dependency_keys_aggregated_from_configs() {
        let page = make_page();
        assert_eq!(page.dependency_keys.len(), 1);
        assert_eq!(page.dependency_keys[0].as_str(), "TicketStatus:123");
    }

    #[test]
    fn slot_kinds() {
        let text = BakedSlot::text("status");
        let html = BakedSlot::trusted_html("body");
        assert_eq!(text.kind, BakedSlotKind::Text);
        assert_eq!(html.kind, BakedSlotKind::TrustedHtml);
    }

    #[test]
    fn multiple_dependency_configs() {
        let page = BakedPage::new(
            BakedPagePaths::new(
                "/tickets/:id",
                "/tickets/123",
                "shells/tickets__id.html",
                "data/tickets/123.json",
                "metadata/tickets/123.json",
            ),
            vec![
                BakedSlot::text("status"),
                BakedSlot::trusted_html("description"),
            ],
            vec![
                DependencyConfig::immediate("TicketStatus:123", "status"),
                DependencyConfig::immediate("TicketBody:123", "description"),
            ],
            None,
            "v1",
        );
        assert_eq!(page.dependency_keys.len(), 2);
        assert_eq!(page.dependency_keys[0].as_str(), "TicketStatus:123");
        assert_eq!(page.dependency_keys[1].as_str(), "TicketBody:123");
    }

    #[test]
    fn stale_state_transitions() {
        let fresh = StaleState::fresh();
        assert!(!fresh.stale);

        let stale = StaleState::stale("dep changed");
        assert!(stale.stale);
        assert_eq!(stale.reason.as_deref(), Some("dep changed"));
    }
}
