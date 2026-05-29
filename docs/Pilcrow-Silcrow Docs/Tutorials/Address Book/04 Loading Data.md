# 04 — Loading Data

Add the index page. It shows a welcome message and a live contact count.

## `pages/(app)/index.rs`

```rust
use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub const PROMOTE_AFTER: u32 = 0;

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
```

There are three things here that are new:

**`pub const PROMOTE_AFTER: u32 = 0`** — tells FSR to bake this route to Redis on the first hit. After that, subsequent requests are served from the cache. This is valid here because `/` is a static route with a single URL instance. Never put `PROMOTE_AFTER` on dynamic routes like `/contacts/[id]` — see [[13 FSR Live Fields]] for why.

**`live: Live`** — the FSR extractor injects `Live` before `load()` runs. It executes `Live::query()` against the database and populates each `LiveProp<T>` field.

**`LiveProp<i64>`** — marks `total_contacts` as a live field. The framework registers it in the `pilcrow_fsr` table, and the watcher can push SSE updates to connected browsers when the value changes.

## `pages/(app)/index.html`

```html
<pilcrow:head>
  <title>Pilcrow Contacts</title>
</pilcrow:head>

<section class="mx-auto flex min-h-[70vh] max-w-2xl flex-col items-center justify-center text-center">
  <h1 class="text-4xl font-bold tracking-tight text-slate-950 sm:text-5xl">Address Book</h1>
  <p class="mt-4 text-base leading-7 text-slate-600">
    Built with file routes, Rust loaders, Silcrow navigation, and FSR live fields.
  </p>
  <div class="mt-8 rounded-lg border border-slate-200 bg-white px-6 py-4 shadow-sm">
    <span class="text-3xl font-bold text-slate-950"
          s-live="total_contacts">{{ live.total_contacts.value }}</span>
    <span class="ml-2 text-sm font-medium text-slate-500">live contacts</span>
  </div>
</section>
```

`s-live="total_contacts"` is the FSR patch target. When the watcher detects the slot is stale, it re-runs the SQL and sends an SSE event. Silcrow patches `textContent` of this element without a page reload.

`{{ live.total_contacts.value }}` renders the initial value at SSR time, so the page shows the correct number even before the SSE connection opens.

## Verify

Visit `http://127.0.0.1:3010`. You should see the welcome message and the contact count. The sidebar shows "0 rows" and an empty contact list — the seed data is added in the next step.

---

Next: [[05 Contact Detail]]
