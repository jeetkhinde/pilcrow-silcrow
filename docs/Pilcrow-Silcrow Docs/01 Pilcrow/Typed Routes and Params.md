
## Typed Route Helpers

Pilcrow generates a `routes` module with one helper per page route.

Static routes return `&'static str`; dynamic routes return `String`.

Use generated route helpers instead of hardcoding paths when linking between app routes.

## Param Matchers

Param matchers constrain dynamic route segments at request time.

```text
pages/products/[id=integer].html
params/integer.rs
```

```rust
pub fn match_param(value: &str) -> bool {
    value.parse::<i64>().is_ok()
}
```

Rules:

- Matcher files live in `params/`.
- The matcher filename must match the route segment matcher name.
- The file exports `pub fn match_param(value: &str) -> bool`.

## Route Groups

Route groups organize files without changing URLs:

```text
pages/(admin)/dashboard.html -> /dashboard
```

Use them for layout scoping and source organization.
