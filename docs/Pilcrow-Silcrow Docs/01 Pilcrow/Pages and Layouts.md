

Pages are HTML templates with optional Rust code-behind.

## Minimal Page

```text
pages/products/index.html
pages/products/index.rs
```

```rust
pub struct Props {
    pub title: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Products".to_string(),
    })
}
```

```html
<h1>{{ title }}</h1>
```

Rules:

- If a `.rs` code-behind file exists, it must define `pub struct Props`.
- `load()` must be async.
- `load()` must accept `Req`.
- `load()` must return `AppResult<Props>`.
- If a page has no dynamic data, omit the `.rs` file.

## Layouts

A layout is named `_layout.html` and applies to pages in its directory subtree.

```html
<!doctype html>
<html>
  <body>
    {{ content|safe }}
  </body>
</html>
```

Layouts can have their own `_layout.rs` with `Props` and `load()`. Layout props are merged with page props.

Constraints:

- Layout files must be named `_layout.html`.
- Layout code-behind may define `load()`.
- Layouts cannot define named actions.
- Layout props and page props cannot use the same field name.
- A page can opt out with `pub const LAYOUT: &str = "none";`.

## Page Options

Common page constants live in the page code-behind:

```rust
pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
pub const LAYOUT: &str = "none";
pub const PROMOTE_AFTER: u32 = 50;
```

Use `PROMOTE_AFTER` for FSR promotion. Removed constants such as `REVALIDATE`, `STREAMING`, and `PRERENDER` should not be used.
