# API Routes and Fragments

## API Routes

API routes are Rust files under `api/`.

```text
api/health.rs
```

```rust
pub fn router() -> axum::Router {
    axum::Router::new().route("/health", axum::routing::get(health))
}
```

The generated app wires API modules into the router. Do not mount them manually in `main.rs`.

## Fragments

Fragments are server-rendered HTML endpoints configured in `Pilcrow.toml`.

```toml
[[fragments]]
dir = "widgets"
url = "/widgets"
```

Fragment directories are not auto-discovered. The config is the contract.

Use fragments for reusable server-rendered HTML that should be addressable independently from full pages.

## Islands

Pilcrow islands use fragment-like server rendering behind an `<island>` tag. See [[../03 Rendering/Islands]].
