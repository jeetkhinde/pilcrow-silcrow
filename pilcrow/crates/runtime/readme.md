# Pilcrow (Core Response Layer)

`pilcrow` is the low-level response/runtime crate used by `pilcrow-web`.

For app developers, Pilcrow has one mandatory path:

- UI lives in `apps/web` as file-based templates (`pages/components/layouts`)
- templates compile into Rust render functions at build time
- web handlers call backend APIs and return rendered HTML
- backend in `apps/backend` owns domain logic and returns JSON APIs

See [../../CONVENTION.md](../../CONVENTION.md).

## Canonical Example (from `astro_todo_demo`)

```rust
mod generated_templates {
    include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));
}

async fn index(State(state): State<AppState>) -> Response {
    let props = generated_templates::page_index::Props {
        title: "Astro Todo Preview".to_string(),
        todos: map_todos(&state.todos),
    };

    match generated_templates::page_index::render_page_index(props) {
        Ok(markup) => Html(markup).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Html(err.to_string())).into_response(),
    }
}
```

## Rule for App Code

- Do not call DB/repositories directly in `apps/web`.
- Do not render UI from `apps/backend`.
- Web calls backend via API clients and renders HTML from compiled templates.

## Docs

- [docs/guide.md](docs/guide.md)
- [../../crates/routekit/README.md](../../crates/routekit/README.md)
- [../../apps/README.md](../../apps/README.md)
