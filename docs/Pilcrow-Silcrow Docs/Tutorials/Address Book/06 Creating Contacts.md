# 06 — Creating Contacts

Wire up the New button so it creates an empty contact and navigates to its edit page.

## Named actions

A **named action** is a public async function in a page's code-behind that handles a POST. The form targets it with `action="?/function_name"`. Pilcrow dispatches the POST to the matching function — no router registration needed.

## Add the `create` action to `pages/(app)/index.rs`

```rust
pub async fn create(req: Req) -> ActionResult {
    let contact = crate::data::create_empty().await?;
    req.fsr.invalidate_route("/").await;
    let path = format!("/contacts/{}/edit", contact.id);
    redirect(&path).retarget("#detail").push_history(&path)
}
```

- `create_empty()` inserts a blank contact row with a random ID and returns it.
- `req.fsr.invalidate_route("/")` marks the `total_contacts` slot stale. The watcher re-runs `COUNT(*)` and pushes an SSE patch to any open tabs showing the index — the counter updates live.
- `redirect(&path)` sets the 303 response location.
- `.retarget("#detail")` tells Silcrow to load the redirect target into the `#detail` element instead of navigating the whole page.
- `.push_history(&path)` updates the browser URL bar to the new contact's edit path.

## Add `create_empty` to `src/data.rs`

```rust
pub async fn create_empty() -> AppResult<Contact> {
    let id = random_id();
    sqlx::query(
        "INSERT INTO contacts (id, first, last, avatar, twitter, notes, favorite)
         VALUES ($1, '', '', 'https://sessionize.com/image/124e-400o400o2-wHVdAuNaxi8KJrgtN3ZKci.jpg', '', '', FALSE)",
    )
    .bind(&id)
    .execute(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)?;

    get(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("new contact not found".into()))
}

fn random_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("contact-{nanos:x}")
}
```

## Update the layout form to use Silcrow

The New button form in `(app)/_layout.html` already has `s-post="/?/create"`. Silcrow intercepts the submit, posts to `/?/create`, and applies the redirect response using the `silcrow-retarget` response header set by `.retarget("#detail")`.

```html
<form method="post" action="/?/create" s-post="/?/create">
  <button type="submit">New</button>
</form>
```

The `s-post` attribute tells Silcrow to POST via `fetch()` instead of a native form submit. If Silcrow hasn't loaded yet, the form falls back to a normal POST — the action still works, just without client-side navigation.

## Add the edit page (stub)

Create `pages/(app)/contacts/[contact_id]/edit/index.html` as a stub so the redirect target resolves:

```html
<pilcrow:head>
  <title>Edit Contact</title>
</pilcrow:head>

<p>Edit form coming in step 10.</p>
```

## Verify

Click New. The URL should change to `/contacts/<id>/edit` and the right pane should show the stub. The sidebar contact count should increment in real time if you have the index page open in another tab.

---

Next: [[07 Client-Side Navigation]]
