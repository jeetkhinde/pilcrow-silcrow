# Islands

Pilcrow islands are server-rendered HTML fragments mounted through the `<island>` template tag.

## Usage

```html
<island src="/widgets/user-card" strategy="visible"></island>
```

Strategies:

- `load`
- `visible`
- `idle`

The default is `load`.

## Constraints

- `src` must be a static string.
- Askama expressions in `src` are not supported.
- Island files should live under co-located `pages/*/islands/` directories or configured fragment directories.

## When to Use

Use Pilcrow islands when you want server-rendered HTML to load separately from the page shell.

Use React islands when the island needs client-side React behavior. See [[React Islands]].
