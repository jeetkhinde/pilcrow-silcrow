# Pilcrow Address Book

This is the React Router address book tutorial rebuilt as an independent Pilcrow/Silcrow project.

It mirrors the tutorial's core DX surfaces:

- file-system routes: `pages/(app)/contacts/[contact_id]/index.html`
- route-group layouts: `pages/(app)/_layout.html` scopes the sidebar without changing URLs
- typed server loading: each route keeps `Props` and `load()` beside its template
- named actions: `?/create`, `?/save`, `?/favorite`, and `?/destroy`
- Silcrow-enhanced links and forms with normal HTTP fallbacks
- Postgres-backed contacts seeded from the React Router tutorial data
- Tailwind via `<script src="https://cdn.tailwindcss.com"></script>`
- Redis-backed FSR using Pilcrow's `pilcrow_fsr` table, `PROMOTE_AFTER = 0`, and SQL-driven `LiveProp` fields

Configuration:

- Put `DATABASE_URL` in `.env`.
- Set the Redis URL in `Pilcrow.toml` under `fsr.redis_url`.
- Run from this directory so both files are discovered.

Run:

```bash
cargo run
```

Then open `http://127.0.0.1:3010`. If that port is already occupied, Pilcrow will print the fallback port.

After visiting `/` and `/contacts/ryan-florence`, the promoted artifacts appear under:

- `.pilcrow-baked/pages/*.html`
- `.pilcrow-baked/data/*.json`
