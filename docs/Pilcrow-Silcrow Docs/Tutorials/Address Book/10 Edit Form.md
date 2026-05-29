# 10 — Edit Form

Add a real edit page that loads the contact's current values and saves changes.

## `pages/(app)/contacts/[contact_id]/edit/index.rs`

```rust
use pilcrow_web::AppError;
use pilcrow_web::live::*;

pub struct Props {
    pub id: String,
    pub first: String,
    pub last: String,
    pub twitter: String,
    pub avatar: String,
    pub notes: String,
    pub live: Live,
}

#[pilcrow::depends_on_route(contacts, contact_id)]
pub struct Live {
    pub updated_label: LiveProp<String>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("contact_id").and_then(|v| v.as_str()).unwrap_or("");
        live_query!(
            "SELECT 'Updated ' || to_char(updated_at AT TIME ZONE 'UTC',
               'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label
             FROM contacts WHERE id = $1",
            id
        )
    }
}

pub async fn load(req: Req, live: Live) -> AppResult<Props> {
    let id = req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid id".into()))?;
    let contact = crate::data::get(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    Ok(Props {
        id,
        first: contact.first,
        last: contact.last,
        twitter: contact.twitter,
        avatar: contact.avatar,
        notes: contact.notes,
        live,
    })
}

pub async fn save(req: Req) -> ActionResult {
    let id = req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid id".into()))?;

    let update = crate::data::ContactUpdate {
        first:   req.form.get("first").unwrap_or("").to_owned(),
        last:    req.form.get("last").unwrap_or("").to_owned(),
        twitter: req.form.get("twitter").unwrap_or("").to_owned(),
        avatar:  req.form.get("avatar").unwrap_or("").to_owned(),
        notes:   req.form.get("notes").unwrap_or("").to_owned(),
    };

    crate::data::update(&id, update)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    // Invalidate live slots — both the detail page and the edit page show updated_label
    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;

    redirect(format!("/contacts/{id}"))
}
```

`req.form.get("field")` reads URL-encoded form fields. Form values are always strings — trim and validate as needed.

The `save` action invalidates three routes:
- `/contacts/{id}` — the detail page shows `updated_label` and `favorite_mark`
- `/contacts/{id}/edit` — this page also shows `updated_label`
- `/` — `total_contacts` is unaffected by a save, but this covers edge cases if the count changes

Add `update` to `src/data.rs`:

```rust
pub async fn update(id: &str, update: ContactUpdate) -> AppResult<Option<Contact>> {
    sqlx::query(
        "UPDATE contacts
         SET first = $1, last = $2, twitter = $3, avatar = $4, notes = $5, updated_at = now()
         WHERE id = $6",
    )
    .bind(update.first.trim())
    .bind(update.last.trim())
    .bind(update.twitter.trim().trim_start_matches('@'))
    .bind(if update.avatar.trim().is_empty() {
        "https://sessionize.com/image/124e-400o400o2-wHVdAuNaxi8KJrgtN3ZKci.jpg"
    } else {
        update.avatar.trim()
    })
    .bind(update.notes.trim())
    .bind(id)
    .execute(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)?;

    crate::data::get(id).await
}
```

## `pages/(app)/contacts/[contact_id]/edit/index.html`

```html
<pilcrow:head>
  <title>Edit Contact - Pilcrow Contacts</title>
</pilcrow:head>

<form class="mx-auto grid max-w-2xl gap-5 rounded-lg border border-slate-200 bg-white p-6 shadow-sm"
      method="post" action="?/save" s-post="?/save">
  <div>
    <span class="mb-2 block text-xs font-bold uppercase tracking-wide text-slate-500">Name</span>
    <div class="grid gap-3 sm:grid-cols-2">
      <input class="w-full rounded-md border px-3 py-2 text-sm"
             name="first" placeholder="First" value="{{ first }}" />
      <input class="w-full rounded-md border px-3 py-2 text-sm"
             name="last"  placeholder="Last"  value="{{ last }}"  />
    </div>
  </div>
  <label class="grid gap-2">
    <span class="text-xs font-bold uppercase tracking-wide text-slate-500">Twitter</span>
    <input class="w-full rounded-md border px-3 py-2 text-sm"
           name="twitter" placeholder="@jack" value="{{ twitter }}" />
  </label>
  <label class="grid gap-2">
    <span class="text-xs font-bold uppercase tracking-wide text-slate-500">Avatar URL</span>
    <input class="w-full rounded-md border px-3 py-2 text-sm"
           name="avatar" placeholder="https://example.com/avatar.jpg" value="{{ avatar }}" />
  </label>
  <label class="grid gap-2">
    <span class="text-xs font-bold uppercase tracking-wide text-slate-500">Notes</span>
    <textarea class="w-full rounded-md border px-3 py-2 text-sm"
              name="notes" rows="6">{{ notes }}</textarea>
  </label>
  <div class="flex flex-wrap items-center gap-3">
    <button class="rounded-md bg-blue-700 px-4 py-2 text-sm font-semibold text-white"
            type="submit">Save</button>
    <button class="rounded-md border border-slate-300 px-4 py-2 text-sm"
            type="button" onclick="history.back()">Cancel</button>
    <span class="ml-auto text-xs text-slate-400">
      <span s-live="updated_label">{{ live.updated_label.value }}</span>
    </span>
  </div>
</form>
```

The `updated_label` slot in the footer updates live if another browser tab saves this contact while you are editing.

## Verify

Click Edit on a contact, change the name, and click Save. The detail page should show the updated name. The timestamp in the edit form footer should update in any other tab showing the same contact.

---

Next: [[11 Favourite Toggle]]
