use super::baking::inject_fsr_slots;
use super::store::{FsrStore, StaleSlot};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

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
    /// The dependency key to invalidate (must match `depends_on` in affected live.rs fields).
    pub dep_key: String,
    /// How often to fire the invalidation.
    pub interval: Duration,
}

impl ScheduledInvalidation {
    pub fn new(dep_key: impl Into<String>, interval: Duration) -> Self {
        Self { dep_key: dep_key.into(), interval }
    }
}

/// Configuration for the embedded FSR watcher.
#[derive(Debug, Clone, Default)]
pub struct WatcherConfig {
    /// How often to poll for stale rows (polling mode or pub/sub fallback).
    pub poll_interval_ms: u64,
    /// Framework default promote_after_hits (per-field can override via pilcrow_fsr).
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
}

impl WatcherConfig {
    pub fn new() -> Self {
        Self {
            poll_interval_ms: 500,
            promote_after_hits: 100,
            patch_debounce_secs: 30,
            purge_after_seconds: 2_592_000,
            scheduled_invalidations: Vec::new(),
        }
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

    for slot_row in stale {
        let value = match re_execute_query(store, &slot_row).await {
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

        if slot_row.promoted {
            if let Some(ref html_path) = slot_row.html_path {
                patch_html_file(html_path, &slot_row.slot, &value).await;
            }
            if let Some(ref json_path) = slot_row.json_path {
                patch_json_file(json_path, &slot_row.slot, &value).await;
            }
        }

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

    for slot_row in stale {
        let value = match re_execute_query(store, &slot_row).await {
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

        // Patch disk files for promoted routes and capture patched HTML.
        let patched_html: Option<String> = if slot_row.promoted {
            let html = if let Some(ref html_path) = slot_row.html_path {
                patch_html_file_returning(html_path, &slot_row.slot, &value).await
            } else {
                None
            };
            if let Some(ref json_path) = slot_row.json_path {
                patch_json_file(json_path, &slot_row.slot, &value).await;
            }
            html
        } else {
            None
        };

        // 1. Update Redis slot HASH.
        let value_str = value_to_string(&value);
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

        // 2. Push fresh HTML into Redis for promoted routes.
        if slot_row.promoted {
            if let Some(ref html) = patched_html {
                if let Err(e) = redis.set_html(&slot_row.route, html).await {
                    tracing::warn!(
                        route = %slot_row.route,
                        error = %e,
                        "FSR watcher: Redis set_html failed"
                    );
                }
            }
            // 3. Patch Redis JSON if this route opted in.
            if slot_row.json_path.is_some() {
                let existing = redis
                    .get_json(&slot_row.route)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| serde_json::json!({}));
                let mut map = match existing {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
                map.insert(slot_row.slot.clone(), value.clone());
                if let Err(e) = redis
                    .set_json(&slot_row.route, &serde_json::Value::Object(map))
                    .await
                {
                    tracing::warn!(
                        route = %slot_row.route,
                        error = %e,
                        "FSR watcher: Redis set_json failed"
                    );
                }
            }
        }

        // 4. Publish to pilcrow:patch for multi-pod SSE fanout.
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

        // 5. Broadcast in-process SSE event (single-pod clients).
        if let Some(tx) = event_tx {
            let _ = tx.send(SlotPatch {
                route: slot_row.route.clone(),
                slot: slot_row.slot.clone(),
                value: value.clone(),
            });
        }

        // 6. Mark fresh in Postgres.
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

/// Patch an `s-live` slot in a baked HTML file on disk (fire and forget).
async fn patch_html_file(html_path: &str, slot: &str, value: &serde_json::Value) {
    patch_html_file_returning(html_path, slot, value).await;
}

/// Patch an `s-live` slot in a baked HTML file on disk, returning the patched HTML.
async fn patch_html_file_returning(
    html_path: &str,
    slot: &str,
    value: &serde_json::Value,
) -> Option<String> {
    match tokio::fs::read_to_string(html_path).await {
        Ok(html) => {
            let slots = vec![(slot.to_string(), value.clone())];
            let patched = inject_fsr_slots(&html, &slots);
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

/// Patch a JSON field in a baked JSON file on disk.
async fn patch_json_file(json_path: &str, slot: &str, value: &serde_json::Value) {
    match tokio::fs::read_to_string(json_path).await {
        Ok(content) => {
            let mut obj: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            if let serde_json::Value::Object(ref mut map) = obj {
                map.insert(slot.to_string(), value.clone());
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

/// Serialise a JSON value to a compact string suitable for Redis HSET storage.
#[cfg(feature = "live-props-redis")]
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Start the embedded watcher in polling mode (no Redis).
///
/// Also spawns one background timer task per `config.scheduled_invalidations` entry.
/// These tasks call `invalidate_dep_key` at the configured interval, replacing the
/// old `REVALIDATE` TTL pattern for FSR routes.
pub fn spawn_embedded_watcher(
    store: Arc<FsrStore>,
    config: WatcherConfig,
    event_tx: Option<WatcherEventTx>,
) -> tokio::task::JoinHandle<()> {
    // Spawn one timer task per scheduled invalidation before starting the main loop.
    for scheduled in config.scheduled_invalidations.iter().cloned() {
        let store_clone = Arc::clone(&store);
        tokio::spawn(async move {
            let mut ticker = time::interval(scheduled.interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = store_clone.invalidate_dep_key(&scheduled.dep_key).await {
                    tracing::error!(
                        dep_key = %scheduled.dep_key,
                        error = %e,
                        "FSR: scheduled invalidation failed"
                    );
                }
            }
        });
    }

    tokio::spawn(async move {
        let poll_interval = if config.poll_interval_ms > 0 {
            Duration::from_millis(config.poll_interval_ms)
        } else {
            Duration::from_millis(500)
        };
        let mut ticker = time::interval(poll_interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Err(e) = watcher_tick(&store, event_tx.as_ref()).await {
                tracing::error!(error = %e, "FSR watcher tick failed");
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
/// (same as the non-Redis variant).
#[cfg(feature = "live-props-redis")]
pub fn spawn_embedded_watcher_redis(
    store: Arc<FsrStore>,
    config: WatcherConfig,
    event_tx: Option<WatcherEventTx>,
    redis: Arc<RedisCache>,
) -> tokio::task::JoinHandle<()> {
    use futures_util::StreamExt as _;

    // Spawn scheduled invalidation timer tasks.
    for scheduled in config.scheduled_invalidations.iter().cloned() {
        let store_clone = Arc::clone(&store);
        tokio::spawn(async move {
            let mut ticker = time::interval(scheduled.interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = store_clone.invalidate_dep_key(&scheduled.dep_key).await {
                    tracing::error!(
                        dep_key = %scheduled.dep_key,
                        error = %e,
                        "FSR: scheduled invalidation failed"
                    );
                }
            }
        });
    }

    tokio::spawn(async move {
        let fallback_interval = Duration::from_millis(config.poll_interval_ms);

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
                        match tokio::time::timeout(Duration::from_secs(60), msg_stream.next())
                            .await
                        {
                            // Invalidation message received — tick immediately.
                            Ok(Some(_)) => {
                                if let Err(e) =
                                    watcher_tick_redis(&store, event_tx.as_ref(), &redis).await
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
                                    watcher_tick_redis(&store, event_tx.as_ref(), &redis).await
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
            if let Err(e) = watcher_tick_redis(&store, event_tx.as_ref(), &redis).await {
                tracing::error!(error = %e, "FSR watcher: fallback tick failed");
            }
            tokio::time::sleep(fallback_interval).await;
        }
    })
}

/// Execute a single watcher tick from an external process (no Redis).
pub async fn pilcrow_fsr_watcher_tick(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
    let store = FsrStore::new(pool);
    watcher_tick(&store, None).await
}
