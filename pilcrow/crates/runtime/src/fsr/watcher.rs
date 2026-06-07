use super::baking::inject_fsr_slots;
use super::store::{FsrStore, StaleSlot};
use futures_util::StreamExt as _;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// Abort the wrapped task handle when this guard is dropped.
struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Spawn a supervised background task.
///
/// `make_task` is called once per run. On panic the outer loop logs an error,
/// waits 1 s, then calls `make_task` again. Clean exit (`Ok(())`) or
/// cancellation (`JoinError::is_cancelled`) stops the loop.
fn spawn_supervised<F, Fut>(task_name: &'static str, make_task: F) -> tokio::task::JoinHandle<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let child = tokio::spawn(make_task());
            let _guard = AbortOnDrop(child.abort_handle());
            match child.await {
                Ok(()) => break,
                Err(e) if e.is_panic() => {
                    tracing::error!(
                        task = task_name,
                        error = ?e,
                        "FSR background task panicked, restarting in 1s"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(_) => break, // cancelled — don't restart
            }
        }
    })
}

#[cfg(feature = "live-props-redis")]
use super::cache::{PatchPayload, RedisCache};

/// A scheduled dep-key invalidation fired on a fixed interval by the embedded watcher.
///
/// Use this to replace `REVALIDATE` TTL: instead of expiring a cache entry after N seconds,
/// register the dep key that the affected slots `depends_on` and the watcher will call
/// `invalidate_dep_key` on every `interval`, triggering a re-bake of all stale slots.
///
/// ```rust,ignore
/// WatcherConfig {
///     scheduled_invalidations: vec![
///         ScheduledInvalidation::new("exchange_rates", Duration::from_secs(60)),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ScheduledInvalidation {
    /// The dependency key to invalidate (must match `depends_on` in affected Live fields).
    pub dep_key: String,
    /// How often to fire the invalidation.
    pub interval: Duration,
}

impl ScheduledInvalidation {
    pub fn new(dep_key: impl Into<String>, interval: Duration) -> Self {
        Self {
            dep_key: dep_key.into(),
            interval,
        }
    }
}

/// Configuration for the embedded FSR watcher.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// How often to poll for stale rows (polling mode or pub/sub fallback).
    pub poll_interval_ms: u64,
    /// Framework default promote_after_hits. Override route-level via `PROMOTE_AFTER: u32 = N` in `page.rs`.
    pub promote_after_hits: u32,
    /// Framework default patch_debounce_secs.
    pub patch_debounce_secs: u32,
    /// Seconds before a route's baked artefacts are purged.
    pub purge_after_seconds: u64,
    /// Dep keys that the watcher invalidates on a fixed schedule.
    ///
    /// Each entry spawns a dedicated timer task that calls `invalidate_dep_key` at the
    /// specified interval, marking all dependent slots stale for re-baking. This replaces
    /// the old `REVALIDATE` TTL pattern for FSR routes.
    pub scheduled_invalidations: Vec<ScheduledInvalidation>,
    /// How often the idle-eviction task runs, in seconds. `0` disables idle eviction.
    pub idle_evict_secs: u64,
    /// Routes with no traffic for longer than this (in seconds) are un-promoted and
    /// their Redis keys evicted. Only meaningful when `idle_evict_secs > 0`.
    pub idle_threshold_secs: u64,
}

