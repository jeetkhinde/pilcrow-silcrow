- Always update both pilcrow and silcrow if APIs change
- Never leave silcrow.js out of sync
- Prefer build.rs automation over manual copy

## Syncing silcrow.js

Edit source files under `silcrow/src/`, then run `node build.js` (or `npm run build`)
inside `silcrow/`. No manual copy needed.

Pilcrow depends on silcrow via `package.json` (`"silcrow": "file:../silcrow"`).
`crates/runtime/build.rs` copies `node_modules/silcrow/dist/silcrow.js` into `OUT_DIR`
at Cargo build time, and `assets.rs` embeds it with `include_str!(concat!(env!("OUT_DIR"), "/silcrow.js"))`.

After editing silcrow source: `npm run build` in silcrow, then `cargo build` in pilcrow — that's it.
If node_modules is missing or stale, run `npm install` in the pilcrow root first.

## react-islands.js and Silcrow

`pilcrow/crates/runtime/assets/react-islands.js` is a **Pilcrow** asset, not a Silcrow one.

**Why it lives in Pilcrow:** React islands are a Pilcrow feature. Silcrow is a generic
DOM-patching library and has no knowledge of React. Moving the loader into Silcrow would
force a Silcrow→React dependency that violates the layering.

**How they connect:** Silcrow's `patcher.js` dispatches a `silcrow:patched` CustomEvent
after every DOM patch (detail includes `paths` and `target`). `react-islands.js` listens
for that event and re-scans `event.detail.target` for `[data-pilcrow-react]` elements,
mounting any newly inserted React islands.

## Attribute naming conventions

These two prefixes coexist in the HTML and serve different systems — they are not related:

- `s-*` / `:<prop>` — **Silcrow** binding directives (`s-use`, `s-for`, `:text`, `:class` …).
  Written by the developer in HTML templates; processed by Silcrow's client-side JS.
- `data-pilcrow-*` — **Pilcrow** server-rendered markers (`data-pilcrow-react`,
  `data-prop-*`, `data-src`, `data-strategy` …). Written by Pilcrow's routekit at
  build/render time; read by `react-islands.js` in the browser.

The `data-pilcrow-*` attributes look unrelated to Silcrow's `s-*` style because they are —
Silcrow never reads or writes them.
