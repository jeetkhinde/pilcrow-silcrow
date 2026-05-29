# 05 — Contact Detail

Add the contact detail page. It uses a URL param to load one contact from the database.

## File layout

```text
pages/(app)/contacts/[contact_id]/
  index.html
  index.rs
```

`[contact_id]` in the directory name becomes `:contact_id` in the Axum route pattern. The value is available as `req.params.get("contact_id")`.

## `pages/(app)/contacts/[contact_id]/index.rs`

```rust
use pilcrow_web::AppError;
use pilcrow_web::live::*;

pub struct Props {
    pub id: String,
    pub name: String,
    pub first: String,
    pub last: String,
    pub avatar: String,
    pub twitter: String,
    pub notes: String,
    pub favorite: bool,
    pub live: Live,
}

// No PROMOTE_AFTER — this is a dynamic route with an open ID-space.
// Every /contacts/<id> URL is valid. Using PROMOTE_AFTER = 0 would mark
// every contact as promoted but leave html_path = NULL, causing watcher
// warnings on every invalidation. Omit it.

#[pilcrow::depends_on_route(contacts, contact_id)]
pub struct Live {
    pub favorite_mark: LiveProp<String>,
    pub updated_label: LiveProp<String>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("contact_id").and_then(|v| v.as_str()).unwrap_or("");
        live_query!(
            "SELECT
               CASE WHEN favorite THEN '*' ELSE '☆' END AS favorite_mark,
               'Updated ' || to_char(updated_at AT TIME ZONE 'UTC',
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
        name: crate::data::display_name(&contact),
        id,
        first: contact.first,
        last: contact.last,
        avatar: contact.avatar,
        twitter: contact.twitter,
        notes: contact.notes,
        favorite: contact.favorite,
        live,
    })
}
```

**`#[pilcrow::depends_on_route(contacts, contact_id)]`** — a struct-level attribute that applies to all `LiveProp<T>` fields in `Live`. It generates the dep key `contacts:contact_id=<value>` for each request, wiring the slot to the specific contact row so targeted invalidation works.

## `pages/(app)/contacts/[contact_id]/index.html`

```html
<pilcrow:head>
  <title>{{ name }} - Pilcrow Contacts</title>
</pilcrow:head>

<article class="mx-auto grid max-w-4xl gap-8 md:grid-cols-[13rem_1fr]">
  <div>
    <img class="h-52 w-52 rounded-lg border border-slate-200 object-cover shadow-sm"
         alt="{{ name }} avatar" src="{{ avatar }}" />
  </div>
  <div class="min-w-0">
    <h1 class="flex flex-wrap items-center gap-3 text-4xl font-bold">
      {% if first.len() > 0 || last.len() > 0 %}
        {{ first }} {{ last }}
      {% else %}
        <i>No Name</i>
      {% endif %}
      <span>{{ live.favorite_mark.value }}</span>
    </h1>

    {% if twitter.len() > 0 %}
    <p class="mt-2">
      <a class="font-medium text-blue-700" href="https://twitter.com/{{ twitter }}">
        @{{ twitter }}
      </a>
    </p>
    {% endif %}

    {% if notes.len() > 0 %}
    <p class="mt-6 whitespace-pre-line leading-7 text-slate-700">{{ notes }}</p>
    {% else %}
    <p class="mt-6 italic text-slate-500">No notes yet.</p>
    {% endif %}

    <p class="mt-4 text-sm text-slate-500">{{ live.updated_label.value }}</p>

    <div class="mt-8 flex gap-3">
      <a href="/contacts/{{ id }}/edit" class="rounded-md border border-slate-300 px-4 py-2 text-sm">
        Edit
      </a>
    </div>
  </div>
</article>
```

`{{ live.favorite_mark.value }}` and `{{ live.updated_label.value }}` render the initial SSR values. In the next steps you will add `s-live` attributes so they update via SSE — for now this is enough to verify the page loads.

## Add seed data

Add a seed function to `src/data.rs` so the sidebar has contacts to show:

```rust
pub async fn seed_if_empty(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM contacts")
        .fetch_one(pool)
        .await?;
    if count > 0 { return Ok(()); }

    for contact in SEED_CONTACTS {
        sqlx::query(
            "INSERT INTO contacts (id, first, last, avatar, twitter)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(contact.id)
        .bind(contact.first)
        .bind(contact.last)
        .bind(contact.avatar)
        .bind(contact.twitter.unwrap_or(""))
        .execute(pool)
        .await?;
    }
    Ok(())
}
```

The full seed list is in `src/data.rs` in the `address-book/` source tree.

## Verify

Restart the server. The sidebar should show the seeded contacts. Click one — the detail pane should render the contact's name, avatar, and placeholder fields.

---

Next: [[06 Creating Contacts]]
