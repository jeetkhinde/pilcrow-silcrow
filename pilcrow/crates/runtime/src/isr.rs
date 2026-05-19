use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::context::FormMap;

// ── Cache entry ───────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    html: String,
    stored_unix: u64,
    ttl_secs: u64,
    #[serde(skip)]
    revalidating: bool,
    tags: Vec<String>,
}

impl CacheEntry {
    fn new(key: String, html: String, ttl_secs: u64, tags: Vec<String>) -> Self {
        Self {
            key,
            html,
            stored_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ttl_secs,
            revalidating: false,
            tags,
        }
    }

    fn age_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.stored_unix)
    }

    fn is_fresh(&self) -> bool {
        self.age_secs() < self.ttl_secs
    }
}

// ── CacheState ────────────────────────────────────────────────

/// The result of looking up a key in the ISR cache.
#[derive(Debug)]
pub enum IsrCacheState {
    /// Data is within TTL — serve immediately.
    Fresh(String),
    /// Data has exceeded TTL — serve stale, optionally revalidate in background.
    Stale(String),
    /// No cached entry or past max stale age — must render synchronously.
    Miss,
}

// ── IsrCache ──────────────────────────────────────────────────

/// In-process SSG cache backed by a `DashMap<String, CacheEntry>`.
///
/// Used by `PRERENDER = true` routes. Always in-memory; filesystem persistence
/// has been removed. `IsrCache` is `Clone` — cloning shares the same storage.
#[derive(Clone, Default)]
pub struct IsrCache {
    map: Arc<DashMap<String, CacheEntry>>,
}

impl IsrCache {
    pub fn new() -> Self {
        Self {
            map: Arc::new(DashMap::new()),
        }
    }

    /// Check the cache state for a given key.
    pub async fn check(&self, key: &str, max_stale: Option<u64>) -> IsrCacheState {
        match self.map.get(key) {
            None => IsrCacheState::Miss,
            Some(entry) => {
                if entry.is_fresh() {
                    IsrCacheState::Fresh(entry.html.clone())
                } else {
                    let stale_secs = entry.age_secs().saturating_sub(entry.ttl_secs);
                    if max_stale.is_none_or(|ms| stale_secs <= ms) {
                        IsrCacheState::Stale(entry.html.clone())
                    } else {
                        IsrCacheState::Miss
                    }
                }
            }
        }
    }

    /// Attempt to set the `revalidating` flag. Returns `true` if this caller
    /// should spawn the revalidation task (thundering-herd coalescing).
    pub async fn begin_revalidation(&self, key: &str) -> bool {
        match self.map.get_mut(key) {
            None => true,
            Some(mut entry) => {
                if entry.revalidating {
                    false
                } else {
                    entry.revalidating = true;
                    true
                }
            }
        }
    }

    /// Clear the `revalidating` flag after a revalidation task finishes.
    pub async fn end_revalidation(&self, key: &str) {
        if let Some(mut entry) = self.map.get_mut(key) {
            entry.revalidating = false;
        }
    }

    /// Write a rendered HTML string to the cache with the given TTL and tags.
    pub async fn store(&self, key: &str, html: String, ttl_secs: u64, tags: Vec<String>) {
        let entry = CacheEntry::new(key.to_string(), html, ttl_secs, tags);
        self.map.insert(key.to_string(), entry);
    }

    /// Remove all cache entries whose key starts with `path`.
    pub fn invalidate_path(&self, path: &str) {
        self.map.retain(|k, _| !k.starts_with(path));
    }
}

// ── IsrHandle ─────────────────────────────────────────────────

/// Per-request SSG cache handle attached to `req.cache`.
///
/// Provides path-based cache invalidation (`revalidate`).
/// The `__arc()` accessor is used internally by generated SSG handler code.
#[derive(Clone, Default)]
pub struct IsrHandle {
    cache: Option<Arc<IsrCache>>,
}

impl IsrHandle {
    pub(crate) fn new(cache: Arc<IsrCache>) -> Self {
        Self { cache: Some(cache) }
    }

    /// Invalidate all cached entries whose URL path starts with `path`.
    ///
    /// ```rust,ignore
    /// pub async fn update_product(req: Req) -> ActionResult {
    ///     db.update_product(&req.form).await?;
    ///     req.cache.revalidate("/products/1");
    ///     redirect("/products")
    /// }
    /// ```
    pub fn revalidate(&self, path: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate_path(path);
        }
    }

    /// Return an `Arc<IsrCache>` for use in `tokio::spawn` tasks.
    #[doc(hidden)]
    pub fn __arc(&self) -> Option<Arc<IsrCache>> {
        self.cache.clone()
    }
}

impl std::fmt::Debug for IsrHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IsrHandle")
            .field("enabled", &self.cache.is_some())
            .finish()
    }
}

// ── Cache key computation ─────────────────────────────────────

/// Compute the SSG cache key: `{path}?{sorted_query}`.
///
/// Used by generated SSG handler code — not part of the public API.
#[doc(hidden)]
pub fn __isr_cache_key(
    path: &str,
    query: &FormMap,
    _vary_keys: &[&str],
    _cookies: &CookieJar,
    _headers: &HeaderMap,
) -> String {
    let mut pairs: Vec<(String, String)> = query
        .0
        .iter()
        .flat_map(|(k, vs)| vs.iter().map(move |v| (k.clone(), v.clone())))
        .collect();
    pairs.sort();
    let qs = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    if qs.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{qs}")
    }
}
