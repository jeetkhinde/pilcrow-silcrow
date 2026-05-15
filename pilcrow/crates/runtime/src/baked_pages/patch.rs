use super::{BakedPageStore, DependencyKey};
use std::{collections::BTreeMap, io, sync::Arc};

type FieldRecomputeFn = Arc<dyn Fn(&DependencyKey) -> io::Result<serde_json::Value> + Send + Sync>;

/// Registry of per-field recompute functions, keyed by dependency key.
///
/// When a dep key fires (e.g. from a DB notification), `patch_dependency` walks the
/// reverse index to find all affected JSON artifacts, calls the registered recompute
/// functions for that dep key, and atomically patches the affected JSON keys.
///
/// External patchers (written in any language) can read the metadata file to discover
/// the `patch_delay` and `field_name` for each dep key and implement the same protocol
/// without Pilcrow being involved.
pub struct BakedPatchRegistry {
    store: BakedPageStore,
    /// dep_key → [(field_name, recompute_fn)]
    recomputes: BTreeMap<String, Vec<(String, FieldRecomputeFn)>>,
}

impl BakedPatchRegistry {
    pub fn new(store: BakedPageStore) -> Self {
        Self {
            store,
            recomputes: BTreeMap::new(),
        }
    }

    pub fn store(&self) -> &BakedPageStore {
        &self.store
    }

    /// Register a recompute function for a single JSON field under a dep key.
    ///
    /// When `dep_key` fires, `recompute` is called to produce the new value for
    /// `field_name` in all affected JSON artifacts.
    pub fn register_field_recompute<F>(
        &mut self,
        dep_key: impl Into<String>,
        field_name: impl Into<String>,
        recompute: F,
    ) where
        F: Fn(&DependencyKey) -> io::Result<serde_json::Value> + Send + Sync + 'static,
    {
        self.recomputes
            .entry(dep_key.into())
            .or_default()
            .push((field_name.into(), Arc::new(recompute)));
    }

