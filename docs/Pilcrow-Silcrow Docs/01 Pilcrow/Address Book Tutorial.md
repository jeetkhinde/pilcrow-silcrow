# Address Book Tutorial

A complete walkthrough of the address-book demo — the [React Router address book tutorial](https://reactrouter.com/tutorials/address-book) rebuilt in Pilcrow + Silcrow. Covers file routing, layouts, loaders, named actions, FSR live fields, and Silcrow navigation.

Source: `address-book/` in the workspace root.

---

## What You Will Build

A two-column contact manager:

- Left sidebar: contact list with search and a New button
- Right pane: contact detail or edit form
- Live fields: favorite star and timestamp update in the browser without a reload
- Named actions: create, save, favorite, destroy

---

## 1. Project Setup

```toml
# address-book/Cargo.toml
[dependencies]
pilcrow-web = { path = "../pilcrow/crates/web", features = ["live-props"] }
pilcrow_runtime = { path = "../pilcrow/crates/runtime", features = ["live-props-redis"] }
sqlx = { version = "0.8", features = ["runtime-tokio-native-tls", "postgres", "json", "chrono"] }
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
pilcrow-routekit = { path = "../pilcrow/crates/routekit" }
```

```toml
# address-book/Pilcrow.toml
[web]
host = "127.0.0.1"
port = 3010

[fsr]
poll_interval_ms = 200
revalidate_seconds = 15
redis_url = "redis://127.0.0.1:6379"
```

```
# address-book/.env
DATABASE_URL=postgresql://user:password@localhost:5432/address_book
```

---

## 2. Hooks: Database and FSR Init

`hooks.rs` runs before the server accepts traffic. It creates tables and registers the FSR store.

```rust
// hooks.rs
pub async fn init() {
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("failed to connect to DATABASE_URL");

    create_contacts_table(&pool).await;
    create_fsr_table(&pool).await;
    crate::data::seed_if_empty(&pool).await.expect("seed failed");

    // Register FSR store — required before any FSR route handles a request
    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
}
```

The `pilcrow_fsr` table is created by the app (not by a migration runner) because the demo wants to be self-contained. In a real project you would use `sqlx migrate run`.

---

## 3. File Layout

```text
address-book/
  pages/
    (app)/
      _layout.html          ← sidebar + main pane
      _layout.rs            ← loads contact list for sidebar
      index.html            ← right pane: welcome / index stats
      index.rs              ← total_contacts LiveProp
      contacts/
        [contact_id]/
          index.html        ← contact detail
          index.rs          ← favorite_mark + updated_label LiveProps
          edit/
            index.html      ← edit form
            index.rs        ← updated_label LiveProp
  src/
    main.rs
    data.rs                 ← DB queries
    db.rs                   ← global pool
  hooks.rs
  Cargo.toml
  Pilcrow.toml
  .env
```

The `(app)` directory is a **route group** — it scopes the layout without adding a URL segment. All routes under `(app)/` share the sidebar layout.

---

## 4. Layout: Sidebar + Main Pane

```rust
// pages/(app)/_layout.rs
pub struct Props {
    pub contacts: Vec<ContactSummary>,
    pub q: String,
    pub searching: bool,
    pub sidebar_contact_count: i64,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let q = req.query.get("q").unwrap_or("").to_owned();
    let active_id = req.params.get("contact_id").map(String::as_str);
    let contacts = crate::data::list(Some(&q), active_id).await?;
    let count = crate::data::count().await?;

    Ok(Props {
        contacts,
        searching: !q.is_empty(),
        q,
        sidebar_contact_count: count,
    })
}
```

The layout renders the sidebar and a `<slot />` for the right pane. Contact links use `s-get` for Silcrow navigation without full page reloads. The search form uses `s-get="/" s-target="body"` to replace the whole page on search.

The sidebar's `{% if contact.active %}` active highlight is computed server-side from `active_id`. Since layout-aware (PS fragment) navigation only swaps the right pane, active highlight is also kept in sync client-side:

```html
<script>
  document.addEventListener('silcrow:load', function () {
    var path = window.location.pathname;
    document.querySelectorAll('#sidebar nav a[href]').forEach(function (a) {
      var active = a.getAttribute('href') === path;
      if (active) {
        a.classList.remove('text-slate-600', 'hover:bg-slate-100', 'hover:text-slate-950');
        a.classList.add('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.setAttribute('aria-current', 'page');
      } else if (a.classList.contains('bg-blue-50')) {
        a.classList.remove('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.classList.add('text-slate-600', 'hover:bg-slate-100', 'hover:text-slate-950');
        a.removeAttribute('aria-current');
      }
    });
  });
</script>
```

`silcrow:load` fires after the DOM swap and `history.pushState` — `window.location.pathname` is the new URL at that point.

---

## 5. Index Route: Live Contact Count

```rust
// pages/(app)/index.rs
use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub const PROMOTE_AFTER: u32 = 0;  // static route — bake on first hit

pub struct Live {
    pub total_contacts: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!("SELECT COUNT(*)::bigint AS total_contacts FROM contacts")
    }
}

pub async fn load(_req: Req, live: Live) -> AppResult<Props> {
    Ok(Props { live })
}

pub async fn create(req: Req) -> ActionResult {
    let contact = crate::data::create_empty().await?;
    req.fsr.invalidate_route("/").await;
    let path = format!("/contacts/{}/edit", contact.id);
    redirect(&path).retarget("#detail").push_history(&path)
}
```

`PROMOTE_AFTER: u32 = 0` is valid here because `/` is a static route — one URL instance. The framework bakes it on first hit; subsequent requests are served from Redis.

The `total_contacts` slot is registered in `pilcrow_fsr` with `depends_on = {page_index::__revalidate_default}`. The `create` and `destroy` actions call `req.fsr.invalidate_route("/")` to trigger a slot refresh.

Template (`index.html`):

```html
<span class="text-3xl font-bold" s-live="total_contacts">{{ live.total_contacts.value }}</span>
```

---

## 6. Contact Detail: Multiple Live Fields

```rust
// pages/(app)/contacts/[contact_id]/index.rs
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

// No PROMOTE_AFTER — dynamic route with open ID space

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
               'Updated ' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label
             FROM contacts WHERE id = $1",
            id
        )
    }
}

pub async fn load(req: Req, live: Live) -> AppResult<Props> {
    let id = parse_id(&req)?;
    let contact = crate::data::get(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    Ok(Props {
        id,
        name: crate::data::display_name(&contact),
        first: contact.first,
        last: contact.last,
        avatar: contact.avatar,
        twitter: contact.twitter,
        notes: contact.notes,
        favorite: contact.favorite,
        live,
    })
}

pub async fn favorite(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    let favorite = req.form.get("favorite").unwrap_or("false") == "true";
    crate::data::set_favorite(&id, favorite).await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;

    let path = format!("/contacts/{id}");
    redirect(&path).retarget("#detail").push_history(&path)
}

pub async fn destroy(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    crate::data::delete(&id).await?;
    req.fsr.tombstone(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route("/").await;
    redirect("/").retarget("#detail").push_history("/")
}
```

Key points:

- No `PROMOTE_AFTER` — dynamic routes work as pure SSR + live patching.
- `#[pilcrow::depends_on_route(contacts, contact_id)]` wires each slot's `depends_on` to `contacts:contact_id=<value>`, so targeted invalidation by dep key is also possible.
- `favorite` action invalidates both `/contacts/{id}` and `/contacts/{id}/edit` because `set_favorite` touches `updated_at`.
- `destroy` uses `tombstone()` — the route is marked dead and its Redis/disk artifacts are removed. The next request returns 404.

Template excerpt:

```html
<span id="favorite-slot" s-live="favorite_mark">{{ live.favorite_mark.value }}</span>

<span s-live="updated_label">{{ live.updated_label.value }}</span>
```

---

## 7. Edit Route: Live Timestamp

The edit form is a normal SSR form. It adds one live field — `updated_label` — so an editor can see if someone else saved the contact while they were editing.

```rust
// pages/(app)/contacts/[contact_id]/edit/index.rs
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
            "SELECT 'Updated ' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label FROM contacts WHERE id = $1",
            id
        )
    }
}

pub async fn load(req: Req, live: Live) -> AppResult<Props> {
    let id = parse_id(&req)?;
    let contact = crate::data::get(&id).await?
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
    let id = parse_id(&req)?;
    crate::data::update(&id, ContactUpdate { /* ... */ }).await?;

    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;

    redirect(format!("/contacts/{id}"))
}
```

Template (footer area):

```html
<div class="flex flex-wrap items-center gap-3">
  <button type="submit">Save</button>
  <button type="button" onclick="history.back()">Cancel</button>
  <span class="ml-auto text-xs text-slate-400">
    <span s-live="updated_label">{{ live.updated_label.value }}</span>
  </span>
</div>
```

---

## 8. FSR Shape Summary

| Route | Live slots | Invalidated by |
|---|---|---|
| `/` | `total_contacts` | `create`, `destroy` |
| `/contacts/[id]` | `favorite_mark`, `updated_label` | `favorite`, `save`, `destroy` |
| `/contacts/[id]/edit` | `updated_label` | `save`, `favorite` |

All three routes run `load()` on every request. The framework registers slots in `pilcrow_fsr`, the watcher re-runs stored SQL when slots go stale, and connected browsers receive SSE patches within the next poll cycle (200 ms in this demo).

---

## 9. Silcrow Navigation

Contact links use `s-get` for smooth navigation:

```html
<a href="/contacts/{{ contact.id }}" s-get="/contacts/{{ contact.id }}">
  {{ contact.name }}
</a>
```

Pilcrow uses layout-aware (PS fragment) navigation: Silcrow sends `X-PS-Present: /, /(app)` on navigation. The server sees the header and returns only the changed slot (`[data-ps-slot="/contacts/:contact_id"]`), not the full page. The sidebar stays intact; only the right pane swaps.

When navigating from the index page (which has no contact slot in the DOM), fragment application fails and Silcrow falls back to `window.location.assign()` — a full page reload. This is expected and correct.

Actions use `s-post`, `retarget("#detail")`, and `push_history()` to update only the right pane:

```rust
redirect(&path).retarget("#detail").push_history(&path)
```

---

## 10. Run It

```bash
# From workspace root
cargo run --manifest-path address-book/Cargo.toml
```

Open `http://127.0.0.1:3010`.

To reset FSR state (e.g. after changing slot definitions):

```sql
TRUNCATE pilcrow_fsr;
```

Rows are re-registered automatically on the next request to each route.

---

## Related

- [[../03 Rendering/Build an FSR Page]]
- [[../03 Rendering/Live Props and FSR]]
- [[../03 Rendering/FSR SSE Hub]]
- [[../05 Reference/FSR Ownership and Invalidation]]
- [[../01 Pilcrow/Pages and Layouts]]
- [[../01 Pilcrow/Actions and Forms]]
- [[../02 Silcrow/Navigation and Live Connections]]
