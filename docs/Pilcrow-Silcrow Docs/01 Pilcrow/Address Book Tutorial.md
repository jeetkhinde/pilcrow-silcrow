# Address Book Tutorial

Build a two-column contact manager in Pilcrow + Silcrow — the same app as the [React Router address book tutorial](https://reactrouter.com/tutorials/address-book), rebuilt with Rust server-side rendering, file routes, named actions, and FSR live fields.

**What you will build:** a full contact manager with a sidebar, detail pane, create/edit/delete, favourite toggling, live-updating counters and timestamps, and client-side navigation without full-page reloads.

The finished code lives in `address-book/` in the workspace root.

---

## 1. Project setup

Create the directory and workspace files.

### `Cargo.toml`

```toml
[package]
name = "pilcrow-address-book"
version = "0.1.0"
edition = "2021"

[dependencies]
pilcrow-web = { path = "../pilcrow/crates/web", features = ["live-props"] }
pilcrow_runtime = { path = "../pilcrow/crates/runtime", features = ["live-props-redis"] }
sqlx   = { version = "0.8", features = ["runtime-tokio-native-tls", "postgres", "json", "chrono"] }
tokio  = { version = "1",   features = ["full"] }
dotenvy = "0.15"
serde  = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
pilcrow-routekit = { path = "../pilcrow/crates/routekit" }
```

`live-props` enables FSR. `live-props-redis` enables Redis caching and pub/sub-driven invalidation.

### `build.rs`

```rust
fn main() {
    routekit::compile_current_crate_sources().expect("pilcrow compile");
}
```

### `Pilcrow.toml`

```toml
[web]
host = "127.0.0.1"
port = 3010

[fsr]
poll_interval_ms   = 200
revalidate_seconds = 15
redis_url          = "redis://127.0.0.1:6379"
```

### `.env`

```
DATABASE_URL=postgresql://user:password@localhost:5432/address_book
```

### `src/main.rs`

```rust
pilcrow_web::pilcrow_app!();

mod data;
mod db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pilcrow_start(pilcrow_router()).await
}
```

`pilcrow_app!()` expands to the generated router. `pilcrow_start()` wires the Axum server, FSR watcher, and middleware.

---

## 2. Database and global pool

### `src/db.rs`

```rust
use sqlx::PgPool;
use std::sync::OnceLock;

pub static POOL: OnceLock<PgPool> = OnceLock::new();

pub fn pool() -> &'static PgPool {
    POOL.get().expect("DB pool not initialized")
}
```

### `hooks.rs` (project root)

Pilcrow calls `hooks::init()` before accepting traffic. Create and seed the database here.

```rust
use pilcrow_web::{HookError, Next, Req, Response};

pub async fn handle(_req: Req, next: Next) -> Response {
    next.run().await
}

pub async fn handle_error(error: &HookError, _req: &Req) -> Option<Response> {
    pilcrow_web::tracing::error!(status = error.status, "server error: {}", error.message);
    None
}

pub async fn init() {
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("failed to connect to DATABASE_URL");

    // Create tables
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS contacts (
            id         TEXT PRIMARY KEY,
            first      TEXT NOT NULL DEFAULT '',
            last       TEXT NOT NULL DEFAULT '',
            avatar     TEXT NOT NULL DEFAULT '',
            twitter    TEXT NOT NULL DEFAULT '',
            notes      TEXT NOT NULL DEFAULT '',
            favorite   BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create contacts table");

    // FSR metadata table (slot tracking, hit counting, promotion)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pilcrow_fsr (
            route         TEXT    NOT NULL,
            slot          TEXT    NOT NULL DEFAULT '',
            /* ... full schema in hooks.rs ... */
            PRIMARY KEY (route, slot)
        )",
    )
    .execute(&pool)
    .await
    .ok();

    // Seed contacts if table is empty
    crate::data::seed_if_empty(&pool).await.expect("seed failed");

    // Register FSR store — required before any FSR route handles a request
    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
}
```

The full `pilcrow_fsr` schema is in `hooks.rs` in the source tree. Alternatively, use a migration runner and point it at `pilcrow/crates/runtime/migrations/`.

---

## 3. Data layer

### `src/data.rs` — types

```rust
#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct Contact {
    pub id: String,
    pub first: String,
    pub last: String,
    pub avatar: String,
    pub twitter: String,
    pub notes: String,
    pub favorite: bool,
    pub updated_label: String, // formatted updated_at from SQL
}

#[derive(Clone, serde::Serialize, sqlx::FromRow)]
pub struct ContactSummary {
    pub id: String,
    pub name: String,
    pub favorite: bool,
    pub href: String,
    #[sqlx(default)]
    pub active: bool, // set in Rust, not from DB
}

#[derive(Default)]
pub struct ContactUpdate {
    pub first: String,
    pub last: String,
    pub twitter: String,
    pub avatar: String,
    pub notes: String,
}
```

### Queries

```rust
pub async fn list(q: Option<&str>, active_id: Option<&str>) -> AppResult<Vec<ContactSummary>> {
    let needle = q.unwrap_or("").trim();
    let rows = if needle.is_empty() {
        sqlx::query_as::<_, ContactSummary>(
            "SELECT id,
                    COALESCE(NULLIF(TRIM(first || ' ' || last), ''), 'No Name') AS name,
                    favorite,
                    '/contacts/' || id AS href
             FROM contacts
             ORDER BY last, created_at DESC",
        )
        .fetch_all(crate::db::pool())
        .await
    } else {
        let pattern = format!("%{needle}%");
        sqlx::query_as::<_, ContactSummary>(
            "SELECT id,
                    COALESCE(NULLIF(TRIM(first || ' ' || last), ''), 'No Name') AS name,
                    favorite,
                    '/contacts/' || id AS href
             FROM contacts
             WHERE first ILIKE $1 OR last ILIKE $1 OR twitter ILIKE $1 OR notes ILIKE $1
             ORDER BY last, created_at DESC",
        )
        .bind(pattern)
        .fetch_all(crate::db::pool())
        .await
    }
    .map_err(|_| AppError::Internal)?;

    Ok(rows
        .into_iter()
        .map(|mut c| {
            c.active = active_id == Some(c.id.as_str());
            c
        })
        .collect())
}

pub async fn get(id: &str) -> AppResult<Option<Contact>> {
    sqlx::query_as::<_, Contact>(
        "SELECT id, first, last, avatar, twitter, notes, favorite,
                'Updated ' || to_char(updated_at AT TIME ZONE 'UTC',
                    'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label
         FROM contacts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)
}

pub async fn count() -> AppResult<i64> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM contacts")
        .fetch_one(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)
}
```

---

## 4. File layout

```text
pages/
  (app)/                      ← route group — shared layout, no URL segment
    _layout.html
    _layout.rs
    index.html                ← /
    index.rs
    contacts/
      [contact_id]/           ← /contacts/:contact_id
        index.html
        index.rs
        edit/
          index.html          ← /contacts/:contact_id/edit
          index.rs
  _layout.html                ← root HTML shell
  _not_found.html
  _error.html
```

`(app)` is a **route group** — it scopes the layout without adding a URL segment. All routes under it share the sidebar layout.

---

## 5. Root layout

### `pages/_layout.html`

The outermost HTML shell. All routes render inside it.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script src="https://cdn.tailwindcss.com"></script>
  <pilcrow:head />
</head>
<body class="min-h-screen bg-slate-100 text-slate-950 antialiased">
  <div>
    {{ content|safe }}
  </div>
  <script src="/__pilcrow/runtime/silcrow.js" defer></script>
</body>
</html>
```

`<pilcrow:head />` renders any `<pilcrow:head>` blocks declared in child pages. `{{ content|safe }}` renders the child page HTML.

---

## 6. App layout — sidebar

The `(app)` layout renders the two-column shell. Its `load()` fetches the contact list for the sidebar.

### `pages/(app)/_layout.rs`

```rust
pub struct Props {
    pub contacts: Vec<crate::data::ContactSummary>,
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

`active_id` comes from the current route's `contact_id` param if present — Pilcrow merges params from the full route chain into `req.params`.

### `pages/(app)/_layout.html`

```html
---
---
<div class="min-h-screen lg:grid lg:grid-cols-[22rem_1fr]">
  <aside id="sidebar" class="border-b border-slate-200 bg-white p-4 shadow-sm lg:border-r">
    <div class="mb-4 flex items-center justify-between gap-3">
      <h1 class="text-base font-semibold tracking-tight">
        <a href="/about" s-get="/about">Pilcrow Contacts</a>
      </h1>
      <span class="rounded-full bg-slate-100 px-2.5 py-1 text-xs font-medium">
        {{ sidebar_contact_count }} rows
      </span>
    </div>

    <div class="mb-4 grid grid-cols-[1fr_auto] gap-2">
      <form id="search-form" role="search" s-get="/" s-target="body" s-skip-history>
        <input name="q" type="search" value="{{ q }}"
               placeholder="Search"
               autocomplete="off"
               oninput="window.__contactSearch && window.__contactSearch(this.form)" />
      </form>
      <form method="post" action="/?/create" s-post="/?/create">
        <button type="submit">New</button>
      </form>
    </div>

    <nav>
      {% if contacts.len() > 0 %}
      <ul>
        {% for contact in contacts %}
        <li>
          <a href="{{ contact.href }}"
             s-get="{{ contact.href }}"
             class="{% if contact.active %}bg-blue-50 font-semibold text-blue-900{% else %}text-slate-600{% endif %}"
             {% if contact.active %}aria-current="page"{% endif %}>
            <span>{{ contact.name }}</span>
            {% if contact.favorite %}<span aria-label="Favorite">★</span>{% endif %}
          </a>
        </li>
        {% endfor %}
      </ul>
      {% else %}
      <p>No contacts</p>
      {% endif %}
    </nav>
  </aside>

  <main id="detail" class="p-8">
    <slot />
  </main>
</div>

<script>
  /* Debounced search — submits the search form 140ms after the user stops typing */
  window.__contactSearch = (function () {
    var timer = 0;
    return function (form) {
      clearTimeout(timer);
      timer = setTimeout(function () {
        if (window.Silcrow && window.Silcrow.submit) {
          window.Silcrow.submit(form, { replace: true });
        } else {
          form.requestSubmit();
        }
      }, 140);
    };
  })();

  /* Move the active sidebar link to match the current URL after Silcrow fragment nav */
  document.addEventListener('silcrow:load', function () {
    var path = window.location.pathname;
    document.querySelectorAll('#sidebar nav a[href]').forEach(function (a) {
      var active = a.getAttribute('href') === path;
      if (active) {
        a.classList.remove('text-slate-600');
        a.classList.add('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.setAttribute('aria-current', 'page');
      } else if (a.classList.contains('bg-blue-50')) {
        a.classList.remove('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.classList.add('text-slate-600');
        a.removeAttribute('aria-current');
      }
    });
  });

  /* Fade the detail pane during navigation */
  document.addEventListener('silcrow:navigate', function () {
    var detail = document.getElementById('detail');
    if (detail) detail.classList.add('opacity-50');
    setTimeout(function () {
      var next = document.getElementById('detail');
      if (next) next.classList.remove('opacity-50');
    }, 220);
  });
</script>
```

Key points:
- `<slot />` is where child page content renders inside this layout.
- `s-get="{{ contact.href }}"` enables Silcrow client-side navigation — Silcrow fetches the contact page and swaps only the right pane, leaving the sidebar intact.
- The `silcrow:load` listener moves the active highlight to match the current URL after Silcrow's PS fragment navigation (the server-rendered `contact.active` class only updates on full page renders).

---

## 7. Index route — welcome page

### `pages/(app)/index.rs`

```rust
use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub const PROMOTE_AFTER: u32 = 0; // static route — bake on first hit

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

- `PROMOTE_AFTER: u32 = 0` tells FSR to bake this route to Redis on the first hit. This is valid here because `/` is a static route — one URL instance.
- `live: Live` is injected by Pilcrow's FSR extractor before `load()` runs. It executes `Live::query()` against the database.
- `create` is a **named action** dispatched when the form POSTs to `/?/create`.
- `req.fsr.invalidate_route("/")` marks the `total_contacts` slot stale after a new contact is created. The watcher re-runs the query and pushes an SSE patch.
- `.retarget("#detail").push_history(&path)` tells Silcrow to swap the `#detail` element and push the new path to the browser history, all without a full page reload.

### `pages/(app)/index.html`

```html
<pilcrow:head>
  <title>Pilcrow Contacts</title>
</pilcrow:head>

<section class="flex min-h-[70vh] flex-col items-center justify-center text-center">
  <h1 class="text-4xl font-bold">Address Book</h1>
  <p class="mt-4 text-slate-600">
    Built with file routes, Rust loaders, Silcrow navigation, and FSR live fields.
  </p>
  <div class="mt-8 rounded-lg border bg-white px-6 py-4 shadow-sm">
    <span class="text-3xl font-bold" s-live="total_contacts">{{ live.total_contacts.value }}</span>
    <span class="ml-2 text-sm text-slate-500">live contacts</span>
  </div>
</section>
```

`s-live="total_contacts"` is the FSR slot. When the watcher detects the slot is stale (after `invalidate_route`), it re-runs the SQL and pushes an SSE event. Silcrow patches `textContent` of this element without a page reload.

---

## 8. Contact detail route

### `pages/(app)/contacts/[contact_id]/index.rs`

`[contact_id]` in the directory name becomes `:contact_id` in the Axum route. The value is available in `req.params`.

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

// No PROMOTE_AFTER — dynamic route with open ID-space
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
        .ok_or_else(|| AppError::NotFound("invalid contact id".into()))?;

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

pub async fn favorite(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
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

pub async fn destroy(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    crate::data::delete(&id).await?;
    req.fsr.tombstone(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route("/").await;
    redirect("/").retarget("#detail").push_history("/")
}

fn parse_id(req: &Req) -> AppResult<String> {
    req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid contact id".into()))
}
```

Key points:
- **No `PROMOTE_AFTER`** — dynamic routes have an open ID-space. Every `/contacts/<id>` URL is a separate row in the `pilcrow_fsr` table. Setting `PROMOTE_AFTER = 0` would mark every contact URL as promoted but leave `html_path = NULL` (no baked file), causing watcher warnings. Omit it; the route works correctly as SSR + live patching.
- `#[pilcrow::depends_on_route(contacts, contact_id)]` — applies to all `LiveProp<T>` fields in `Live`. Generates dep key `contacts:contact_id=<value>` for each request, wiring the slot to the specific contact row.
- `favorite` action — after toggling, invalidates both the detail route and the edit route (both show `updated_label`), then redirects with `.retarget("#detail")` to swap only the right pane.
- `destroy` action — uses `tombstone()` which marks the route dead, clears Redis keys, and removes any baked files. The next request to this URL returns 404.

### `pages/(app)/contacts/[contact_id]/index.html`

```html
<pilcrow:head>
  <title>{{ name }} - Pilcrow Contacts</title>
</pilcrow:head>

<article>
  <div>
    <img alt="{{ name }} avatar" src="{{ avatar }}" />
  </div>
  <div>
    <h1>
      {% if first.len() > 0 || last.len() > 0 %}{{ first }} {{ last }}{% else %}<i>No Name</i>{% endif %}
      <form method="post" action="?/favorite" s-post="?/favorite"
            onsubmit="window.__optimisticStar && window.__optimisticStar(this)">
        <button
          name="favorite"
          value="{% if favorite %}false{% else %}true{% endif %}"
          aria-label="{% if favorite %}Remove from favorites{% else %}Add to favorites{% endif %}"
          type="submit">
          <span id="favorite-slot" s-live="favorite_mark">{{ live.favorite_mark.value }}</span>
        </button>
      </form>
    </h1>

    {% if twitter.len() > 0 %}
    <p><a href="https://twitter.com/{{ twitter }}">@{{ twitter }}</a></p>
    {% endif %}

    {% if notes.len() > 0 %}
    <p>{{ notes }}</p>
    {% else %}
    <p><i>No notes yet.</i></p>
    {% endif %}

    <p class="text-sm text-slate-500">
      <span s-live="updated_label">{{ live.updated_label.value }}</span>
    </p>

    <div>
      <form method="get" action="/contacts/{{ id }}/edit">
        <button type="submit">Edit</button>
      </form>
      <form method="post" action="?/destroy" s-post="?/destroy"
            onsubmit="return confirm('Delete this contact?')">
        <button type="submit">Delete</button>
      </form>
    </div>
  </div>
</article>

<script>
  /* Optimistic favourite toggle — updates the star immediately on submit */
  window.__optimisticStar = function (form) {
    var slot = document.getElementById('favorite-slot');
    var btn  = form.querySelector('button[name="favorite"]');
    if (!slot || !btn) return true;
    slot.textContent = btn.value === 'true' ? '*' : '☆';
    return true;
  };
</script>
```

Both `s-live="favorite_mark"` and `s-live="updated_label"` are explicit slots — when the watcher re-runs the SQL after `invalidate_route`, it pushes SSE events that patch each element's `textContent`. Any browser tab showing this contact updates in real time.

---

## 9. Edit route

### `pages/(app)/contacts/[contact_id]/edit/index.rs`

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
    let id = parse_id(&req)?;
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
    let id = parse_id(&req)?;
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

    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;

    redirect(format!("/contacts/{id}"))
}

fn parse_id(req: &Req) -> AppResult<String> {
    req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid contact id".into()))
}
```

`save` invalidates both the detail and edit routes. Both show `updated_label`. Since `update()` calls `SET updated_at = now()`, both slots go stale and both connected tabs see the new timestamp.

### `pages/(app)/contacts/[contact_id]/edit/index.html`

```html
<pilcrow:head>
  <title>Edit Contact - Pilcrow Contacts</title>
</pilcrow:head>

<form method="post" action="?/save" s-post="?/save">
  <div>
    <label>Name</label>
    <div>
      <input name="first" placeholder="First" value="{{ first }}" />
      <input name="last"  placeholder="Last"  value="{{ last }}"  />
    </div>
  </div>
  <label>
    Twitter
    <input name="twitter" placeholder="@jack" value="{{ twitter }}" />
  </label>
  <label>
    Avatar URL
    <input name="avatar" placeholder="https://..." value="{{ avatar }}" />
  </label>
  <label>
    Notes
    <textarea name="notes" rows="6">{{ notes }}</textarea>
  </label>
  <div>
    <button type="submit">Save</button>
    <button type="button" onclick="history.back()">Cancel</button>
    <span class="text-xs text-slate-400">
      <span s-live="updated_label">{{ live.updated_label.value }}</span>
    </span>
  </div>
</form>
```

The `updated_label` slot in the form footer shows editors if someone else saved the contact while they were editing.

---

## 10. How Silcrow navigation works

Contact links use `s-get`:

```html
<a href="/contacts/{{ contact.id }}" s-get="/contacts/{{ contact.id }}">
  {{ contact.name }}
</a>
```

Silcrow intercepts the click, sends a GET with an `X-PS-Present` header listing the layout IDs currently in the DOM. The server detects this header and returns only the changed fragment (`[data-ps-slot="/contacts/:contact_id"]`) instead of the full page. Silcrow swaps the fragment into `[data-ps-slot]` in the DOM. The sidebar stays intact.

When navigating from the index page to a contact page, the index DOM has no contact slot — the swap fails and Silcrow falls back to `window.location.assign()` for a full navigation. This is expected.

**FSR SSE reconnect after navigation:**

The injected FSR script splits its reconnect across two Silcrow events:

- `silcrow:navigate` — fires before the DOM swap; closes the old SSE connection and captures the destination URL from `e.detail.url`.
- `silcrow:load` — fires after the DOM swap and `history.pushState`; reopens the SSE connection with the correct route and the updated DOM's `s-live` slots.

Never reconnect in `silcrow:navigate` — at that point `__fsr_slots()` still sees the old page's elements and would subscribe to the wrong slots.

---

## 11. FSR shape summary

| Route | Live slots | Invalidated by |
|---|---|---|
| `/` | `total_contacts` | `create`, `destroy` |
| `/contacts/[id]` | `favorite_mark`, `updated_label` | `favorite`, `save`, `destroy` |
| `/contacts/[id]/edit` | `updated_label` | `save`, `favorite` |

All three routes run `load()` on every request. The framework registers slots in `pilcrow_fsr`, the watcher re-runs stored SQL when slots go stale, and connected browsers receive SSE patches within the next poll cycle (200 ms).

---

## 12. Run it

```bash
# From the workspace root
cargo run --manifest-path address-book/Cargo.toml
```

Open `http://127.0.0.1:3010`.

To reset FSR state (e.g. after changing slot definitions):

```sql
TRUNCATE pilcrow_fsr;
```

Rows are re-registered on the next request to each route.

---

## What to explore next

- [[../03 Rendering/Build an FSR Page]] — full FSR field reference with debounce, depends_on, scheduled revalidation
- [[../03 Rendering/FSR SSE Hub]] — how the SSE connection works, scalar vs object fields, the class trap
- [[../05 Reference/FSR Ownership and Invalidation]] — app vs framework responsibility, invalidation patterns
- [[../02 Silcrow/Navigation and Live Connections]] — `s-get`, `s-post`, `s-target`, `silcrow:navigate` vs `silcrow:load`
- [[../01 Pilcrow/Actions and Forms]] — named actions, form parsing, `redirect()` modifiers
