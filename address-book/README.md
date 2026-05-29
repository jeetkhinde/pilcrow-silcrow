# Pilcrow Address Book

The [React Router address book tutorial](https://reactrouter.com/tutorials/address-book) rebuilt as a Pilcrow + Silcrow app. It covers the same DX surfaces — file routes, layouts, loaders, actions — and adds FSR live fields on top.

## What it demonstrates

- **File-system routes** — `pages/(app)/contacts/[contact_id]/index.html`
- **Route-group layouts** — `pages/(app)/_layout.html` scopes the two-column sidebar without changing URLs
- **Typed server loading** — `Props` + `load()` co-located with each template
- **Named actions** — `?/create`, `?/save`, `?/favorite`, `?/destroy` posted by Silcrow-enhanced forms with normal HTTP fallbacks
- **FSR live fields** — two `LiveProp<String>` fields (`favorite_mark`, `updated_label`) on the contact detail route; the index route tracks `total_contacts`. All three update in the browser via SSE without a page reload when data changes.
- **Postgres-backed contacts** seeded from the React Router tutorial data
- **Redis-backed FSR** — watcher subscribes to `pilcrow:invalidate`, re-executes stored SQL, and publishes `pilcrow:patch` to connected SSE clients
- **Tailwind** via CDN (`<script src="https://cdn.tailwindcss.com">`)

## FSR shape

| Route | Live slots | Invalidated by |
|---|---|---|
| `/` | `total_contacts` | `create`, `destroy` |
| `/contacts/[id]` | `favorite_mark`, `updated_label` | `favorite`, `save`, `destroy` |
| `/contacts/[id]/edit` | `updated_label` | `save`, `favorite` |

All three routes run SSR on every request (`load()` always executes). `load()` returns `Props` with a `live: Live` field populated by the framework extractor; templates render live values into `s-live` slots. After any mutation the action calls `req.fsr.invalidate_route()`, which marks the affected slots stale in `pilcrow_fsr`. The embedded watcher detects stale rows, re-runs the stored SQL, and pushes SSE patches to connected clients within the next poll cycle (200 ms).

The index route (`/`) declares `PROMOTE_AFTER: u32 = 0` — a static route with a single URL instance. The contact and edit routes are dynamic (`[contact_id]`) so they use no `PROMOTE_AFTER`.

`load()` does only data fetching — the framework owns the FSR lifecycle.

## Setup

1. Create a Postgres database and put `DATABASE_URL` in `address-book/.env`.
2. Set `redis_url` in `address-book/Pilcrow.toml` under `[fsr]`.
3. The `pilcrow_fsr` and `contacts` tables are created automatically on first run.

## Run

```bash
cargo run --manifest-path address-book/Cargo.toml
```

Or from inside `address-book/`:

```bash
cargo run
```

Then open `http://127.0.0.1:3010`.

To reset the FSR state (e.g. after changing slot definitions):

```sql
TRUNCATE pilcrow_fsr;
```

Rows are re-registered on the next request to each route.
