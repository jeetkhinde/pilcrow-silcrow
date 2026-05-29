# Config Env I18n Images and Head

## Typed Env Config

`Pilcrow.toml` can declare public and private environment variables. Pilcrow generates typed `Public` and `Private` env structs with compile-time field names.

Rules:

- Private env names must not start with `PUBLIC_`.
- `env::Private::global()` and `env::Public::global()` return process-wide singletons.
- `global()` is fail-fast: it panics if required env vars are missing.

## i18n

Pilcrow uses Fluent for internationalisation.

Rules:

- `i18n.locales` must include `i18n.default_locale`.
- The default locale is served at bare URLs.
- Non-default locales use a URL prefix such as `/de/` or `/fr/`.
- Fluent files are loaded at startup; changing them requires a server restart.
- `req.locale` is an empty string when i18n is not configured.

## Image Optimization

Image optimization serves images through:

```text
/_image?src=&w=&q=&f=
```

Rules:

- `images.enabled = true` is required.
- `images.domains` is a list of hostnames without protocol prefixes.
- `<pilcrow:image>` is transpiled by the template pipeline.

## Head and Meta

Use `<pilcrow:head>` for per-page metadata:

```html
<pilcrow:head>
  <title>Products</title>
  <meta name="description" content="Product catalog">
</pilcrow:head>
```

Rules:

- `<pilcrow:head>` must have a matching closing tag.
- Layouts that want per-page head control must declare `<slot name="pilcrow_head">`.
