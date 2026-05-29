# FSR Ownership and Invalidation

Quick reference for what the app owns versus what the framework owns in an FSR route.

## What the App Owns

| Surface | Where | Notes |
|---|---|---|
| `s-live="slot_name"` | HTML template | DOM element the watcher patches via `textContent` |
| `req.fsr.invalidate_route(path)` | Action handler | Marks all slot rows for `path` stale; watcher re-runs their SQL |
| `req.fsr.tombstone(path)` | Action handler | Marks route dead; clears Redis + disk; next request returns 404 |
| `req.fsr.invalidate_dep_key(key)` | Action handler | Marks all slots whose `depends_on` contains `key` stale |
| `req.fsr.prebake_next(path)` | Action handler | Background GET to pre-warm the next pagination window |

That is the entire app-facing FSR write surface. Nothing else.

## What the Framework Owns

- Slot registration (`pilcrow_fsr` rows) — happens automatically in the `Live` extractor on every request
- Hit counting — `hit_count` incremented on every request
- Promotion — when `hit_count >= PROMOTE_AFTER`, route is marked `promoted = TRUE`
- Baking — watcher writes `html_path` / `json_path` artifacts for promoted routes
- Watcher re-execution — detects `stale = TRUE`, re-runs stored SQL, patches Redis + disk
- SSE push — `SlotPatch` events broadcast to connected clients
- Redis caching — `pilcrow:html:<route>`, `pilcrow:slot:<route>`, `pilcrow:json:<route>`

## Anti-Patterns

These are wrong and should never appear in app code:

```rust
// ❌ Never write baked files from load()
tokio::fs::write(".pilcrow-baked/pages/foo.html", html).await?;

// ❌ Never touch pilcrow_fsr directly from load()
sqlx::query("UPDATE pilcrow_fsr SET promoted = TRUE WHERE route = $1")
    .bind(route)
    .execute(pool)
    .await?;

// ❌ Never call bake functions from load()
crate::bake::bake_contact_pane(&contact).await?;
```

## Invalidation Patterns

### After a mutation that changes live data

```rust
pub async fn save(req: Req) -> ActionResult {
    let id = req.params.get("id").unwrap();
    db::update_contact(id, &req.form).await?;

    // Invalidate all live slots for the detail and edit routes
    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;

    redirect(format!("/contacts/{id}"))
}
```

### After a delete

```rust
pub async fn destroy(req: Req) -> ActionResult {
    let id = req.params.get("id").unwrap();
    db::delete_contact(id).await?;

    // Tombstone marks the route dead and clears Redis + disk artifacts
    req.fsr.tombstone(&format!("/contacts/{id}")).await;

    redirect("/")
}
```

### Using dep keys (optional; `invalidate_route` is simpler)

```rust
// In a background job or bulk importer that updates many contacts:
req.fsr.invalidate_dep_key("contacts:id=alex-anderson").await;
```

## `PROMOTE_AFTER` Scope Rule

Only use `PROMOTE_AFTER` on routes with a **finite, known URL instance set**:

```rust
// ✅ Static route — one URL instance
pub const PROMOTE_AFTER: u32 = 0;  // bake on first hit

// ✅ Near-static route with a bounded set
pub const PROMOTE_AFTER: u32 = 10;
```

Do not use `PROMOTE_AFTER` on dynamic routes:

```rust
// ❌ Dynamic route — open ID space
// Every /contacts/<id> URL gets its own promoted=TRUE row
// but html_path stays NULL (watcher logs warnings)
pub const PROMOTE_AFTER: u32 = 0;  // wrong on [contact_id] routes
```

Dynamic routes work correctly as pure SSR + live patching with no `PROMOTE_AFTER`.

## `s-live` Slot Rules

- Each `s-live` value must match a `LiveProp<T>` field name in inline `Live`
- Each `LiveProp<T>` field must have either a text-node use `{{ live.field.value }}` (routekit auto-inserts `s-live`) or an explicit `s-live` slot, unless marked `#[pilcrow::allow_unused]`
- Duplicate `s-live` slot names in a single template are build errors

## Invalidation Scope

`invalidate_route(path)` marks ALL slot rows for that exact path stale. Use the exact URL path including any param values:

```rust
req.fsr.invalidate_route("/contacts/kent-c-dodds").await;      // ✅
req.fsr.invalidate_route("/contacts/[contact_id]").await;       // ❌ pattern, not a real path
req.fsr.invalidate_route(&format!("/contacts/{id}")).await;     // ✅
```

## Related

- [[../03 Rendering/Build an FSR Page]]
- [[../03 Rendering/FSR SSE Hub]]
- [[../03 Rendering/Live Props and FSR]]
