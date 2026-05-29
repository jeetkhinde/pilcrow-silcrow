# 03 — FSR Dashboard

The tickets page is a multi-counter FSR dashboard. Six live counters — total, open, and four priority tiers — all backed by a single SQL query.

## `pages/tickets/index.rs`

```rust
use pilcrow_web::AppError;
use pilcrow_web::live::*;

#[derive(sqlx::FromRow)]
pub struct Ticket {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub priority: String,
}

pub struct Props {
    pub tickets: Vec<Ticket>,
    pub live: Live,
}

pub struct Live {
    pub total_count: LiveProp<i64>,
    pub open_count: LiveProp<i64>,
    pub normal_count: LiveProp<i64>,
    pub high_count: LiveProp<i64>,
    pub very_high_count: LiveProp<i64>,
    pub critical_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT
               COUNT(*)::bigint                                    AS total_count,
               COUNT(*) FILTER (WHERE status = 'open')::bigint    AS open_count,
               COUNT(*) FILTER (WHERE priority = 'normal')::bigint AS normal_count,
               COUNT(*) FILTER (WHERE priority = 'high')::bigint   AS high_count,
               COUNT(*) FILTER (WHERE priority = 'very_high')::bigint AS very_high_count,
               COUNT(*) FILTER (WHERE priority = 'critical')::bigint  AS critical_count
             FROM tickets"
        )
    }
}

pub async fn load(_req: Req, live: Live) -> AppResult<Props> {
    let pool = crate::db::pool();

    let tickets = sqlx::query_as::<_, Ticket>(
        "SELECT id, title, status, priority FROM tickets ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::Internal)?;

    Ok(Props { tickets, live })
}
```

Six `LiveProp<i64>` fields, one `Live::query()`. The watcher executes this query once per stale-slot cycle and extracts each column by name using `column_name` in the `pilcrow_fsr` table. All six slots share the same stored SQL — when `/tickets` is invalidated, one query re-run refreshes all six counters.

## Template excerpt

```html
<div class="stats">
  <div class="stat-card">
    <div class="stat-label">Total Tickets</div>
    <div class="stat-value">{{ live.total_count.value }}</div>
  </div>
  <div class="stat-card">
    <div class="stat-label">Open</div>
    <div class="stat-value">{{ live.open_count.value }}</div>
  </div>
</div>

<div class="priority-grid">
  <div class="priority-card p-normal">
    <div class="stat-label">Normal</div>
    <div class="stat-value">{{ live.normal_count.value }}</div>
  </div>
  <!-- high, very_high, critical … -->
</div>
```

Routekit auto-inserts `s-live` for every text-node use of `{{ live.field.value }}`. No explicit `s-live` attributes needed on these scalar fields.

## Invalidation from the API

The tickets API route (`api/tickets.rs`) updates the database and then invalidates:

```rust
fsr_store.invalidate_dep_key(&format!("tickets:id={id}")).await.ok();
fsr_store.invalidate_route("/tickets").await.ok();
```

`invalidate_route("/tickets")` marks all six counter slots stale. The watcher detects this, re-runs the single aggregate query, and pushes six SSE events. Every connected browser tab showing the dashboard updates all six counters simultaneously.

## `pilcrow_fsr` after a request

```sql
SELECT route, slot, depends_on FROM pilcrow_fsr
WHERE route = '/tickets' AND slot != ''
ORDER BY slot;
```

```
  route    |     slot      |          depends_on
-----------+---------------+-------------------------------
 /tickets  | critical_count | {page_tickets::__revalidate_default}
 /tickets  | high_count     | {page_tickets::__revalidate_default}
 /tickets  | normal_count   | {page_tickets::__revalidate_default}
 /tickets  | open_count     | {page_tickets::__revalidate_default}
 /tickets  | total_count    | {page_tickets::__revalidate_default}
 /tickets  | very_high_count| {page_tickets::__revalidate_default}
```

All six slots share the same `depends_on` dep key (the default revalidation timer). `invalidate_route("/tickets")` marks all of them stale regardless of dep key.

---

Next: [[04 Ticket Detail — Scalar vs Object]]
