use super::model::LiveFieldData;
use crate::baked_pages::DependencyKey;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct LivePageStore {
    pool: Arc<PgPool>,
}

impl LivePageStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── Write ─────────────────────────────────────────────────────────────────

    /// Upsert live field values into `pilcrow_cache`. Called after every render.
    pub async fn write_live_fields(
        &self,
        route: &str,
        params: &serde_json::Value,
        fields: &[LiveFieldData],
    ) -> sqlx::Result<()> {
        for field in fields {
            let dep_keys: Vec<String> = field
                .depends_on
                .iter()
                .map(|k| k.as_str().to_string())
                .collect();
            let dep_array = serde_json::to_value(&dep_keys).unwrap_or_default();
            sqlx::query(
                r#"
                INSERT INTO pilcrow_cache
                    (route, slot, params, value, depends_on, hit_count, last_hit)
                VALUES ($1, $2, $3, $4, $5::text[], 1, now())
                ON CONFLICT (route, slot, params) DO UPDATE SET
                    value      = EXCLUDED.value,
                    depends_on = EXCLUDED.depends_on,
                    hit_count  = pilcrow_cache.hit_count + 1,
                    last_hit   = now(),
                    stale      = FALSE
                "#,
            )
            .bind(route)
            .bind(&field.field_name)
            .bind(params)
            .bind(&field.json_value)
            .bind(&dep_keys as &[String])
            .execute(&*self.pool)
            .await?;
            let _ = dep_array;
        }
        Ok(())
    }

    // ── Read ──────────────────────────────────────────────────────────────────

    /// Read current (non-stale) slot values for a route + params.
    /// Returns `(field_name, json_value)` pairs.
    pub async fn read_live_slots(
        &self,
        route: &str,
        params: &serde_json::Value,
    ) -> sqlx::Result<Vec<(String, serde_json::Value)>> {
        let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
            "SELECT slot, value FROM pilcrow_cache WHERE route = $1 AND params = $2 AND stale = FALSE",
        )
        .bind(route)
        .bind(params)
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Invalidation ──────────────────────────────────────────────────────────

    /// Mark all `pilcrow_cache` rows whose `depends_on` contains `dep_key` as stale.
    /// Returns the distinct routes that were affected.
    pub async fn invalidate_dep_key(&self, dep_key: &DependencyKey) -> sqlx::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            UPDATE pilcrow_cache
               SET stale = TRUE, version = version + 1
             WHERE depends_on @> ARRAY[$1::text]
            RETURNING route
            "#,
        )
        .bind(dep_key.as_str())
        .fetch_all(&*self.pool)
        .await?;
        let mut routes: Vec<String> = rows.into_iter().map(|(r,)| r).collect();
        routes.sort();
        routes.dedup();
        Ok(routes)
    }

    // ── Hit tracking + promotion ──────────────────────────────────────────────

    /// Increment `hit_count` for a route and return `true` if the route just
    /// crossed the `promote_after` threshold for the first time.
    pub async fn increment_hit(
        &self,
        route: &str,
        promote_after: Option<u32>,
    ) -> sqlx::Result<bool> {
        let row: (i32, bool) = sqlx::query_as(
            r#"
            INSERT INTO pilcrow_routes (route, hit_count)
            VALUES ($1, 1)
            ON CONFLICT (route) DO UPDATE
                SET hit_count = pilcrow_routes.hit_count + 1
            RETURNING hit_count, promoted
            "#,
        )
        .bind(route)
        .fetch_one(&*self.pool)
        .await?;

        let (hit_count, already_promoted) = row;
        let just_crossed = if let Some(threshold) = promote_after {
            !already_promoted && hit_count >= threshold as i32
        } else {
            false
        };

        if just_crossed {
            sqlx::query("UPDATE pilcrow_routes SET promoted = TRUE WHERE route = $1")
                .bind(route)
                .execute(&*self.pool)
                .await?;
        }

        Ok(just_crossed)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baked_pages::DependencyKey;
    use serde_json::json;

    async fn test_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for live-props integration tests");
        PgPool::connect(&url)
            .await
            .expect("failed to connect to DATABASE_URL")
    }

    fn make_status_field(dep_key: &str) -> LiveFieldData {
        LiveFieldData {
            field_name: "status".to_string(),
            json_value: json!("Open"),
            depends_on: vec![DependencyKey::new(dep_key)],
            patch_debounce: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing at a database with pilcrow migrations applied"]
    async fn write_and_read_live_fields() {
        let store = LivePageStore::new(test_pool().await);
        let fields = vec![make_status_field("tickets:id=write-test")];

        store
            .write_live_fields("/tickets/:id", &json!({"id": "write-test"}), &fields)
            .await
            .unwrap();

        let slots = store
            .read_live_slots("/tickets/:id", &json!({"id": "write-test"}))
            .await
            .unwrap();
        assert!(
            slots
                .iter()
                .any(|(name, val)| name == "status" && val == &json!("Open")),
            "expected status slot to be 'Open', got: {slots:?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn invalidate_dep_key_marks_rows_stale() {
        let store = LivePageStore::new(test_pool().await);
        let dep = DependencyKey::new("tickets:id=invalidate-test");
        let fields = vec![LiveFieldData {
            field_name: "status".to_string(),
            json_value: json!("Open"),
            depends_on: vec![dep.clone()],
            promote_after: None,
            patch_debounce: None,
        }];
        store
            .write_live_fields("/tickets/:id", &json!({"id": "invalidate-test"}), &fields)
            .await
            .unwrap();

        let affected = store.invalidate_dep_key(&dep).await.unwrap();
        assert!(
            affected.contains(&"/tickets/:id".to_string()),
            "expected /tickets/:id in affected routes, got: {affected:?}"
        );

        let slots = store
            .read_live_slots("/tickets/:id", &json!({"id": "invalidate-test"}))
            .await
            .unwrap();
        assert!(
            slots.is_empty(),
            "stale slot should not be returned by read_live_slots, got: {slots:?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn increment_hit_promotes_at_threshold() {
        let store = LivePageStore::new(test_pool().await);
        let route = "/promote-test-route-unique-42";

        // Clean up any leftover state from previous runs.
        sqlx::query("DELETE FROM pilcrow_routes WHERE route = $1")
            .bind(route)
            .execute(store.pool())
            .await
            .unwrap();

        let promoted = store.increment_hit(route, Some(3)).await.unwrap();
        assert!(!promoted, "should not promote on first hit");

        store.increment_hit(route, Some(3)).await.unwrap();
        let promoted = store.increment_hit(route, Some(3)).await.unwrap();
        assert!(
            promoted,
            "should promote when hit_count reaches threshold of 3"
        );

        // Further hits should not report promotion again.
        let promoted_again = store.increment_hit(route, Some(3)).await.unwrap();
        assert!(!promoted_again, "promotion flag should only fire once");
    }
}
