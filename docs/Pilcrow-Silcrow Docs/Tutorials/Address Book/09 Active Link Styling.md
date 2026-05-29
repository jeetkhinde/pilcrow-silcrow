# 09 — Active Link Styling

The sidebar highlights the contact whose detail page is currently shown. There are two cases to handle: full page loads and fragment navigations.

## Full page loads

The layout's `load()` passes `active_id` to `data::list()`. The query sets `active = true` on the matching contact:

```rust
// In data::list()
Ok(rows
    .into_iter()
    .map(|mut c| {
        c.active = active_id == Some(c.id.as_str());
        c
    })
    .collect())
```

The template applies the active classes server-side:

```html
<a href="{{ contact.href }}"
   s-get="{{ contact.href }}"
   class="... {% if contact.active %}bg-blue-50 font-semibold text-blue-900{% else %}text-slate-600 hover:bg-slate-100{% endif %}"
   {% if contact.active %}aria-current="page"{% endif %}>
```

On a full page load — or after a search that replaces the body — the sidebar re-renders with the correct active state.

## Fragment navigations

When Silcrow performs a PS fragment navigation (contact-to-contact click), only the right pane swaps. The sidebar is **not** re-rendered. The previously-active link stays highlighted even after the URL and right pane have changed.

Fix this with a `silcrow:load` listener in the app layout script. `silcrow:load` fires after the DOM swap and `history.pushState`, so `window.location.pathname` is already the new URL:

```html
<script>
  document.addEventListener('silcrow:load', function () {
    var path = window.location.pathname;
    document.querySelectorAll('#sidebar nav a[href]').forEach(function (a) {
      var active = a.getAttribute('href') === path;
      if (active) {
        a.classList.remove('text-slate-600', 'hover:bg-slate-100', 'hover:text-slate-950');
        a.classList.add('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.setAttribute('aria-current', 'page');
      } else if (a.classList.contains('bg-blue-50')) {
        a.classList.remove('bg-blue-50', 'font-semibold', 'text-blue-900');
        a.classList.add('text-slate-600', 'hover:bg-slate-100', 'hover:text-slate-950');
        a.removeAttribute('aria-current');
      }
    });
  });
</script>
```

This runs after every Silcrow navigation and keeps the highlight in sync with the current URL.

## Why not `silcrow:navigate`?

`silcrow:navigate` fires **before** the DOM swap and **before** `history.pushState`. At that point `window.location.pathname` is still the old URL. Use `silcrow:load` for anything that reads the current URL or the updated DOM.

## Verify

Click between contacts — the sidebar highlight should move to the clicked contact on each navigation.

---

Next: [[10 Edit Form]]
