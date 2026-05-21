use std::collections::HashMap;
use std::sync::RwLock;

// ── ListChunkCache trait ──────────────────────────────────────────────────────

/// Per-row pre-baked HTML storage for keyed lists.
///
/// Stores the rendered HTML for individual list rows so pages can serve them
/// from cache instead of re-rendering. Rows are keyed by `(list_name, row_key)`,
/// matching the `data-pilcrow-list` / `data-pilcrow-key` attributes in the DOM.
///
/// The intended Redis key scheme (for future `live-props-redis` impl) is
/// `pilcrow:chunk:{list_name}:{row_key}`.
///
/// # Usage
///
/// ```rust,ignore
/// // Register at startup:
/// let cache = Arc::new(InMemoryListChunkCache::new());
/// app.layer(Extension(cache.clone() as Arc<dyn ListChunkCache>));
///
/// // In a page handler — serve from cache or render + cache:
/// if let Some(html) = cache.get("tickets", &id) {
///     // use pre-baked html
/// } else {
///     let html = render_ticket_row(&ticket);
///     cache.set("tickets", &id, html.clone());
///     // use html
/// }
///
/// // After a mutation — invalidate the stale chunk:
/// list_broadcast.send_row("tickets", &updated);
/// cache.invalidate("tickets", &updated.id.to_string());
/// ```
pub trait ListChunkCache: Send + Sync + 'static {
    /// Return the pre-baked HTML for the given row, if cached and valid.
    fn get(&self, list_name: &str, key: &str) -> Option<String>;

    /// Store pre-baked HTML for a row. Overwrites any existing entry.
    fn set(&self, list_name: &str, key: &str, html: String);

    /// Remove the cached HTML for a single row (e.g. after a `ListBroadcast::send_row`).
    fn invalidate(&self, list_name: &str, key: &str);

    /// Remove all cached rows for an entire list.
    fn invalidate_list(&self, list_name: &str);
}

// ── InMemoryListChunkCache ────────────────────────────────────────────────────

/// In-process `ListChunkCache` backed by a `RwLock<HashMap>`.
///
/// Suitable for development, testing, and single-process deployments without Redis.
/// All chunks are lost on process restart.
#[derive(Debug, Default)]
pub struct InMemoryListChunkCache {
    inner: RwLock<HashMap<String, HashMap<String, String>>>,
}

impl InMemoryListChunkCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ListChunkCache for InMemoryListChunkCache {
    fn get(&self, list_name: &str, key: &str) -> Option<String> {
        self.inner.read().ok()?.get(list_name)?.get(key).cloned()
    }

    fn set(&self, list_name: &str, key: &str, html: String) {
        if let Ok(mut map) = self.inner.write() {
            map.entry(list_name.to_string()).or_default().insert(key.to_string(), html);
        }
    }

    fn invalidate(&self, list_name: &str, key: &str) {
        if let Ok(mut map) = self.inner.write() {
            if let Some(inner) = map.get_mut(list_name) {
                inner.remove(key);
            }
        }
    }

    fn invalidate_list(&self, list_name: &str) {
        if let Ok(mut map) = self.inner.write() {
            map.remove(list_name);
        }
    }
}

/// Returns the canonical Redis key for a list chunk.
///
/// Format: `pilcrow:chunk:{list_name}:{row_key}` where each segment has `:` percent-encoded
/// to prevent collisions between `("a:b","c")` and `("a","b:c")`.
pub fn list_chunk_key(list_name: &str, row_key: &str) -> String {
    let list = list_name.replace(':', "%3A");
    let key = row_key.replace(':', "%3A");
    format!("pilcrow:chunk:{list}:{key}")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_round_trips() {
        let cache = InMemoryListChunkCache::new();
        cache.set("tickets", "42", "<li>Open</li>".to_string());
        assert_eq!(cache.get("tickets", "42"), Some("<li>Open</li>".to_string()));
    }

    #[test]
    fn get_missing_returns_none() {
        let cache = InMemoryListChunkCache::new();
        assert!(cache.get("tickets", "99").is_none());
    }

    #[test]
    fn invalidate_removes_single_row() {
        let cache = InMemoryListChunkCache::new();
        cache.set("tickets", "1", "<li>A</li>".to_string());
        cache.set("tickets", "2", "<li>B</li>".to_string());
        cache.invalidate("tickets", "1");
        assert!(cache.get("tickets", "1").is_none());
        assert!(cache.get("tickets", "2").is_some());
    }

    #[test]
    fn invalidate_list_clears_all_rows_for_that_list() {
        let cache = InMemoryListChunkCache::new();
        cache.set("tickets", "1", "<li>A</li>".to_string());
        cache.set("tickets", "2", "<li>B</li>".to_string());
        cache.set("orders", "99", "<li>O</li>".to_string());
        cache.invalidate_list("tickets");
        assert!(cache.get("tickets", "1").is_none());
        assert!(cache.get("tickets", "2").is_none());
        assert!(cache.get("orders", "99").is_some(), "other list unaffected");
    }

    #[test]
    fn set_overwrites_existing_entry() {
        let cache = InMemoryListChunkCache::new();
        cache.set("tickets", "5", "<li>Old</li>".to_string());
        cache.set("tickets", "5", "<li>New</li>".to_string());
        assert_eq!(cache.get("tickets", "5"), Some("<li>New</li>".to_string()));
    }

    #[test]
    fn list_chunk_key_format() {
        assert_eq!(list_chunk_key("tickets", "42"), "pilcrow:chunk:tickets:42");
        assert_eq!(list_chunk_key("orders", "order:99"), "pilcrow:chunk:orders:order%3A99");
    }

    #[test]
    fn list_chunk_key_no_collision() {
        assert_ne!(list_chunk_key("a:b", "c"), list_chunk_key("a", "b:c"));
    }
}
