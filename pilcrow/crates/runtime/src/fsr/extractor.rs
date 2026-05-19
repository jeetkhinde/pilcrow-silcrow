use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::request::Parts;
use pilcrow_core::AppError;
use serde_json::Value;

use super::PilcrowLive;
use super::store::{FsrStore, HitStatus};

static FSR_STORE: OnceLock<Arc<FsrStore>> = OnceLock::new();

/// Register the FSR store globally. Call once from app init (e.g. hooks.rs::init()).
pub fn register_fsr_store(pool: sqlx::PgPool) -> Arc<FsrStore> {
    let store = Arc::new(FsrStore::new(pool));
    FSR_STORE.set(Arc::clone(&store)).ok();
    store
}

fn global_fsr_store() -> Option<&'static Arc<FsrStore>> {
    FSR_STORE.get()
}

/// Returns a clone of the global FSR store for constructing a [`FsrHandle`].
///
/// Returns `None` when FSR is not configured (no Postgres connection).
pub fn fsr_store_for_handle() -> Option<Arc<FsrStore>> {
    FSR_STORE.get().cloned()
}

/// Runtime helper called from the generated `FromRequestParts` impl for each `Live` type.
///
/// Extracts route params, executes `Live::query()`, calls `Live::from_row()`,
/// then registers slots and increments hit count via the FSR store.
///
/// Gracefully degrades to an empty Live struct if no store is registered or
/// if the query fails.
pub async fn extract_live_from_parts<T: PilcrowLive>(parts: &mut Parts) -> Result<T, AppError> {
    // Build route params map from axum matched path params.
    let raw = RawPathParams::from_request_parts(parts, &()).await;
    let mut params_map = serde_json::Map::new();
    if let Ok(ref rp) = raw {
        for (k, v) in rp.iter() {
            params_map.insert(k.to_string(), Value::String(v.to_string()));
        }
    }

    let Some(store) = global_fsr_store() else {
        // No store registered — return default-populated Live.
        let row: HashMap<String, Value> = HashMap::new();
        return Ok(T::from_row(&row, &params_map));
    };

    let route = parts.uri.path().to_string();
    let live_query = T::query(&params_map);

    // Execute via row_to_json (same pattern as watcher).
    let json_sql = format!("SELECT row_to_json(t) AS __row FROM ({}) t", live_query.sql);
    let mut q = sqlx::query_scalar::<_, Value>(&json_sql);
    for p in &live_query.params {
        let s = match p {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        q = q.bind(s);
    }

    let row_map: HashMap<String, Value> = match q.fetch_optional(store.pool()).await {
        Ok(Some(v)) => v
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        _ => HashMap::new(),
    };

    let live = T::from_row(&row_map, &params_map);

    // Register slots and track hits — fire-and-forget, don't fail the request.
    let fields = T::live_fields(&params_map);
    let query_params_json = Value::Array(live_query.params.clone());

    // Prefer the route-level PROMOTE_AFTER / PRERENDER constant over per-field values.
    let route_promote_after =
        T::route_promote_after().or_else(|| fields.first().and_then(|f| f.promote_after));
    store
        .ensure_route_row(&route, route_promote_after)
        .await
        .ok();

    for field in &fields {
        store
            .upsert_slot(
                &route,
                field.slot,
                live_query.sql,
                &query_params_json,
                &field.depends_on,
                field.promote_after,
                field.patch_debounce,
                field.column_name,
            )
            .await
            .ok();
    }

    match store.increment_hit(&route).await {
        Ok(HitStatus::Tombstoned) => {
            return Err(AppError::NotFound(format!(
                "route {route} has been tombstoned"
            )));
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(route, error = %e, "FSR: increment_hit failed");
        }
    }

    Ok(live)
}