impl WatcherConfig {
    pub fn new() -> Self {
        Self {
            poll_interval_ms: 500,
            promote_after_hits: 100,
            patch_debounce_secs: 0,
            purge_after_seconds: 2_592_000,
            scheduled_invalidations: Vec::new(),
            idle_evict_secs: 1_800,
            idle_threshold_secs: 86_400,
        }
    }
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A channel sender for pushing FSR slot-patch events to the SSE hub.
pub type WatcherEventTx = tokio::sync::broadcast::Sender<SlotPatch>;

/// A single slot-patch event pushed by the watcher.
#[derive(Debug, Clone)]
pub struct SlotPatch {
    pub route: String,
    pub slot: String,
    pub value: serde_json::Value,
}

/// Tick function for polling mode (no Redis).
///
/// - Fetches stale rows from `pilcrow_fsr`
/// - Re-executes stored queries via sqlx
/// - Patches baked HTML/JSON files on disk (promoted routes)
/// - Broadcasts `SlotPatch` events to connected SSE clients
/// - Marks rows fresh
pub async fn watcher_tick(
    store: &FsrStore,
    event_tx: Option<&WatcherEventTx>,
) -> Result<(), sqlx::Error> {
    let stale = store.fetch_stale_slots().await?;

    // Phase 1: run DB queries with bounded concurrency.
    // buffer_unordered keeps the pipeline full (max 8 in-flight); each future
    // carries its slot_row so Phase 2 can iterate directly without a zip.
    let results: Vec<_> = futures_util::stream::iter(stale.into_iter())
        .map(|slot_row| async move {
            let result = re_execute_query(store, &slot_row).await;
            (slot_row, result)
        })
        .buffer_unordered(8)
        .collect()
        .await;

    // Phase 2a: build per-file patch batches from successful results.
    let mut html_patches: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();
    let mut json_patches: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();
    for (slot_row, result) in &results {
        let Ok(value) = result else { continue };
        if slot_row.promoted {
            if let Some(ref p) = slot_row.html_path {
                html_patches
                    .entry(p.clone())
                    .or_default()
                    .push((slot_row.slot.clone(), value.clone()));
            }
            if let Some(ref p) = slot_row.json_path {
                json_patches
                    .entry(p.clone())
                    .or_default()
                    .push((slot_row.slot.clone(), value.clone()));
            }
        }
    }

    // Phase 2b: one read/write per unique file — multiple slots on the same
    // promoted route share a file, so this eliminates redundant I/O.
    for (html_path, patches) in &html_patches {
        patch_html_file_batch(html_path, patches).await;
    }
    for (json_path, patches) in &json_patches {
        patch_json_file_batch(json_path, patches).await;
    }

    // Phase 2c: per-slot — log errors, broadcast SSE, mark fresh.
    for (slot_row, result) in &results {
        let value = match result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    route = %slot_row.route,
                    slot = %slot_row.slot,
                    error = %e,
                    "FSR watcher: failed to re-execute query"
                );
                continue;
            }
        };

        if let Some(tx) = event_tx {
            let _ = tx.send(SlotPatch {
                route: slot_row.route.clone(),
                slot: slot_row.slot.clone(),
                value: value.clone(),
            });
        }

        if let Err(e) = store.mark_fresh(&slot_row.route, &slot_row.slot).await {
            tracing::warn!(
                route = %slot_row.route,
                slot = %slot_row.slot,
                error = %e,
                "FSR watcher: failed to mark slot fresh"
            );
        }
    }

    Ok(())
}

