# 08 — Search

The search box in the sidebar filters contacts as the user types.

## How it works

The search form uses `s-get="/" s-target="body"`:

```html
<form id="search-form" role="search"
      s-get="/" s-target="body" s-skip-history>
  <input
    name="q" type="search" value="{{ q }}"
    placeholder="Search" autocomplete="off"
    oninput="window.__contactSearch && window.__contactSearch(this.form)" />
</form>
```

- `s-get="/"` — on submit, Silcrow GETs `/` with the form values serialised as query params.
- `s-target="body"` — the response replaces the entire `<body>` instead of using PS fragment navigation. This is intentional: search results change the sidebar, so we need a full re-render of the layout.
- `s-skip-history` — does not push a history entry (search state does not need a back-button entry).

The layout's `load()` reads `req.query.get("q")` and passes it to `data::list()`, which applies the `ILIKE` filter.

## Debounced submission

Submitting on every keypress would fire a request per character. A small JS debouncer waits 140 ms after the user stops typing:

```html
<script>
  window.__contactSearch = (function () {
    var timer = 0;
    return function (form) {
      clearTimeout(timer);
      timer = setTimeout(function () {
        if (window.Silcrow && window.Silcrow.submit) {
          window.Silcrow.submit(form, { replace: true });
        } else {
          form.requestSubmit();
        }
      }, 140);
    };
  })();
</script>
```

`{ replace: true }` replaces the current history entry so repeated search inputs do not pollute the history stack.

## Keeping the input in sync

`value="{{ q }}"` on the input renders the current search term from `req.query`. After a full body swap, the input shows the term that produced the results — the URL and the visible input stay in sync automatically.

## Verify

Type in the search box. The contact list should filter after a short pause. Clearing the box should restore the full list.

---

Next: [[09 Active Link Styling]]
