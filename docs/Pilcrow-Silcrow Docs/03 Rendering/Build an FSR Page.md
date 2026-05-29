# Build an FSR Page

This guide teaches the FSR shape end to end.

FSR stands for Field-Selective Rendering. A route has one page template and one code-behind file. The code-behind defines normal page data plus an inline `Live` struct for fields the framework watcher can refresh without calling `load()`.

## What You Will Build

A tickets dashboard with:

- SSR table data loaded by `index.rs`.
- Live counters declared by inline `Live`.
- Automatic `s-live` insertion for `{{ live.field.value }}`.
- Optional route promotion with `PROMOTE_AFTER`.
- Optional scheduled revalidation with `#[revalidate(N)]`.
- Optional debounce metadata with `#[debounce(N)]`.

## File Layout

```text
pages/
  tickets/
    index.html
    index.rs
```

For larger pages, split the UI into islands or fragments. Each page, island, or fragment can own its own inline `Live` contract.

## Step 1: Enable the Feature

Your app must enable the `live-props` feature on `pilcrow-web`.

```toml
pilcrow-web = { path = "../pilcrow/crates/web", features = ["live-props"] }
```

Use `live-props-redis` only when you want Redis as the hot cache and pub/sub layer.

## Step 2: Define `Live` in `index.rs`

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

#[debounce(30)]
pub struct Live {
    pub total_count: LiveProp<i64>,
    pub open_count: LiveProp<i64>,

    #[debounce(5)]
    pub critical_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT \
             COUNT(*)::bigint AS total_count, \
             COUNT(*) FILTER (WHERE status = 'open')::bigint AS open_count, \
             COUNT(*) FILTER (WHERE priority = 'critical')::bigint AS critical_count \
             FROM tickets"
        )
    }
}
```

What matters:

- The struct is named `Live`.
- `Props` contains `pub live: Live`.
- Each live field is `LiveProp<T>`.
- SQL aliases match field names.
- `Live::query()` returns a `LiveQuery`.
- `#[debounce(30)]` on the struct is the default for fields.
- `#[debounce(5)]` on a field overrides the struct default.

Current debounce caveat: debounce is parsed and stored as FSR metadata, but the embedded watcher still patches stale slots immediately. Treat it as forward-compatible metadata until runtime coalescing is implemented.

## Step 3: Use `Live` in `load()`

```rust
pub const PROMOTE_AFTER: u32 = 50;

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

What matters:

- `load()` accepts `live: Live`.
- FSR route promotion is controlled by `PROMOTE_AFTER`.
- `PROMOTE_AFTER = 0` means bake on first hit.
- Omit `PROMOTE_AFTER` if the route should remain normal SSR plus live patching.

## Step 4: Render Live Values Normally

```html
<section class="stats">
  <article>
    <h2>Total</h2>
    <strong>{{ live.total_count.value }}</strong>
  </article>

  <article>
    <h2>Open</h2>
    <strong>{{ live.open_count.value }}</strong>
  </article>

  <article>
    <h2>Critical</h2>
    <strong>{{ live.critical_count.value }}</strong>
  </article>
</section>
```

Routekit rewrites text-node uses of `{{ live.field.value }}` into:

```html
<span s-live="field">{{ live.field.value }}</span>
```

You only write explicit `s-live` when you need manual control.

## Step 5: Use Route Params with `depends_on_route`

For dynamic routes, reference the route param from the inline `Live` struct.

```text
pages/tickets/[id]/
  index.html
  index.rs
```

```rust
use pilcrow_web::live::*;

#[pilcrow::depends_on_route(tickets, id)]
pub struct Live {
    pub status: LiveProp<String>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("0");

        live_query!(
            "SELECT status FROM tickets WHERE id = $1::bigint",
            id
        )
    }
}
```

What matters:

- `depends_on_route(tickets, id)` requires the route to have an `id` param.
- It creates a dependency relationship for the route.
- Use explicit `#[pilcrow::depends_on(dep!(...))]` when the dependency value is not a route param.

## Step 6: Use `allow_unused` for Object Fields

Some live fields should update a Silcrow atom instead of a direct text slot.

```rust
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct PriorityBadge {
    pub text: String,
    pub class: String,
    pub raw: String,
}

pub struct Live {
    pub status: LiveProp<String>,

    #[pilcrow::allow_unused]
    pub priority: LiveProp<PriorityBadge>,
}
```

```html
<span>{{ live.status.value }}</span>
<span class="{{ live.priority.value.class }}" s-use="fsr.priority">
  {{ live.priority.value.text }}
</span>
```

Use `#[pilcrow::allow_unused]` when there is no direct text-node `{{ live.priority.value }}` use and no explicit `s-live="priority"` slot.

## Step 7: Add Scheduled Revalidation

`#[revalidate(N)]` belongs on `LiveProp<T>` fields in `Props`, not on fields in inline `Live`.

```rust
pub struct Props {
    pub live: Live,

    #[revalidate(60)]
    pub price: LiveProp<f64>,

    #[revalidate(60)]
    pub market_cap: LiveProp<f64>,

    #[revalidate(30)]
    pub volume: LiveProp<u64>,

    pub summary: LiveProp<String>,
}
```

Precedence:

1. Field-level `#[revalidate(N)]`.
2. `[fsr] revalidate_seconds` in `Pilcrow.toml`.
3. 86400 seconds, or 24 hours.

Fields on the same route with the same interval share one internal timer. Different intervals create separate timers.

Use `#[depends_on("key")]` on a `Props` `LiveProp<T>` field when it should use a static dependency key instead of a timer. Do not combine `#[depends_on("key")]` with `#[revalidate(N)]` on the same field.

## Step 8: Configure Global Fallbacks

```toml
[fsr]
revalidate_seconds = 3600
artifact_ttl_secs = 86400
idle_evict_secs = 1800
idle_threshold_secs = 86400
```

Meaning:

- `revalidate_seconds`: fallback timer for eligible fields without `#[revalidate(N)]`.
- `artifact_ttl_secs`: Redis TTL for baked artifacts.
- `idle_evict_secs`: how often idle eviction runs. Set `0` to disable.
- `idle_threshold_secs`: how long a route must be cold before un-promotion.

Redis mode also requires:

```toml
[fsr]
redis_url = "redis://127.0.0.1:6379"
```

## Step 9: Know the Error Cases

This fails:

```html
<span s-live="ticket_status">{{ live.status.value }}</span>
```

when inline `Live` only has:

```rust
pub struct Live {
    pub status: LiveProp<String>,
}
```

This also fails:

```html
<span s-live="status"></span>
<strong s-live="status"></strong>
```

because duplicate `s-live` slots are ambiguous.

This fails too:

```rust
#[pilcrow::depends_on_route(tickets, id)]
pub struct Live {
    pub status: LiveProp<String>,
}
```

when the route path has no `[id]` segment.

## Mental Model

`load()` answers: what does the full page need on request?

`Live::query()` answers: what small fields can the watcher refresh later?

`index.html` connects them with:

- `{{ live.field.value }}` for initial SSR output and automatic `s-live` patching.
- `s-use="fsr.field"` for object-shaped live data.