/// Redis-aware tick — same as `watcher_tick` but also:
/// - Writes patched slot values to `pilcrow:slot:<route>` HASH
/// - Writes patched HTML to `pilcrow:html:<route>` for promoted routes
/// - Updates `pilcrow:json:<route>` for routes with FSR_JSON enabled
/// - Publishes `pilcrow:patch` events for multi-pod SSE fanout
#[cfg(feature = "live-props-redis")]
pub async fn watcher_tick_redis(
    store: &FsrStore,
    event_tx: Option<&WatcherEventTx>,
    redis: &RedisCache,
) -> Result<(), sqlx::Error> {
    let stale = store.fetch_stale_slots().await?;

    // Phase 1: run DB queries with bounded concurrency.
    // buffer_unordered keeps the pipeline full (max 8 in-flight); each future
    // carries its slot_row so Phase 2 can iterate directly without a zip.
    let results: Vec<_> = futures_util::stream::iter(stale.into_iter())
        .map(|slot_row| async move {
            let result = re_execute_query(store, &slot_row).await;
            (slot_row, result)
        })
        .buffer_unordered(8)
        .collect()
        .await;

    // Phase 2a: build per-file patch batches and per-route Redis JSON batches.
    let mut html_patches: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();
    let mut json_patches: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();
    // route → slot patches for Redis JSON (one Redis read/write per route).
    let mut redis_json_patches: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();
    for (slot_row, result) in &results {
        let Ok(value) = result else { continue };
        if slot_row.promoted {
            if let Some(ref p) = slot_row.html_path {
                html_patches
                    .entry(p.clone())
                    .or_default()
                    .push((slot_row.slot.clone(), value.clone()));
            }
            if let Some(ref p) = slot_row.json_path {
                json_patches
                    .entry(p.clone())
                    .or_default()
                    .push((slot_row.slot.clone(), value.clone()));
                redis_json_patches
                    .entry(slot_row.route.clone())
                    .or_default()
                    .push((slot_row.slot.clone(), value.clone()));
            }
        }
    }

    // Phase 2b: one read/write per unique file — multiple slots on the same
    // promoted route share a file, so this eliminates redundant I/O.
    // `html_patched` maps html_path → patched HTML string for Redis set_html.
    let mut html_patched: HashMap<String, Option<String>> = HashMap::new();
    for (html_path, patches) in &html_patches {
        let result = patch_html_file_batch_returning(html_path, patches).await;
        html_patched.insert(html_path.clone(), result);
    }
    for (json_path, patches) in &json_patches {
        patch_json_file_batch(json_path, patches).await;
    }

    // Phase 2c: Redis JSON — one read/merge/write per route (not per slot).
    for (route, patches) in &redis_json_patches {
        let existing = redis
            .get_json(route)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut map = match existing {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        for (slot, value) in patches {
            map.insert(slot.clone(), value.clone());
        }
        if let Err(e) = redis
            .set_json(route, &serde_json::Value::Object(map))
            .await
        {
            tracing::warn!(route = %route, error = %e, "FSR watcher: Redis set_json failed");
        }
    }

    // Phase 2d: per-slot — Redis slot HASH, Redis HTML, publish, SSE, mark fresh.
    for (slot_row, result) in &results {
        let value = match result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    route = %slot_row.route,
                    slot = %slot_row.slot,
                    error = %e,
                    "FSR watcher: failed to re-execute query"
                );
                continue;
            }
        };

        // 1. Update Redis slot HASH.
        let value_str = value_to_string(value);
        if let Err(e) = redis
            .patch_slot(&slot_row.route, &slot_row.slot, &value_str)
            .await
        {
            tracing::warn!(
                route = %slot_row.route,
                slot = %slot_row.slot,
                error = %e,
                "FSR watcher: Redis patch_slot failed"
            );
        }

        // 2. Push patched HTML into Redis (uses the batched result from Phase 2b).
        if slot_row.promoted {
            if let Some(html_path) = &slot_row.html_path {
                if let Some(Some(html)) = html_patched.get(html_path.as_str()) {
                    if let Err(e) = redis.set_html(&slot_row.route, html).await {
                        tracing::warn!(
                            route = %slot_row.route,
                            error = %e,
                            "FSR watcher: Redis set_html failed"
                        );
                    }
                }
            }
        }

        // 3. Publish to pilcrow:patch for multi-pod SSE fanout.
        let payload = PatchPayload {
            route: slot_row.route.clone(),
            slot: slot_row.slot.clone(),
            value: value.clone(),
        };
        if let Err(e) = redis.publish_patch(&payload).await {
            tracing::warn!(
                route = %slot_row.route,
                slot = %slot_row.slot,
                error = %e,
                "FSR watcher: Redis publish_patch failed"
            );
        }

        // 4. Broadcast in-process SSE event (single-pod clients).
        if let Some(tx) = event_tx {
            let _ = tx.send(SlotPatch {
                route: slot_row.route.clone(),
                slot: slot_row.slot.clone(),
                value: value.clone(),
            });
        }

        // 5. Mark fresh in Postgres.
        if let Err(e) = store.mark_fresh(&slot_row.route, &slot_row.slot).await {
            tracing::warn!(
                route = %slot_row.route,
                slot = %slot_row.slot,
                error = %e,
                "FSR watcher: failed to mark slot fresh"
            );
        }
    }

    Ok(())
}

