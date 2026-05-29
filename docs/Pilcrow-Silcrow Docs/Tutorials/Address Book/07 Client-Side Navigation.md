# 07 — Client-Side Navigation

The sidebar links already have `s-get`. This step explains how that navigation works and what Pilcrow does behind the scenes.

## How `s-get` works

When a user clicks an `s-get` link, Silcrow:

1. Sends a GET request with an `X-PS-Present` header listing the layout IDs currently rendered in the DOM (e.g. `/, /(app)`).
2. The server sees the header and returns only the changed **fragment** — the `[data-ps-slot="/contacts/:contact_id"]` div — instead of the full HTML page. The content type is `text/html; x-ps-fragment=1`.
3. Silcrow finds the matching `[data-ps-slot]` in the current DOM and swaps its `innerHTML`.
4. The sidebar stays intact. Only the right pane updates.
5. `history.pushState` updates the browser URL.

This is called **PS (Pilcrow Slots) fragment navigation**. Routekit injects the `data-ps-layout` and `data-ps-slot` attributes at build time — you do not set them manually.

## Fallback

If the current DOM does not have a matching `[data-ps-slot]` (e.g. when navigating from the index page, which has no contact slot), `applyFragment` returns false and Silcrow falls back to `window.location.assign(url)` — a full page load. This is correct and expected.

## FSR SSE after navigation

Each time Silcrow navigates, the FSR client script must reconnect to `/__pilcrow/fsr` with the new route and the new `s-live` slots. It does this across two events:

| Event | Fires | What happens |
|---|---|---|
| `silcrow:navigate` | Before fetch, before DOM swap | Close old SSE connection. Capture destination URL from `e.detail.url`. |
| `silcrow:load` | After DOM swap + `pushState` | Reopen SSE with `window.location.pathname` and the new DOM's `s-live` elements. |

**Why the split?** `silcrow:navigate` fires before the page content has changed. If you reconnected there, `__fsr_slots()` would scan the old DOM and subscribe to the wrong slots on the new route.

## Action navigation

Actions use `.retarget("#detail")` so only the right pane updates after a form submit:

```rust
redirect(&path).retarget("#detail").push_history(&path)
```

This is equivalent to `s-get` navigation but triggered by a POST response.

## Verify

Click a contact in the sidebar — the URL should change and the right pane should update without the sidebar re-rendering. Use browser devtools Network tab to confirm the response content-type is `text/html; x-ps-fragment=1`.

---

Next: [[08 Search]]
