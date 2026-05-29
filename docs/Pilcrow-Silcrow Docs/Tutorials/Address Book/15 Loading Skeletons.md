# 15 — Loading Skeletons

Give the detail pane a pending state so navigation between contacts never shows a blank or stale pane while the next fragment is in flight.

## What a loading skeleton is

A `_loading.html` template is a **scoped pending-UI placeholder**. Pilcrow discovers any `_loading.html` inside `pages/` (see `is_loading_page_file` in `routekit/src/routing/discovery.rs`), injects it into the rendered response as a `<template>`, and Silcrow shows it inside the navigation target while the next route's fragment is loading. It is not a route — you never navigate *to* it.

It is scoped by directory, exactly like `_layout.html`: a `_loading.html` in `(app)/` applies to every route in the `(app)` group.

## Add `pages/(app)/_loading.html`

```html
<div class="mx-auto max-w-3xl animate-pulse">
  <div class="flex items-center gap-5">
    <div class="h-32 w-32 shrink-0 rounded-2xl bg-slate-200"></div>
    <div class="flex-1 space-y-3">
      <div class="h-8 w-2/3 rounded bg-slate-200"></div>
      <div class="h-4 w-1/3 rounded bg-slate-200"></div>
    </div>
  </div>
  <div class="mt-8 space-y-3">
    <div class="h-4 w-full rounded bg-slate-200"></div>
    <div class="h-4 w-5/6 rounded bg-slate-200"></div>
    <div class="h-4 w-2/3 rounded bg-slate-200"></div>
  </div>
</div>
```

The skeleton mirrors the shape of the real contact detail (avatar block, name line, notes lines) so the layout does not jump when the real content arrives.

## How it pairs with fragment navigation

This is the missing half of [[07 Client-Side Navigation]]. There, `s-get` swaps only the `#detail` pane. On a fast local connection you rarely see a gap, but on a slow contact load the pane would otherwise sit blank or show the previous contact. The skeleton fills that window:

| Phase | What the user sees |
|---|---|
| `silcrow:navigate` fires | The `#detail` fragment is requested; the skeleton is shown in the target |
| Fragment arrives | Silcrow swaps the real contact detail in, replacing the skeleton |
| `silcrow:load` fires | FSR SSE reconnects for the new route (see [[07 Client-Side Navigation]]) |

The `(app)/_layout.html` already fades the detail pane with `transition-opacity` on `silcrow:navigate`; the skeleton and the fade work together.

## Why this matters

The About page advertises "pending UI" as one of the app's behaviours. Before this step that claim was aspirational — there was no loading state. A `_loading.html` is the smallest possible way to make it real, and it costs zero Rust: it is a pure template the framework wires automatically.

## Verify

Throttle your network in devtools (e.g. "Slow 3G"), then click between contacts in the sidebar. The detail pane should show the shimmering skeleton for the duration of the fragment fetch, then snap to the real contact. The sidebar never flickers.

---

This is the final step. Back to [[00 Introduction]].
