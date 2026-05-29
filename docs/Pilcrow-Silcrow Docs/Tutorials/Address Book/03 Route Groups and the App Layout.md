# 03 — Route Groups and the App Layout

The address-book UI has two columns: a sidebar on the left and a detail pane on the right. That shell is shared by every page — the index, the contact detail, and the edit form. A **route group** applies this shared layout without adding a URL segment.

## Create the route group

```text
pages/
  (app)/          ← route group directory
    _layout.html
    _layout.rs
```

The parenthesised name `(app)` is stripped from the URL. Routes inside it share the `(app)/_layout` without any `/app/` prefix in the browser.

## `pages/(app)/_layout.rs`

The layout loader runs on every request for any page in the group. It fetches the sidebar data.

```rust
pub struct Props {
    pub contacts: Vec<crate::data::ContactSummary>,
    pub q: String,
    pub searching: bool,
    pub sidebar_contact_count: i64,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let q = req.query.get("q").unwrap_or("").to_owned();

    // req.params includes params from the full route chain.
    // contact_id is set when the user is on a contact detail or edit page.
    let active_id = req.params.get("contact_id").map(String::as_str);

    let contacts = crate::data::list(Some(&q), active_id).await?;
    let count    = crate::data::count().await?;

    Ok(Props {
        contacts,
        searching: !q.is_empty(),
        q,
        sidebar_contact_count: count,
    })
}
```

`active_id` lets the data layer mark the currently-viewed contact so the template can highlight it in the sidebar.

## `pages/(app)/_layout.html`

```html
---
---
<div class="min-h-screen lg:grid lg:grid-cols-[22rem_1fr]">
  <aside id="sidebar" class="border-b border-slate-200 bg-white p-4 shadow-sm
                              lg:min-h-screen lg:border-b-0 lg:border-r">

    <div class="mb-4 flex items-center justify-between gap-3">
      <h1 class="text-base font-semibold tracking-tight text-slate-950">
        <a href="/" s-get="/">Pilcrow Contacts</a>
      </h1>
      <span class="rounded-full bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-600">
        {{ sidebar_contact_count }} rows
      </span>
    </div>

    <div class="mb-4 grid grid-cols-[1fr_auto] gap-2">
      <form id="search-form" role="search" s-get="/" s-target="body" s-skip-history class="relative">
        <input
          name="q" type="search" value="{{ q }}"
          placeholder="Search" autocomplete="off"
          oninput="window.__contactSearch && window.__contactSearch(this.form)" />
      </form>
      <form method="post" action="/?/create" s-post="/?/create">
        <button type="submit"
          class="rounded-md bg-blue-700 px-3 py-2 text-sm font-semibold text-white">New</button>
      </form>
    </div>

    <nav aria-label="Contacts">
      {% if contacts.len() > 0 %}
      <ul class="grid gap-1">
        {% for contact in contacts %}
        <li>
          <a href="{{ contact.href }}"
             s-get="{{ contact.href }}"
             class="flex items-center justify-between gap-3 rounded-md px-3 py-2 text-sm transition
                    {% if contact.active %}bg-blue-50 font-semibold text-blue-900{% else %}text-slate-600 hover:bg-slate-100{% endif %}"
             {% if contact.active %}aria-current="page"{% endif %}>
            <span class="truncate">{{ contact.name }}</span>
            {% if contact.favorite %}<span class="text-amber-500" aria-label="Favourite">★</span>{% endif %}
          </a>
        </li>
        {% endfor %}
      </ul>
      {% else %}
      <p class="text-sm italic text-slate-500">No contacts</p>
      {% endif %}
    </nav>
  </aside>

  <main id="detail" class="min-w-0 p-8">
    <slot />
  </main>
</div>
```

The `---` frontmatter fences at the top mark this as a layout template. `<slot />` is where child page content renders.

## What `s-get` does

`s-get="{{ contact.href }}"` tells Silcrow to intercept clicks and perform a client-side navigation instead of a full page reload. Silcrow fetches the URL, and — because the app layout already exists in the DOM — swaps only the right pane. The sidebar stays intact.

You can still hold ⌘/Ctrl and open in a new tab. The link degrades to a plain `href` for keyboard users and bots.

## Verify

Move the placeholder `pages/index.html` into `pages/(app)/index.html`. Visit `http://127.0.0.1:3010` — you should see the two-column layout with an empty sidebar.

---

Next: [[04 Loading Data]]
