# Lists and Live Patching

Pilcrow's canonical list DX is plain Rust collections plus row metadata. A list is still `Vec<T>` in `Props`; the row type describes which field identifies the row and which fields can be patched later.

## Canonical Shape

Use:

- `Vec<T>` where `T: ListRow`
- `#[derive(PilcrowListRow)]`
- `ListBroadcast`
- `data-pilcrow-list`
- `data-pilcrow-key`
- Silcrow bindings such as `:text` or `s-use` inside each row

Do not introduce container types such as `LiveList<T>` or `AppendList<T>`. Do not model a whole list as `LiveProp<Vec<T>>`.

## Row Derive

```rust
use pilcrow_web::PilcrowListRow;

#[derive(PilcrowListRow)]
pub struct TicketRow {
    #[pilcrow(key)]
    pub id: i64,

    #[pilcrow(live)]
    pub status: String,

    #[pilcrow(live)]
    pub priority: String,

    pub title: String,
}
```

Rules:

- Exactly one field must have `#[pilcrow(key)]`.
- The key field is converted with `to_string()`.
- Any number of fields can have `#[pilcrow(live)]`.
- Live fields are serialized into list patch events.
- Unannotated fields are static HTML from the initial render.

The derive implements `pilcrow_web::live::ListRow`:

```rust
fn pilcrow_key(&self) -> String;
fn pilcrow_live_fields(&self) -> Vec<(&'static str, serde_json::Value)>;
```

## Page and Template

Use a normal list field in `Props`.

```rust
pub struct Props {
    pub tickets: Vec<TicketRow>,
}
```

Render a keyed container. Inside each row, bind live cells to fields that can arrive in a `list-patch`.

```html
<tbody data-pilcrow-list="tickets">
  {% for ticket in tickets %}
  <tr data-pilcrow-key="{{ ticket.id }}">
    <td>{{ ticket.title }}</td>
    <td><span :text="status">{{ ticket.status }}</span></td>
    <td><span :text="priority">{{ ticket.priority }}</span></td>
  </tr>
  {% endfor %}
</tbody>
```

`title` is static because it is not marked `#[pilcrow(live)]`. It is rendered in the initial HTML and is not included in row patches.

## Broadcast Producer

`ListBroadcast` is clone-safe. Register one shared instance as app state, then call `send_row<T: ListRow>` after a row mutation.

```rust
use pilcrow_web::axum::extract::Extension;
use pilcrow_web::{ListBroadcast, axum};

async fn update_ticket(
    Extension(broadcasts): Extension<ListBroadcast>,
) -> axum::response::Json<serde_json::Value> {
    let updated: TicketRow = save_ticket_change().await;

    broadcasts.send_row("tickets", &updated);
    axum::response::Json(serde_json::json!({ "ok": true }))
}
```

`send_row("tickets", &updated)` extracts:

- `list_name`: `"tickets"`
- `key`: `updated.pilcrow_key()`
- `fields`: only the `#[pilcrow(live)]` fields

## SSE Relay

`ListBroadcast` is not the FSR `/__pilcrow/fsr` hub. It is a developer-owned broadcast channel. You relay it from an SSE route with `SilcrowEvent::list_patch`.

```rust
use pilcrow_web::axum;
use pilcrow_web::axum::extract::Extension;
use pilcrow_web::{ListBroadcast, SilcrowEvent, sse_stream};
use tokio::sync::broadcast;

async fn live_tickets(
    Extension(broadcasts): Extension<ListBroadcast>,
) -> impl axum::response::IntoResponse {
    sse_stream(move |emitter| async move {
        let mut rx = broadcasts.subscribe();

        loop {
            match rx.recv().await {
                Ok(ev) => {
                    emitter
                        .send(SilcrowEvent::list_patch(ev.list_name, ev.key, ev.fields))
                        .await?;
                }
                Err(broadcast::error::RecvError::Closed) => break Ok(()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    })
}
```

Attach that SSE route to the list container:

```html
<tbody data-pilcrow-list="tickets" s-sse="/api/tickets/live">
  ...
</tbody>
```

## Wire Format

The browser receives:

```text
event: list-patch
data: {"list":"tickets","key":"123","status":"closed","priority":"critical"}
```

Silcrow finds:

```html
<tbody data-pilcrow-list="tickets">
  <tr data-pilcrow-key="123"></tr>
</tbody>
```

Then it patches that row with the remaining fields, after removing `list` and `key` from the payload.

## FSR Boundary

List patching and FSR live slots are separate systems.

- FSR uses inline `Live`, `LiveProp<T>`, `s-live`, and `/__pilcrow/fsr`.
- Lists use `PilcrowListRow`, `ListBroadcast`, `data-pilcrow-list`, `data-pilcrow-key`, and a developer SSE route.
- A list page can use both systems, but each field should belong to one patching model.

## Per-Row Cache

`ListChunkCache` stores pre-baked HTML for individual rows keyed by `(list_name, row_key)`.

Invalidate the chunk after every `ListBroadcast::send_row` call to avoid stale row HTML.
