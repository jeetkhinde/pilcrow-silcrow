# Live Props and FSR

This page is the concept reference. For a complete build walkthrough, read [[Build an FSR Page]].

## Two Related Surfaces

Pilcrow has two live-field surfaces:

1. `LiveProp<T>` fields directly in `Props`.
2. Inline `Live` fields for FSR watcher-managed data.

They overlap, but they are not the same authoring model.

## `LiveProp<T>` in `Props`

Use this when a normal SSR page needs a field that updates from a developer-managed producer such as a watch channel, poller, or stream.

```rust
pub struct Props {
    pub count: LiveProp<u64>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        count: LiveProp::watch(counter_tx().subscribe()),
    })
}
```

Template:

```html
<p>{{ count }}</p>
```

Pilcrow instruments the rendered field so Silcrow can patch it.

## FSR with Inline `Live`

Use this when the framework watcher should refresh fields from a `LiveQuery`.

```rust
use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub struct Live {
    pub open_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT COUNT(*) FILTER (WHERE status = 'open')::bigint AS open_count FROM tickets"
        )
    }
}

pub async fn load(live: Live) -> AppResult<Props> {
    Ok(Props { live })
}
```

Template:

```html
<span>{{ live.open_count.value }}</span>
```

Routekit auto-inserts `s-live="open_count"` for text-node uses of `{{ live.open_count.value }}`.

## Revalidation

`#[revalidate(N)]` is for `Props` `LiveProp<T>` fields.

```rust
pub struct Props {
    #[revalidate(60)]
    pub price: LiveProp<f64>,

    #[revalidate(60)]
    pub market_cap: LiveProp<f64>,

    #[revalidate(30)]
    pub volume: LiveProp<u64>,
}
```

Rules:

- Field-level `#[revalidate(N)]` wins.
- Otherwise `[fsr] revalidate_seconds` wins.
- Otherwise the fallback is 86400 seconds.
- Same route plus same interval shares one timer.
- `#[revalidate(N)]` is not valid on inline `Live` fields.

## Debounce

`#[debounce(N)]` is valid on the inline `Live` struct or fields.

```rust
#[debounce(30)]
pub struct Live {
    pub status: LiveProp<String>,

    #[debounce(5)]
    pub priority: LiveProp<String>,
}
```

Current caveat: Pilcrow parses and stores `debounce_secs`, but the embedded watcher does not yet delay or coalesce stale-slot patches. Keep examples honest about this until runtime enforcement lands.

## Slot Validation

Routekit validates the template against inline `Live`.

Valid:

```rust
pub struct Live {
    pub status: LiveProp<String>,
}
```

```html
<span>{{ live.status.value }}</span>
```

Routekit rewrites that text-node interpolation into an `s-live="status"` slot.

Invalid:

- Explicit `s-live="ticket_status"` when the field is named `status`.
- A `LiveProp<T>` field with no text-node use, no matching explicit `s-live` slot, and no `#[pilcrow::allow_unused]`.
- Duplicate `s-live="status"` slots.

Use `#[pilcrow::allow_unused]` for fields that are intentionally not direct text slots.

## Promotion

FSR route promotion is controlled in page code-behind:

```rust
pub const PROMOTE_AFTER: u32 = 50;
```

`0` means bake on first hit.

## Redis Mode

With `live-props-redis`, Redis is the hot path:

- `pilcrow:html:<route>`
- `pilcrow:slot:<route>:<name>`
- `pilcrow:json:<route>`

`[fsr].redis_url` is required when Redis mode is enabled. Without Redis, the watcher uses polling fallback.
