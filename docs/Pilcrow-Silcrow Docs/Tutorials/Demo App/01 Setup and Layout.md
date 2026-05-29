# 01 — Setup and Layout

## Project structure

```
demo/
  pages/
    _layout.html        ← root HTML shell with Nav import
    _loading.html       ← global loading skeleton
    _error.html
    _not_found.html
    index.html / .rs    ← home page
    about/ tickets/ products/ demo/ …
  ui/
    Nav.html            ← shared nav component
  api/
    tickets.rs
    products.rs
  src/
    main.rs
    db.rs
  hooks.rs
```

## `Cargo.toml` — key differences from address-book

```toml
[dependencies]
pilcrow-web = { path = "../pilcrow/crates/web", features = ["live-props"] }
pilcrow_runtime = { path = "../pilcrow/crates/runtime", features = ["live-props-redis"] }
reqwest = { version = "0.12", features = ["json"] }
urlencoding = "2"
sqlx = { version = "0.8", features = ["runtime-tokio-native-tls", "postgres", "json"] }
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
serde = { version = "1", features = ["derive"] }
```

`reqwest` is used by the products page to fetch from an external API.

## `Pilcrow.toml`

```toml
[web]
host = "127.0.0.1"
port = 3000

[routing]
ignore_directories = ["react", "solid"]   # JS island source dirs

[imports]
ui = "ui"                                  # import alias → ui/ directory

[client.react]
enabled  = true
dirs     = ["react"]
max_island_kb = 80
ssr      = true

[fsr]
redis_url        = "redis://127.0.0.1:6379"
poll_interval_ms = 200
```

`ignore_directories` prevents routekit from treating `react/` and `solid/` as page directories. `[imports]` creates the `ui` alias used in layout frontmatter.

## `src/main.rs`

```rust
pilcrow_web::pilcrow_app!();

mod baked_pages;
mod db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pilcrow_start(baked_pages::router().merge(pilcrow_router())).await
}
```

`baked_pages::router()` registers a few manually-defined Axum routes for the experimental baked-pages system. `pilcrow_router()` is the generated router from routekit. `.merge()` combines them before passing to `pilcrow_start`.

## `hooks.rs` — database init

```rust
pub async fn init() {
    dotenvy::dotenv().ok();
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tickets (
            id       BIGSERIAL PRIMARY KEY,
            title    TEXT NOT NULL,
            status   TEXT NOT NULL DEFAULT 'open',
            priority TEXT NOT NULL DEFAULT 'normal'
        )",
    )
    .execute(&pool).await.unwrap();

    // pilcrow_fsr table creation + seed 20 tickets if empty
    // (full code in hooks.rs in the source tree)

    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
}
```

## Root layout with UI component import

`pages/_layout.html` uses frontmatter to import the nav component:

```html
---
import Nav from "ui/Nav.html";

pub struct Props {}
---
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <slot name="pilcrow_head">
    <title>Pilcrow</title>
  </slot>
  <style>/* global styles */</style>
</head>
<body>
  <a href="#main-content" class="skip-link">Skip to main content</a>
  <Nav />
  <main id="main-content">
    <slot />
  </main>
  {{ pilcrow_web::assets::assets::script_tag()|safe }}
</body>
</html>
```

Three new things compared to the address-book layout:

**`import Nav from "ui/Nav.html"`** — imports a shared UI component using the `ui` alias defined in `Pilcrow.toml`. The component renders inline wherever `<Nav />` appears. UI components are server-rendered HTML partials; they can have their own Rust frontmatter with a `Props` struct and `load()`.

**`<slot name="pilcrow_head">`** — a named slot. Child pages write to it with `<pilcrow:head>`. The default content (`<title>Pilcrow</title>`) shows when no child page provides a head block.

**`{{ pilcrow_web::assets::assets::script_tag()|safe }}`** — injects the Silcrow script tag with a content-hash filename for cache busting. Equivalent to writing `<script src="/__pilcrow/runtime/silcrow.{hash}.js" defer>` but always current.

## Loading skeleton

`pages/_loading.html` shows while Silcrow fetches a new page:

```html
<div class="loading-bar"></div>
```

Routekit discovers this file automatically. Silcrow appends it to the response body during navigation transitions.

---

Next: [[02 Basic SSR Pages]]
