# Routing

Pilcrow derives routes from files instead of hand-written router setup.

## Page Routes

```text
pages/index.html              -> GET /
pages/products/index.html     -> GET /products
pages/products/[id].html      -> GET /products/:id
pages/[id=integer]/index.html -> GET /:id with matcher
```

Rules:

- Page files must live under `pages/`.
- `index.html` maps to the directory path.
- Dynamic params use bracket syntax: `[id]`.
- Param matchers use `[id=integer]` and map to `params/integer.rs`.
- Route groups use parentheses and do not affect the URL: `pages/(admin)/dashboard.html -> /dashboard`.

## API Routes

API route files live under `api/` and export:

```rust
pub fn router() -> axum::Router
```

Do not mount API routers manually in `main.rs`; the build pipeline wires them.

## Ignore Directories

Use `Pilcrow.toml` to skip directories during route discovery:

```toml
[routing]
ignore_directories = ["Cards", "pages/admin/**/components"]
```

Patterns without slashes match recursively by directory name. Patterns with slashes are evaluated relative to the source directory.

## Common Mistakes

- Using Axum `:id` syntax in filenames. Pilcrow filenames use `[id]`.
- Manually registering page routes in `main.rs`.
- Placing route files outside `pages/` or `api/`.
