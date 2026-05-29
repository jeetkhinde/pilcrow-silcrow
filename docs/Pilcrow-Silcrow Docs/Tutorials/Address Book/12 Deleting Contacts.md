# 12 — Deleting Contacts

Add the Delete button and wire up the `destroy` action.

## `destroy` action

```rust
pub async fn destroy(req: Req) -> ActionResult {
    let id = req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid id".into()))?;

    crate::data::delete(&id).await?;

    // tombstone marks the route dead and clears Redis + disk artifacts.
    // The next request to /contacts/{id} returns 404.
    req.fsr.tombstone(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route("/").await;

    redirect("/").retarget("#detail").push_history("/")
}
```

Add `delete` to `src/data.rs`:

```rust
pub async fn delete(id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM contacts WHERE id = $1")
        .bind(id)
        .execute(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)?;
    Ok(result.rows_affected() > 0)
}
```

## `tombstone` vs `invalidate_route`

| | `invalidate_route` | `tombstone` |
|---|---|---|
| Marks slots stale | ✓ | ✓ |
| Clears Redis keys | ✗ | ✓ |
| Removes baked disk files | ✗ | ✓ |
| Next request returns 404 | ✗ | ✓ |

Use `tombstone` when the entity itself is gone. Use `invalidate_route` when the entity still exists but its live data has changed.

## Update the template

Add the Delete form to the contact detail page:

```html
<div class="mt-8 flex flex-wrap gap-3">
  <form method="get" action="/contacts/{{ id }}/edit">
    <button class="rounded-md border border-slate-300 bg-white px-4 py-2 text-sm"
            type="submit">Edit</button>
  </form>
  <form method="post" action="?/destroy" s-post="?/destroy"
        onsubmit="return confirm('Delete this contact?')">
    <button class="rounded-md border border-red-200 bg-white px-4 py-2 text-sm text-red-700"
            type="submit">Delete</button>
  </form>
</div>
```

The `onsubmit="return confirm(...)"` shows a native browser confirmation dialog before posting. `s-post="?/destroy"` posts via `fetch()` so Silcrow can apply the redirect response without a full page reload.

## Verify

Open a contact, click Delete, confirm. The right pane should navigate back to the index page and the contact should disappear from the sidebar. The contact count in the index should decrement.

---

Next: [[13 FSR Live Fields]]
