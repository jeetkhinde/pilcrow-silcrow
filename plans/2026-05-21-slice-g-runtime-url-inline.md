# Slice G — Silcrow runtime URL unification + `inline_runtime` config

**Date:** 2026-05-21  
**TODOs:** #15, #16  
**Branch:** `claude/slice-f-key-live-attrs` (same branch — these TODOs are small)

---

## Goal

1. **TODO #15** — Move `silcrow.js` serving URL from `/_silcrow/silcrow.{hash}.js` to
   `/__pilcrow/runtime/silcrow.{hash}.js`, unifying all Pilcrow runtime assets under the
   single `/__pilcrow/` namespace. The react/solid island assets (`/_pilcrow/`) are left
   unchanged (they are Pilcrow assets, not the Silcrow runtime).

2. **TODO #16** — Add `inline_runtime = true` to the `[client]` section of `Pilcrow.toml`.
   When true, `assets::script_tag()` returns an inline `<script>` block with the full
   Silcrow JS content instead of a `<script src="...">` tag.

---

## Design

### URL move (TODO #15)

`silcrow_js_path()` returns the content-hashed URL used in both the served route and the
`<script src>` tag. Changing one function fixes the entire chain. The CRC32 hash is
unchanged — only the path prefix changes.

`sw.rs` currently hardcodes `"/_silcrow/"` in the SW exclude list. After the move,
silcrow.js is under `/__pilcrow/runtime/`, which is already covered by the `"/__pilcrow/"`
exclude. The `"/_silcrow/"` entry becomes dead and is removed.

`config.rs` has a doc comment that mentions `/_silcrow/` — updated to `/__pilcrow/runtime/`.

### Inline runtime (TODO #16)

`script_tag()` is a public function called from Askama templates (`|safe` filter).
It cannot receive a parameter from the template easily without plumbing Config through
every Props struct. Instead, the setting is stored in a process-wide `AtomicBool`
(set once at startup from `Pilcrow.toml`) and read on each template render.

```
Pilcrow.toml:  [client]
               inline_runtime = true
                    ↓
start_with_adapter: set_inline_runtime(config.client.inline_runtime)
                    ↓
assets::script_tag(): checks INLINE_RUNTIME AtomicBool
                    ↓
template:  {{ pilcrow_web::assets::assets::script_tag()|safe }}
           → <script>...35KB of silcrow.js...</script>
              OR
           → <script src="/__pilcrow/runtime/silcrow.{hash}.js" defer></script>
```

The demo `_layout.html` is updated from the manual `<script src=...>` construction to
`{{ pilcrow_web::assets::assets::script_tag()|safe }}` so `inline_runtime` takes effect
automatically.

---

## Tasks

| # | File | Work |
|---|------|------|
| G1 | `crates/runtime/src/assets/assets.rs` | Change `silcrow_js_path()` prefix; add `INLINE_RUNTIME: AtomicBool`; add `set_inline_runtime(v)`; update `script_tag()` |
| G2 | `crates/runtime/src/start.rs` | Call `set_inline_runtime(config.client.inline_runtime)` at startup |
| G3 | `crates/runtime/src/sw.rs` | Remove `"/_silcrow/"` from SW excludes |
| G4 | `crates/core/src/config/config.rs` | Add `inline_runtime: bool` to `ClientRuntimeConfig`; fix `/_silcrow/` comment |
| G5 | `demo/pages/_layout.html` | Switch to `{{ pilcrow_web::assets::assets::script_tag()|safe }}` |
| G6 | `pilcrow/registry.toml` | Add `runtime-url-unification` + `inline-runtime-config` entries |
| G7 | `plans/implementation-progress.md` | Mark #15 + #16 done |

---

## Out of scope

- Actual JS module splitting of silcrow.js into separate files (requires refactoring
  silcrow/src as ES modules + bundler changes). The URL move aligns with the intent;
  actual splitting is deferred until the bundle warrants it or a bundler is added.
