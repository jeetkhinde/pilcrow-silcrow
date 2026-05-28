# FSR Cache Persistence & Eviction — Design Plan
_2026-05-24_

## Background

Current FSR baking flow:
- SQLite (`pilcrow_fsr` table) tracks `hit_count` and `promoted` per route
- On threshold: HTML+JSON baked to Redis (`pilcrow:html:*`, `pilcrow:json:*`, `pilcrow:slot:*`)
- Subsequent requests skip `load()` and serve from Redis
- No TTLs. No disk layer. Restart loses promotion state; hit-count resets.

## Problems identified

1. **No TTL on Redis keys** — orphaned keys after route renames, unbounded memory growth for dynamic routes (e.g. `/posts/[slug]` creates a key per unique slug), stale HTML survives deploys if dep-key invalidation is not called.
2. **Restart loses hit-count and promotion state** — SQLite may survive restarts, Redis does not. Every restart effectively un-promotes all baked routes.
3. **No activity-based eviction** — cold routes stay in Redis forever; no way to reclaim memory without a manual flush or tombstone.

## Agreed design direction

### Step 1 — Idle eviction + default TTL (small, do now)

- Add `last_hit_at: DateTime` column to `pilcrow_fsr` SQLite table, updated on every cache hit.
- Add a background task that runs every N minutes (configurable, default 30 min) and calls `delete_route_keys()` on routes where `last_hit_at` is older than a threshold (configurable, default 24h).
- Add `artifact_ttl_secs: Option<u64>` to `FsrConfig` (default `86400` = 24h). When set, `set_html`, `set_json` use `SET ... EX n`; `patch_slot` (HSET) follows with `EXPIRE`.
- With idle eviction in place, Redis misses are safe — they fall through to `load()` and re-promote normally.

Config surface (Pilcrow.toml):
```toml
[fsr]
artifact_ttl_secs = 86400      # 24h default
idle_evict_secs = 1800         # check every 30 min
idle_threshold_secs = 86400    # evict routes cold for > 24h
```

Files to touch:
- `pilcrow/crates/core/src/config/config.rs` — add fields to `FsrConfig`
- `pilcrow/crates/runtime/src/fsr/store.rs` — add `last_hit_at` column, update on hit
- `pilcrow/crates/runtime/src/fsr/cache.rs` — add `EX` to `set_html`, `set_json`; `EXPIRE` after `patch_slot`
- `pilcrow/crates/runtime/src/fsr/watcher.rs` or `start.rs` — spawn idle-eviction background task
- `pilcrow/registry.toml` — update FSR feature entry

### Step 2 — Disk-first baking (medium, next phase)

Goal: Redis becomes a hot cache loaded from disk. Restarts are cheap — scan artifacts, reload Redis. HTML changes delete the disk artifact and trigger re-bake.

Design:
- Each baked route gets a directory: `__pilcrow_bake/<route>/`
  - `shell.html` — baked HTML shell
  - `props.json` — baked JSON snapshot
  - `meta.json` — `{ "promote_after": N, "hit_count": N, "baked_at": "ISO8601" }`
- On startup: scan `__pilcrow_bake/`, for each `meta.json` found, reload `shell.html` + `props.json` into Redis. Restore `hit_count` to SQLite so promotion threshold is not reset.
- On promotion: write to disk first, then populate Redis.
- On invalidation: delete disk artifact first, then evict Redis.
- On HTML template change (rebuild): routekit codegen deletes stale `shell.html` files (or a `pilcrow bake --clean` CLI command).

This wires the existing `baked_pages/` module (already complete, no routekit integration yet) to the FSR promotion flow.

Files to touch:
- `pilcrow/crates/runtime/src/baked_pages/` — already has file I/O and `BakedPatchRegistry`; needs FSR integration hooks
- `pilcrow/crates/runtime/src/fsr/extractor.rs` — on promotion, write disk artifact before Redis
- `pilcrow/crates/runtime/src/fsr/watcher.rs` — on invalidation, delete disk artifact
- `pilcrow/crates/runtime/src/start.rs` — on startup, scan `__pilcrow_bake/` and warm Redis

### Step 3 — TTL becomes low-stakes (falls out of Step 2)

With disk-first baking, Redis TTL eviction no longer means "re-run load()" — it means "reload from disk into Redis on next hit." This is cheap and safe. The 24h default TTL from Step 1 becomes a memory pressure valve rather than a correctness concern.

## Remaining security/audit items (do before starting Step 1)

From `plans/security-performance-findings-2026-05-22.md`:
- [ ] FSR background task supervision — panics silently kill watcher tasks (`start.rs`, `fsr/watcher.rs`)
- [ ] FSR DB fan-out optimization — chunked `join_all` → `buffer_unordered(8)` (`fsr/watcher.rs`)

## TODOs in order

- [ ] **Step 0** (security, small): FSR background task supervision + `buffer_unordered(8)` fan-out
- [ ] **Step 1** (idle eviction + TTL): `last_hit_at`, background idle-evict task, `artifact_ttl_secs` config
- [ ] **Step 2** (disk-first): wire `baked_pages/` to FSR promotion/invalidation; startup Redis warm from disk
- [ ] **Step 3** (TTL revisit): after disk-first, reconsider TTL defaults (lower is now fine)