/// Re-execute the stored query for a stale slot, returning the new value.
async fn re_execute_query(store: &FsrStore, slot: &StaleSlot) -> sqlx::Result<serde_json::Value> {
    let Some(ref sql) = slot.query else {
        return Ok(serde_json::Value::Null);
    };

    let params: Vec<serde_json::Value> = slot
        .query_params
        .as_ref()
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let row = execute_with_params(store.pool(), sql, &params).await?;

    let col_key = slot.column_name.as_deref().unwrap_or(&slot.slot);
    let value = row
        .as_ref()
        .and_then(|m| m.get(col_key))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(value)
}

/// Execute a parameterised query and return the first row as a JSON map.
pub(crate) async fn execute_with_params(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> sqlx::Result<Option<serde_json::Map<String, serde_json::Value>>> {
    let json_sql = format!("SELECT row_to_json(t) as __row FROM ({sql}) t");

    let mut q = sqlx::query_scalar::<_, serde_json::Value>(&json_sql);
    for p in params {
        let s = match p {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        q = q.bind(s);
    }

    let result = q.fetch_optional(pool).await?;
    Ok(result.and_then(|v| v.as_object().cloned()))
}

/// Patch multiple `s-live` slots in a baked HTML file in one read/write pass.
async fn patch_html_file_batch(html_path: &str, patches: &[(String, serde_json::Value)]) {
    patch_html_file_batch_returning(html_path, patches).await;
}

/// Patch multiple `s-live` slots in a baked HTML file, returning the patched HTML.
async fn patch_html_file_batch_returning(
    html_path: &str,
    patches: &[(String, serde_json::Value)],
) -> Option<String> {
    match tokio::fs::read_to_string(html_path).await {
        Ok(html) => {
            let patched = inject_fsr_slots(&html, patches);
            if let Err(e) = tokio::fs::write(html_path, patched.as_bytes()).await {
                tracing::warn!(
                    path = html_path,
                    error = %e,
                    "FSR watcher: failed to write patched HTML"
                );
                return None;
            }
            Some(patched)
        }
        Err(e) => {
            tracing::warn!(
                path = html_path,
                error = %e,
                "FSR watcher: failed to read HTML for patching"
            );
            None
        }
    }
}

/// Patch multiple JSON fields in a baked JSON file in one read/write pass.
async fn patch_json_file_batch(json_path: &str, patches: &[(String, serde_json::Value)]) {
    match tokio::fs::read_to_string(json_path).await {
        Ok(content) => {
            let mut obj: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            if let serde_json::Value::Object(ref mut map) = obj {
                for (slot, value) in patches {
                    map.insert(slot.clone(), value.clone());
                }
            }
            match serde_json::to_string(&obj) {
                Ok(json) => {
                    if let Err(e) = tokio::fs::write(json_path, json).await {
                        tracing::warn!(
                            path = json_path,
                            error = %e,
                            "FSR watcher: failed to write patched JSON"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        path = json_path,
                        error = %e,
                        "FSR watcher: failed to serialise patched JSON"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                path = json_path,
                error = %e,
                "FSR watcher: failed to read JSON for patching"
            );
        }
    }
}

/// Un-promote idle routes and remove their baked disk artifacts.
///
/// Routes with no traffic for longer than `threshold_secs` have `promoted` reset to
/// `FALSE` and `hit_count` reset to `0`. They re-enter the normal promotion cycle on
/// the next request, so Redis misses fall through safely to `load()`.
async fn idle_evict_tick(store: &FsrStore, threshold_secs: u64) {
    match store.evict_idle_routes(threshold_secs).await {
        Ok(evicted) => {
            for r in evicted {
                tracing::info!(route = %r.route, "FSR: idle eviction");
                schedule_artifact_cleanup(r.html_path, r.json_path);
            }
        }
        Err(e) => tracing::error!(error = %e, "FSR: idle eviction query failed"),
    }
}

/// Un-promote idle routes, delete their Redis keys, and remove baked disk artifacts.
#[cfg(feature = "live-props-redis")]
async fn idle_evict_tick_redis(store: &FsrStore, threshold_secs: u64, redis: &RedisCache) {
    match store.evict_idle_routes(threshold_secs).await {
        Ok(evicted) => {
            for r in evicted {
                tracing::info!(route = %r.route, "FSR: idle eviction");
                redis.delete_route_keys(&r.route).await.ok();
                schedule_artifact_cleanup(r.html_path, r.json_path);
            }
        }
        Err(e) => tracing::error!(error = %e, "FSR: idle eviction query failed"),
    }
}

/// Fire-and-forget removal of baked disk artifacts; errors are non-fatal.
fn schedule_artifact_cleanup(html_path: Option<String>, json_path: Option<String>) {
    tokio::spawn(async move {
        if let Some(p) = html_path {
            tokio::fs::remove_file(&p).await.ok();
        }
        if let Some(p) = json_path {
            tokio::fs::remove_file(&p).await.ok();
        }
    });
}

/// Serialise a JSON value to a compact string suitable for Redis HSET storage.
#[cfg(feature = "live-props-redis")]
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Spawn a supervised background timer that calls `invalidate_dep_key` on a fixed interval.
fn spawn_supervised_invalidation(store: Arc<FsrStore>, scheduled: ScheduledInvalidation) {
    spawn_supervised("FSR scheduled invalidation", move || {
        let store = Arc::clone(&store);
        let dep_key = scheduled.dep_key.clone();
        let interval = scheduled.interval.max(Duration::from_millis(1));
        async move {
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            ticker.tick().await; // skip the immediate first tick at t=0
            loop {
                ticker.tick().await;
                if let Err(e) = store.invalidate_dep_key(&dep_key).await {
                    tracing::error!(
                        dep_key = %dep_key,
                        error = %e,
                        "FSR: scheduled invalidation failed"
                    );
                }
            }
        }
    });
}

/// Start the embedded watcher in polling mode (no Redis).
///
/// Also spawns one background timer task per `config.scheduled_invalidations` entry.
/// These tasks call `invalidate_dep_key` at the configured interval, replacing the
/// old `REVALIDATE` TTL pattern for FSR routes.
///
/// All spawned tasks are supervised: a panic logs an error and the task restarts after
/// a 1-second backoff instead of silently dying. When `config.idle_evict_secs > 0`,
/// also spawns an idle-eviction task that un-promotes routes cold for longer than
/// `config.idle_threshold_secs`.
pub fn spawn_embedded_watcher(
    store: Arc<FsrStore>,
    config: WatcherConfig,
    event_tx: Option<WatcherEventTx>,
) -> tokio::task::JoinHandle<()> {
    for scheduled in config.scheduled_invalidations.iter().cloned() {
        spawn_supervised_invalidation(Arc::clone(&store), scheduled);
    }

    // Idle eviction: un-promote cold routes and remove their baked artifacts.
    // Supervised so a panic restarts the eviction loop instead of silently killing it.
    if config.idle_evict_secs > 0 {
        let store_evict = Arc::clone(&store);
        let threshold = config.idle_threshold_secs;
        let evict_interval = config.idle_evict_secs;
        spawn_supervised("FSR idle eviction", move || {
            let store = Arc::clone(&store_evict);
            async move {
                let mut ticker = time::interval(Duration::from_secs(evict_interval));
                ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                ticker.tick().await; // skip the immediate tick at startup
                loop {
                    ticker.tick().await;
                    idle_evict_tick(&store, threshold).await;
                }
            }
        });
    }

    let poll_interval = if config.poll_interval_ms > 0 {
        Duration::from_millis(config.poll_interval_ms)
    } else {
        Duration::from_millis(500)
    };
    spawn_supervised("FSR watcher", move || {
        let store = Arc::clone(&store);
        let event_tx = event_tx.clone();
        async move {
            let mut ticker = time::interval(poll_interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = watcher_tick(&store, event_tx.as_ref()).await {
                    tracing::error!(error = %e, "FSR watcher tick failed");
                }
            }
        }
    })
}

/// Start the Redis pub/sub-driven embedded watcher.
///
/// Subscribes to `pilcrow:invalidate`. On each message, runs a watcher tick
/// that writes patched values back to Redis and publishes `pilcrow:patch`.
///
/// Falls back to polling every `config.poll_interval_ms` if the pub/sub
/// connection drops, and re-subscribes automatically once Redis is reachable.
///
/// Also spawns one background timer task per `config.scheduled_invalidations` entry
/// (same as the non-Redis variant). All spawned tasks are supervised — panics restart
/// after a 1-second backoff instead of silently dying. When `config.idle_evict_secs > 0`,
/// also spawns an idle-eviction task that deletes Redis keys for cold routes.
#[cfg(feature = "live-props-redis")]
pub fn spawn_embedded_watcher_redis(
    store: Arc<FsrStore>,
    config: WatcherConfig,
    event_tx: Option<WatcherEventTx>,
    redis: Arc<RedisCache>,
) -> tokio::task::JoinHandle<()> {
    for scheduled in config.scheduled_invalidations.iter().cloned() {
        spawn_supervised_invalidation(Arc::clone(&store), scheduled);
    }

    // Idle eviction: un-promote cold routes, evict Redis keys, and remove baked artifacts.
    // Supervised so a panic restarts the eviction loop instead of silently killing it.
    if config.idle_evict_secs > 0 {
        let store_evict = Arc::clone(&store);
        let redis_evict = Arc::clone(&redis);
        let threshold = config.idle_threshold_secs;
        let evict_interval = config.idle_evict_secs;
        spawn_supervised("FSR idle eviction (Redis)", move || {
            let store = Arc::clone(&store_evict);
            let redis = Arc::clone(&redis_evict);
            async move {
                let mut ticker = time::interval(Duration::from_secs(evict_interval));
                ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                ticker.tick().await; // skip the immediate tick at startup
                loop {
                    ticker.tick().await;
                    idle_evict_tick_redis(&store, threshold, &redis).await;
                }
            }
        });
    }

    let fallback_interval =
        Duration::from_millis(config.poll_interval_ms).max(Duration::from_millis(100));
    spawn_supervised("FSR watcher (Redis)", move || {
        let store = Arc::clone(&store);
        let event_tx = event_tx.clone();
        let redis = Arc::clone(&redis);
        async move {
            loop {
                match redis.client().get_async_pubsub().await {
                    Ok(mut pubsub) => {
                        if let Err(e) = pubsub.subscribe("pilcrow:invalidate").await {
                            tracing::warn!(error = %e, "FSR watcher: subscribe failed");
                            tokio::time::sleep(fallback_interval).await;
                            continue;
                        }
                        tracing::info!("FSR watcher: subscribed to pilcrow:invalidate");
                        let msg_stream = pubsub.on_message();
                        tokio::pin!(msg_stream);

                        loop {
                            match tokio::time::timeout(
                                Duration::from_secs(60),
                                msg_stream.next(),
                            )
                            .await
                            {
                                // Invalidation message received — tick immediately.
                                Ok(Some(_)) => {
                                    if let Err(e) =
                                        watcher_tick_redis(&store, event_tx.as_ref(), &redis)
                                            .await
                                    {
                                        tracing::error!(
                                            error = %e,
                                            "FSR watcher: tick failed after invalidation event"
                                        );
                                    }
                                }
                                // Stream ended — connection dropped.
                                Ok(None) => {
                                    tracing::warn!(
                                        "FSR watcher: pub/sub connection closed, switching to poll fallback"
                                    );
                                    break;
                                }
                                // 60 s without a message — reconciliation tick.
                                Err(_timeout) => {
                                    if let Err(e) =
                                        watcher_tick_redis(&store, event_tx.as_ref(), &redis)
                                            .await
                                    {
                                        tracing::error!(
                                            error = %e,
                                            "FSR watcher: reconciliation tick failed"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "FSR watcher: failed to open Redis connection, falling back to polling"
                        );
                    }
                }

                // Polling fallback — drain stale rows accumulated while disconnected,
                // then wait before retrying pub/sub.
                if let Err(e) =
                    watcher_tick_redis(&store, event_tx.as_ref(), &redis).await
                {
                    tracing::error!(error = %e, "FSR watcher: fallback tick failed");
                }
                tokio::time::sleep(fallback_interval).await;
            }
        }
    })
}

/// Execute a single watcher tick from an external process (no Redis).
pub async fn pilcrow_fsr_watcher_tick(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
    let store = FsrStore::new(pool);
    watcher_tick(&store, None).await
}