    /// Signal that a dep key has changed. Walks the reverse index, calls registered
    /// recompute functions, and patches the affected JSON artifacts.
    ///
    /// Pages whose JSON artifact does not exist yet (not yet baked) are skipped.
    /// Pages where the recompute fails are marked stale so the next request re-renders.
    pub fn patch_dependency(&self, key: impl Into<DependencyKey>) -> io::Result<BakedPatchOutcome> {
        let key = key.into();
        let index = self.store.ensure_reverse_index()?;
        let pages = index.get(key.as_str()).cloned().unwrap_or_default();
        let mut outcome = BakedPatchOutcome::default();

        for (concrete_path, field_names) in pages {
            // Skip pages that haven't been baked yet — no JSON artifact to patch.
            let Some(page) = self.store.read_page(&concrete_path)? else {
                outcome.skipped_paths.push(concrete_path.clone());
                continue;
            };
            if !page.is_baked {
                outcome.skipped_paths.push(concrete_path.clone());
                continue;
            }

            let recomputes_for_key = self.recomputes.get(key.as_str());

            for field_name in &field_names {
                let recompute = recomputes_for_key
                    .and_then(|fns| fns.iter().find(|(f, _)| f == field_name))
                    .map(|(_, f)| f.clone());

                let Some(recompute) = recompute else {
                    self.store.mark_stale(
                        &concrete_path,
                        format!("no recompute fn registered for field `{field_name}`"),
                    )?;
                    outcome.stale_paths.push(concrete_path.clone());
                    continue;
                };

                match recompute(&key) {
                    Ok(new_value) => {
                        match self
                            .store
                            .patch_json_key(&concrete_path, field_name, new_value)
                        {
                            Ok(()) => {
                                outcome.patched_paths.push(concrete_path.clone());
                            }
                            Err(err) => {
                                let _ = self.store.mark_stale(&concrete_path, err.to_string());
                                outcome.stale_paths.push(concrete_path.clone());
                            }
                        }
                    }
                    Err(err) => {
                        let _ = self.store.mark_stale(&concrete_path, err.to_string());
                        outcome.stale_paths.push(concrete_path.clone());
                    }
                }
            }
        }

        outcome.normalize();
        Ok(outcome)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BakedPatchOutcome {
    pub patched_paths: Vec<String>,
    pub stale_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
}

impl BakedPatchOutcome {
    fn normalize(&mut self) {
        self.patched_paths.sort();
        self.patched_paths.dedup();
        self.stale_paths.sort();
        self.stale_paths.dedup();
        self.skipped_paths.sort();
        self.skipped_paths.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baked_pages::model::{BakedPage, BakedPagePaths, BakedSlot, DependencyConfig};
    use serde_json::json;

    fn make_baked_page(store: &BakedPageStore, concrete_path: &str) -> BakedPage {
        let mut page = BakedPage::new(
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
        );
        page.is_baked = true;
        page
    }

    #[test]
    fn patches_json_key_on_dep_fire() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_baked_page(&store, "/tickets/123");
        store.write_page(&page).unwrap();
        store
            .write_json("/tickets/123", &json!({ "status": "Open" }))
            .unwrap();
        store.upsert_reverse_index_page(&page).unwrap();

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_field_recompute("TicketStatus:123", "status", |_key| Ok(json!("Closed")));

        let outcome = registry.patch_dependency("TicketStatus:123").unwrap();

        assert_eq!(outcome.patched_paths, vec!["/tickets/123".to_string()]);
        let result = store.read_json("/tickets/123").unwrap().unwrap();
        assert_eq!(result["status"], "Closed");
    }

    #[test]
    fn skips_unbaked_pages() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_baked_page(&store, "/tickets/123");
        let mut unbaked = page.clone();
        unbaked.is_baked = false;
        store.write_page(&unbaked).unwrap();
        store.upsert_reverse_index_page(&page).unwrap();

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_field_recompute("TicketStatus:123", "status", |_key| Ok(json!("Closed")));

        let outcome = registry.patch_dependency("TicketStatus:123").unwrap();

        assert!(outcome.patched_paths.is_empty());
        assert_eq!(outcome.skipped_paths, vec!["/tickets/123".to_string()]);
    }

    #[test]
    fn recompute_failure_marks_stale() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let page = make_baked_page(&store, "/tickets/123");
        store.write_page(&page).unwrap();
        store
            .write_json("/tickets/123", &json!({ "status": "Open" }))
            .unwrap();
        store.upsert_reverse_index_page(&page).unwrap();

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_field_recompute("TicketStatus:123", "status", |_key| {
            Err(io::Error::new(io::ErrorKind::Other, "db unavailable"))
        });

        let outcome = registry.patch_dependency("TicketStatus:123").unwrap();

        assert!(outcome.patched_paths.is_empty());
        assert_eq!(outcome.stale_paths, vec!["/tickets/123".to_string()]);
        let meta = store.read_page("/tickets/123").unwrap().unwrap();
        assert!(meta.stale_state.stale);
    }

    #[test]
    fn patches_multiple_pages_for_same_dep_key() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());

        for path in ["/tickets/123", "/tickets/456"] {
            let mut page = BakedPage::new(
                BakedPagePaths::new(
                    "/tickets/:id",
                    path,
                    store
                        .shell_path("/tickets/:id")
                        .to_string_lossy()
                        .to_string(),
                    store.json_path(path).to_string_lossy().to_string(),
                    store.metadata_path(path).to_string_lossy().to_string(),
                ),
                vec![BakedSlot::text("status")],
                vec![DependencyConfig::immediate("SharedKey", "status")],
                None,
                "v1",
            );
            page.is_baked = true;
            store.write_page(&page).unwrap();
            store.write_json(path, &json!({ "status": "Old" })).unwrap();
            store.upsert_reverse_index_page(&page).unwrap();
        }

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_field_recompute("SharedKey", "status", |_key| Ok(json!("New")));

        let outcome = registry.patch_dependency("SharedKey").unwrap();

        assert_eq!(outcome.patched_paths.len(), 2);
        for path in ["/tickets/123", "/tickets/456"] {
            assert_eq!(store.read_json(path).unwrap().unwrap()["status"], "New");
        }
    }
}
