# Lists and Live Patching

Pilcrow's canonical list DX is plain Rust collections plus row metadata.

## Canonical Shape

Use:

- `Vec<T>` where `T: ListRow`
- `#[derive(PilcrowListRow)]`
- `ListBroadcast`
- `data-pilcrow-list`
- `data-pilcrow-key`

Do not introduce container types such as `LiveList<T>` or `AppendList<T>`.

## Row Derive

```rust
#[derive(PilcrowListRow)]
struct Ticket {
    #[pilcrow(key)]
    id: i64,

    #[pilcrow(live)]
    status: String,

    title: String,
}
```

Rules:

- Exactly one field should identify the row key.
- Live fields must implement `serde::Serialize`.

## Wire Format

Server event:

```json
{
  "list": "tickets",
  "key": "123",
  "status": "closed"
}
```

Silcrow finds:

```html
<div data-pilcrow-list="tickets">
  <article data-pilcrow-key="123"></article>
</div>
```

Then it patches the row with changed fields.

## Broadcast Producer

`ListBroadcast` is clone-safe. `send_row<T: ListRow>` extracts the row key and live fields, then fans out a list patch event to SSE subscribers.

## Per-Row Cache

`ListChunkCache` stores pre-baked HTML for individual rows keyed by `(list_name, row_key)`.

Invalidate the chunk after every `ListBroadcast::send_row` call to avoid stale row HTML.
