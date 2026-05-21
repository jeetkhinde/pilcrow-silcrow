use std::sync::Arc;

use super::store::FsrStore;

/// Per-request FSR handle attached to `req.fsr`.
///
/// Provides the public FSR mutation API. All methods are no-ops when FSR is
/// not configured (no Postgres URL in `Pilcrow.toml`).
///
/// # Example
///
/// ```rust,ignore
/// pub async fn delete_ticket(req: Req) -> ActionResult {
///     db::delete_ticket(&req.params["id"]).await?;
///     // Mark the baked artifact dead — next request returns 404.
///     req.fsr.tombstone(&req.path).await;
///     redirect("/tickets")
/// }
/// ```
#[derive(Clone, Default, Debug)]
pub struct FsrHandle {
    store: Option<Arc<FsrStore>>,
}

impl FsrHandle {
    pub(crate) fn new(store: Arc<FsrStore>) -> Self {
        Self { store: Some(store) }
    }

    /// Mark a promoted route as tombstoned.
    ///
    /// Sets `tombstoned = TRUE` in Postgres, evicts Redis keys, and removes
    /// baked HTML/JSON files from disk. The next request to `route` returns 404.
    ///
    /// Silently no-ops when FSR is not configured or the route has no
    /// route-level row (i.e. it was never promoted).
    pub async fn tombstone(&self, route: &str) {
        let Some(ref store) = self.store else { return };
        if let Err(e) = store.tombstone(route).await {
            tracing::warn!(route, error = %e, "fsr.tombstone: DB error");
        }
    }

    /// Mark all slot rows for a route as stale, triggering a watcher re-bake.
    ///
    /// Use this to force re-baking when data changes outside the normal dep
    /// tracking path (e.g. a bulk import that doesn't go through action handlers).
    pub async fn invalidate_route(&self, route: &str) {
        let Some(ref store) = self.store else { return };
        if let Err(e) = store.invalidate_route(route).await {
            tracing::warn!(route, error = %e, "fsr.invalidate_route: DB error");
        }
    }

    /// Returns `true` when the route has a tombstoned route-level FSR row.
    pub async fn is_tombstoned(&self, route: &str) -> bool {
        let Some(ref store) = self.store else {
            return false;
        };
        store.is_tombstoned(route).await.unwrap_or_else(|e| {
            tracing::warn!(route, error = %e, "fsr.is_tombstoned: DB error");
            false
        })
    }

    /// Trigger a background pre-bake of the given path.
    ///
    /// Spawns a fire-and-forget GET to the local server at `path`, warming the
    /// FSR/HTTP cache before the user navigates there. Typical use: call this
    /// at the end of a paginated handler with the next cursor URL.
    ///
    /// No-op when [`crate::prebake::set_local_base`] has not been called (i.e.
    /// before `start_with_adapter` runs in tests or when the server is not yet up).
    pub fn prebake_next(&self, path: &str) {
        crate::prebake::trigger(path);
    }
}
