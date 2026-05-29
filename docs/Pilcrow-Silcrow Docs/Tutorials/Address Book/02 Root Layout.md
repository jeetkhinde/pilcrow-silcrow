# 02 — Root Layout

The root layout is the outermost HTML shell. Every page in your app renders inside it.

## Create `pages/_layout.html`

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script src="https://cdn.tailwindcss.com"></script>
  <pilcrow:head />
</head>
<body class="min-h-screen bg-slate-100 text-slate-950 antialiased">
  {{ content|safe }}
  <script src="/__pilcrow/runtime/silcrow.js" defer></script>
</body>
</html>
```

Two things to know:

- **`<pilcrow:head />`** — renders any `<pilcrow:head>` block declared in a child page. Use it to project per-page `<title>`, `<meta>`, and `<link>` tags into the document head.
- **`{{ content|safe }}`** — renders the current child page's HTML. The `|safe` filter bypasses Minijinja escaping so the raw HTML is emitted.

## Where the script tag goes

Silcrow is loaded by `<script src="/__pilcrow/runtime/silcrow.js" defer>`. Place it at the end of `<body>`. Pilcrow serves this file automatically — you do not need to copy or build it.

## Page titles

Individual pages declare their title with:

```html
<pilcrow:head>
  <title>Contact Detail — Pilcrow</title>
</pilcrow:head>
```

This block is projected into `<pilcrow:head />` in the root layout.

## Verify

Add a minimal index page to test the layout:

`pages/index.html`:

```html
<h1>Hello</h1>
```

Visit `http://127.0.0.1:3010` — you should see the page wrapped in the full HTML shell.

---

Next: [[03 Route Groups and the App Layout]]
