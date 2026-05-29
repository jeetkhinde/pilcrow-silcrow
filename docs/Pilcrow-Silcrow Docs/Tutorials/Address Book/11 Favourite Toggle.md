# 11 — Favourite Toggle

Add the star button that toggles a contact's favourite status.

## `favorite` action in the contact detail code-behind

```rust
pub async fn favorite(req: Req) -> ActionResult {
    let id = req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid id".into()))?;

    let fav = req.form.get("favorite").unwrap_or("false") == "true";

    crate::data::set_favorite(&id, fav)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;

    let path = format!("/contacts/{id}");
    redirect(&path).retarget("#detail").push_history(&path)
}
```

Add `set_favorite` to `src/data.rs`:

```rust
pub async fn set_favorite(id: &str, favorite: bool) -> AppResult<Option<Contact>> {
    sqlx::query("UPDATE contacts SET favorite = $1, updated_at = now() WHERE id = $2")
        .bind(favorite)
        .bind(id)
        .execute(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)?;

    get(id).await
}
```

`set_favorite` also sets `updated_at = now()`, which is why the `save` and `favorite` actions both invalidate the edit route's `updated_label` slot.

## Update the template

Replace the plain `<span>` from step 5 with the star button form:

```html
<h1 class="flex flex-wrap items-center gap-3 text-4xl font-bold">
  {% if first.len() > 0 || last.len() > 0 %}{{ first }} {{ last }}{% else %}<i>No Name</i>{% endif %}

  <form method="post" action="?/favorite" s-post="?/favorite"
        onsubmit="window.__optimisticStar && window.__optimisticStar(this)">
    <button
      class="rounded-md px-2 text-3xl leading-none text-amber-500"
      aria-label="{% if favorite %}Remove from favourites{% else %}Add to favourites{% endif %}"
      name="favorite"
      value="{% if favorite %}false{% else %}true{% endif %}"
      type="submit">
      <span id="favorite-slot" s-live="favorite_mark">{{ live.favorite_mark.value }}</span>
    </button>
  </form>
</h1>
```

The button `value` alternates between `"true"` and `"false"`, so the `?/favorite` action always receives the correct next state.

`s-live="favorite_mark"` is the FSR slot. When `invalidate_route` marks it stale, the watcher re-runs the SQL and pushes an SSE patch. Any open tab on this contact sees the star update without a reload.

## Optimistic UI

The star updates via SSE after the round-trip. To make it feel instant, an `onsubmit` handler flips the star immediately:

```html
<script>
  window.__optimisticStar = function (form) {
    var slot = document.getElementById('favorite-slot');
    var btn  = form.querySelector('button[name="favorite"]');
    if (!slot || !btn) return true;
    slot.textContent = btn.value === 'true' ? '*' : '☆';
    return true;
  };
</script>
```

This updates `textContent` before the request fires. The SSE patch from the watcher confirms the value shortly after — if they match (they always do), the user sees no flicker.

## s-live on `updated_label`

Add `s-live="updated_label"` to the timestamp in the detail template so it also patches via SSE:

```html
<p class="mt-4 text-sm text-slate-500">
  <span s-live="updated_label">{{ live.updated_label.value }}</span>
</p>
```

## Verify

Click the star. It should flip immediately (optimistic), then the sidebar should show or hide the ★ on the contact's name after the action completes.

---

Next: [[12 Deleting Contacts]]
