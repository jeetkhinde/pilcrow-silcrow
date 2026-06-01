# Live List Items

Pilcrow lets you render a list where some columns are **static** (baked into HTML at first render, never touched again) and other columns are **live** (patched over SSE when the row changes). This is separate from the scalar `LiveProp<T>` / FSR system — lists have their own broadcast channel and SSE event type.

## `#[derive(PilcrowListRow)]`

Mark a struct with this derive to make it usable as a list row:

```rust
use pilcrow_web::prelude::*;

#[derive(PilcrowListRow, serde::Serialize)]
pub struct TicketRow {
    #[pilcrow(key)]
    pub id: i64,           // unique row identity

    #[pilcrow(live)]
    pub status: String,    // patched over SSE

    #[pilcrow(live)]
    pub priority: String,  // patched over SSE

    pub title: String,     // static — baked into HTML, never re-sent
    pub created_at: String, // static
}
```

### Field annotations

| Annotation | Role | Constraints |
|-----------|------|-------------|
| `#[pilcrow(key)]` | Row identity — sent as `key` in every patch | **Exactly one** per struct; compile error otherwise |
| `#[pilcrow(live)]` | Live field — serialised and sent via `list-patch` SSE | Zero or more |
| *(none)* | Static field — rendered in initial HTML, never patched | Any remaining fields |

The derive generates two methods:
- `pilcrow_key() -> String` — calls `.to_string()` on the key field
- `pilcrow_live_fields() -> Vec<(&'static str, serde_json::Value)>` — serialises each `#[pilcrow(live)]` field via `serde_json::to_value`

## Using `Vec<T>` in Props

No special container type — use a plain `Vec<T>` in your `Props` struct:

```rust
#[derive(Debug, Default)]
pub struct Props {
    pub tickets: Vec<TicketRow>,
}

pub async fn load(req: Req) -> Props {
    let rows = req
        .db()
        .query("SELECT id, status, priority, title, created_at FROM tickets ORDER BY id", &[])
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| TicketRow {
            id: r.get(0),
            status: r.get(1),
            priority: r.get(2),
            title: r.get(3),
            created_at: r.get(4),
        })
        .collect();

    Props { tickets: rows }
}
```

## Template wiring

Three `data-pilcrow-*` attributes connect the list to the patch system:

| Attribute | Where | Value |
|-----------|-------|-------|
| `data-pilcrow-list` | Container element | List name (matches `send_row` call) |
| `data-pilcrow-key` | Each row element | The key field value |
| `data-pilcrow-live-field` | Each live cell | Field name |

```html
<ul data-pilcrow-list="tickets">
  {% for ticket in props.tickets %}
  <li data-pilcrow-key="{{ ticket.id }}">
    <span>{{ ticket.title }}</span>             <!-- static, no attribute needed -->
    <span>{{ ticket.created_at }}</span>        <!-- static -->
    <span data-pilcrow-live-field="status">{{ ticket.status }}</span>
    <span data-pilcrow-live-field="priority">{{ ticket.priority }}</span>
  </li>
  {% endfor %}
</ul>
```

## Broadcasting row updates

Call `ListBroadcast::send_row` from any handler that mutates a row:

```rust
use pilcrow_web::prelude::*;

pub async fn update_ticket_status(req: Req) -> impl IntoResponse {
    let id: i64 = req.param("id").unwrap().parse().unwrap();
    let new_status: String = req.json_body().await.unwrap();

    req.db()
        .execute("UPDATE tickets SET status = $1 WHERE id = $2", &[&new_status, &id])
        .await
        .unwrap();

    let updated_row = TicketRow {
        id,
        status: new_status,
        ..Default::default() // static fields not needed for the patch
    };

    ListBroadcast::send_row("tickets", &updated_row);

    StatusCode::OK
}
```

`send_row` fans out the row's live fields to **all** clients that have the page containing `data-pilcrow-list="tickets"` open.

## SSE wire format

Each row update arrives as:

```
event: list-patch
data: {"list":"tickets","key":"42","status":"Closed","priority":"High"}
```

- `list` — matches `data-pilcrow-list` on the container
- `key` — identifies which `<li data-pilcrow-key="42">` to update
- remaining keys — `data-pilcrow-live-field` cells to patch

The client finds the matching row by key and updates each live cell's `textContent`. Static cells are never touched.

## What happens when a subscriber lags

If the broadcast ring-buffer overflows (`RecvError::Lagged`), the hub skips the missed events with `continue`. The affected client's list will be stale until the user refreshes — there is no automatic resync for lists (unlike scalar FSR fields which have `fsr-resync`). Design accordingly: list live fields should be low-frequency updates.

## What NOT to do

```rust
// WRONG — Vec is not a valid LiveProp type
pub struct Live {
    pub tickets: LiveProp<Vec<TicketRow>>, // compile error
}

// WRONG — no LiveList<T> type exists in Pilcrow
pub struct Props {
    pub tickets: LiveList<TicketRow>, // does not exist
}
```

Lists and FSR are separate systems. `LiveProp<T>` is for scalar page-level fields. `PilcrowListRow` + `ListBroadcast` is for row-level patching inside a collection.

## Relevant source files

| File | What it contains |
|------|-----------------|
| `crates/macros/src/list_row_derive.rs` | `PilcrowListRow` macro expansion |
| `crates/runtime/src/live_props/list_row.rs` | `ListRow` trait definition |
| `crates/runtime/src/live_props/list_broadcast.rs` | `ListBroadcast::send_row` implementation |
| `crates/web/src/lib.rs` | `PilcrowListRow` re-export |
| `pilcrow/registry.toml` | `"PilcrowListRow Derive"` and `"Vec<T: ListRow>"` feature entries |
