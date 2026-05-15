use super::model::{BakedPage, DependencyKey, StaleState};
use std::{
    collections::BTreeMap,
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// `dep_key → { concrete_path → [field_names] }`
///
/// When a dep key fires, look up which concrete paths are affected and which JSON
/// keys within their data artifacts need to be updated.
pub type ReverseIndex = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Debug, Clone)]
pub struct BakedPageStore {
    root: PathBuf,
}

impl BakedPageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // ── Path helpers ──────────────────────────────────────────────────────────

    /// Shell path: one per route pattern, shared across all concrete paths.
    ///
    /// `/tickets/:id` → `shells/tickets__id.html`
    pub fn shell_path(&self, route_pattern: &str) -> PathBuf {
        self.root.join("shells").join(pattern_name(route_pattern))
    }

    /// JSON data artifact path: one per concrete path.
    ///
    /// `/tickets/123` → `data/tickets/123.json`
    pub fn json_path(&self, concrete_path: &str) -> PathBuf {
        let trimmed = concrete_path.trim_start_matches('/');
        if trimmed.is_empty() {
            self.root.join("data").join("index.json")
        } else {
            let mut path = self.root.join("data");
            for segment in trimmed.split('/') {
                path = path.join(segment);
            }
            path.with_extension("json")
        }
    }

    /// Metadata path: one per concrete path.
    ///
    /// `/tickets/123` → `metadata/tickets/123.json`
    pub fn metadata_path(&self, concrete_path: &str) -> PathBuf {
        let trimmed = concrete_path.trim_start_matches('/');
        if trimmed.is_empty() {
            self.root.join("metadata").join("index.json")
        } else {
            let mut path = self.root.join("metadata");
            for segment in trimmed.split('/') {
                path = path.join(segment);
            }
            path.with_extension("json")
        }
    }

    pub fn reverse_index_path(&self) -> PathBuf {
        self.root.join("reverse-index.json")
    }

    // ── Shell operations ──────────────────────────────────────────────────────

    /// Write the static shell HTML for a route pattern. Idempotent — safe to call on every
    /// render; overwrites only if the file does not exist yet or has changed.
    pub fn write_shell(&self, route_pattern: &str, html: &str) -> io::Result<()> {
        let path = self.shell_path(route_pattern);
        // Skip write if content is identical (avoids unnecessary fsync).
        if let Ok(existing) = fs::read_to_string(&path)
            && existing == html
        {
            return Ok(());
        }
        self.write_atomic(&path, html.as_bytes())
    }

    pub fn read_shell(&self, route_pattern: &str) -> io::Result<Option<String>> {
        match fs::read_to_string(self.shell_path(route_pattern)) {
            Ok(html) => Ok(Some(html)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    // ── JSON data artifact operations ─────────────────────────────────────────

    pub fn write_json(&self, concrete_path: &str, json: &serde_json::Value) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(json).map_err(io::Error::other)?;
        self.write_atomic(&self.json_path(concrete_path), &bytes)
    }

    pub fn read_json(&self, concrete_path: &str) -> io::Result<Option<serde_json::Value>> {
        match fs::read_to_string(self.json_path(concrete_path)) {
            Ok(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Patch a single key in the stored JSON artifact. Reads, updates, and atomically rewrites.
    ///
    /// Supports dot-notation for nested keys: `"details.price"` updates `json["details"]["price"]`.
    pub fn patch_json_key(
        &self,
        concrete_path: &str,
        key: &str,
        value: serde_json::Value,
    ) -> io::Result<()> {
        let mut json = self.read_json(concrete_path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no JSON artifact for `{concrete_path}`"),
            )
        })?;
        set_json_key(&mut json, key, value);
        self.write_json(concrete_path, &json)
    }

    // ── Metadata operations ───────────────────────────────────────────────────

    pub fn read_page(&self, concrete_path: &str) -> io::Result<Option<BakedPage>> {
        let path = self.metadata_path(concrete_path);
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn write_page(&self, page: &BakedPage) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(page).map_err(io::Error::other)?;
        self.write_atomic(&self.metadata_path(&page.concrete_path), &json)
    }

    /// Increment `hit_count` and update `last_accessed_at`. Returns the new hit count.
    pub fn increment_hit_count(&self, concrete_path: &str) -> io::Result<u64> {
        if let Some(mut page) = self.read_page(concrete_path)? {
            page.hit_count += 1;
            page.last_accessed_at = unix_timestamp();
            self.write_page(&page)?;
            Ok(page.hit_count)
        } else {
            Ok(0)
        }
    }

    pub fn mark_stale(&self, concrete_path: &str, reason: impl Into<String>) -> io::Result<()> {
        if let Some(mut page) = self.read_page(concrete_path)? {
            page.stale_state = StaleState::stale(reason);
            self.write_page(&page)?;
        }
        Ok(())
    }

    // ── Reverse index ─────────────────────────────────────────────────────────

    pub fn ensure_reverse_index(&self) -> io::Result<ReverseIndex> {
        match self.reverse_index() {
            Ok(Some(index)) => Ok(index),
            Ok(None) | Err(_) => {
                let index = self.rebuild_reverse_index_from_metadata()?;
                self.write_reverse_index(&index)?;
                Ok(index)
            }
        }
    }

    pub fn rebuild_reverse_index_from_metadata(&self) -> io::Result<ReverseIndex> {
        let mut index = ReverseIndex::new();
        for path in self.metadata_files()? {
            let raw = fs::read_to_string(&path)?;
            let page: BakedPage = match serde_json::from_str(&raw) {
                Ok(p) => p,
                Err(_) => continue,
            };
            for config in &page.dependency_configs {
                add_index_entry(
                    &mut index,
                    &config.key,
                    &page.concrete_path,
                    &config.field_name,
                );
            }
        }
        Ok(index)
    }

    pub fn reverse_index(&self) -> io::Result<Option<ReverseIndex>> {
        match fs::read_to_string(self.reverse_index_path()) {
            Ok(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn write_reverse_index(&self, index: &ReverseIndex) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(index).map_err(io::Error::other)?;
        self.write_atomic(&self.reverse_index_path(), &json)
    }

    /// Update the reverse index to include entries for a newly baked page.
    pub fn upsert_reverse_index_page(&self, page: &BakedPage) -> io::Result<()> {
        let mut index = match self.reverse_index() {
            Ok(Some(i)) => i,
            Ok(None) | Err(_) => ReverseIndex::new(),
        };
        for pages in index.values_mut() {
            pages.remove(&page.concrete_path);
        }
        index.retain(|_, pages| !pages.is_empty());
        for config in &page.dependency_configs {
            add_index_entry(
                &mut index,
                &config.key,
                &page.concrete_path,
                &config.field_name,
            );
        }
        self.write_reverse_index(&index)
    }

    // ── Atomic writes ─────────────────────────────────────────────────────────

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = temp_path_for(path);
        {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent()
            && let Ok(dir) = OpenOptions::new().read(true).open(parent)
        {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn metadata_files(&self) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        collect_json_files(&self.root.join("metadata"), &mut files)?;
        files.sort();
        Ok(files)
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

fn add_index_entry(
    index: &mut ReverseIndex,
    dep: &DependencyKey,
    concrete_path: &str,
    field_name: &str,
) {
    let fields = index
        .entry(dep.as_str().to_string())
        .or_default()
        .entry(concrete_path.to_string())
        .or_default();
    if !fields.iter().any(|f| f == field_name) {
        fields.push(field_name.to_string());
        fields.sort();
    }
}

/// Normalize a route pattern to a shell filename.
///
/// `/tickets/:id` → `tickets__id.html`
/// `/` → `index.html`
fn pattern_name(route_pattern: &str) -> String {
    let trimmed = route_pattern.trim_matches('/');
    if trimmed.is_empty() {
        return "index.html".to_string();
    }
    let normalized = trimmed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    // Collapse runs of underscores from separators.
    let mut result = String::new();
    let mut prev_underscore = false;
    for ch in normalized.chars() {
        if ch == '_' {
            if !prev_underscore {
                result.push('_');
                result.push('_');
            }
            prev_underscore = true;
        } else {
            prev_underscore = false;
            result.push(ch);
        }
    }
    format!("{result}.html")
}

/// Update a JSON value at the given dot-notation key path.
fn set_json_key(json: &mut serde_json::Value, key: &str, value: serde_json::Value) {
    let segments: Vec<&str> = key.split('.').collect();
    let mut current = json;
    for (i, segment) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(segment.to_string(), value);
                return;
            }
        } else {
            let Some(next) = current
                .as_object_mut()
                .and_then(|obj| obj.get_mut(*segment))
            else {
                return;
            };
            current = next;
        }
    }
}

fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let pid = std::process::id();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("artifact");
    path.with_file_name(format!(".{filename}.{pid}.{nonce}.tmp"))
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
    use crate::baked_pages::model::{BakedPagePaths, BakedSlot, DependencyConfig};
    use serde_json::json;

    fn make_page(store: &BakedPageStore, concrete_path: &str) -> BakedPage {
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
            Some(10),
            "v1",
        )
    }

    #[test]
    fn write_and_read_json_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let data = json!({ "status": "Open", "title": "Bug" });

        store.write_json("/tickets/123", &data).unwrap();

        let read = store.read_json("/tickets/123").unwrap().unwrap();
        assert_eq!(read["status"], "Open");
    }

    #[test]
    fn patch_json_key_updates_single_field() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_json("/tickets/123", &json!({ "status": "Open" }))
            .unwrap();

        store
            .patch_json_key("/tickets/123", "status", json!("Closed"))
            .unwrap();

        let result = store.read_json("/tickets/123").unwrap().unwrap();
        assert_eq!(result["status"], "Closed");
    }

    #[test]
    fn write_and_read_shell() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());

        store
            .write_shell(
                "/tickets/:id",
                "<span data-pilcrow-slot=\"status\">Loading</span>",
            )
            .unwrap();

        let shell = store.read_shell("/tickets/:id").unwrap().unwrap();
        assert!(shell.contains("data-pilcrow-slot=\"status\""));
    }

    #[test]
    fn write_shell_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let html = "<span data-pilcrow-slot=\"status\">Loading</span>";

        store.write_shell("/tickets/:id", html).unwrap();
        store.write_shell("/tickets/:id", html).unwrap(); // should not error
        assert_eq!(store.read_shell("/tickets/:id").unwrap().unwrap(), html);
    }

    #[test]
    fn write_and_read_page_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123");

        store.write_page(&page).unwrap();

        let read = store.read_page("/tickets/123").unwrap().unwrap();
        assert_eq!(read.concrete_path, "/tickets/123");
        assert!(!read.is_baked);
    }

    #[test]
    fn increment_hit_count() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123");
        store.write_page(&page).unwrap();

        let count1 = store.increment_hit_count("/tickets/123").unwrap();
        let count2 = store.increment_hit_count("/tickets/123").unwrap();

        assert_eq!(count1, 1);
        assert_eq!(count2, 2);
        assert_eq!(
            store.read_page("/tickets/123").unwrap().unwrap().hit_count,
            2
        );
    }

    #[test]
    fn reverse_index_rebuilt_from_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123");
        store.write_page(&page).unwrap();

        let index = store.rebuild_reverse_index_from_metadata().unwrap();

        assert!(index.contains_key("TicketStatus:123"));
        assert!(index["TicketStatus:123"].contains_key("/tickets/123"));
        assert_eq!(index["TicketStatus:123"]["/tickets/123"], vec!["status"]);
    }

    #[test]
    fn mark_stale_updates_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_page(&store, "/tickets/123");
        store.write_page(&page).unwrap();

        store.mark_stale("/tickets/123", "dep changed").unwrap();

        let read = store.read_page("/tickets/123").unwrap().unwrap();
        assert!(read.stale_state.stale);
        assert_eq!(read.stale_state.reason.as_deref(), Some("dep changed"));
    }

    #[test]
    fn json_path_rooted_at_data_dir() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());

        assert_eq!(
            store.json_path("/tickets/123"),
            temp.path().join("data").join("tickets").join("123.json")
        );
        assert_eq!(
            store.json_path("/"),
            temp.path().join("data").join("index.json")
        );
    }

    #[test]
    fn metadata_path_rooted_at_metadata_dir() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());

        assert_eq!(
            store.metadata_path("/tickets/123"),
            temp.path()
                .join("metadata")
                .join("tickets")
                .join("123.json")
        );
    }
}
