# 01 — Setup

Create the project and wire up the database.

## Create the directory

```bash
mkdir address-book && cd address-book
```

## `Cargo.toml`

```toml
[package]
name = "pilcrow-address-book"
version = "0.1.0"
edition = "2021"

[dependencies]
pilcrow-web    = { path = "../pilcrow/crates/web", features = ["live-props"] }
pilcrow_runtime = { path = "../pilcrow/crates/runtime", features = ["live-props-redis"] }
sqlx   = { version = "0.8", features = ["runtime-tokio-native-tls", "postgres", "json", "chrono"] }
tokio  = { version = "1",   features = ["full"] }
dotenvy = "0.15"
serde  = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
pilcrow-routekit = { path = "../pilcrow/crates/routekit" }
```

`live-props` enables FSR. `live-props-redis` adds Redis caching and pub/sub-driven invalidation.

## `build.rs`

```rust
fn main() {
    routekit::compile_current_crate_sources().expect("pilcrow compile");
}
```

This is the only build script you need. Routekit discovers your pages, compiles templates, and generates the Axum router at build time.

## `Pilcrow.toml`

```toml
[web]
host = "127.0.0.1"
port = 3010

[fsr]
poll_interval_ms   = 200
revalidate_seconds = 15
redis_url          = "redis://127.0.0.1:6379"
```

## `.env`

```
DATABASE_URL=postgresql://user:password@localhost:5432/address_book
```

## `src/main.rs`

```rust
pilcrow_web::pilcrow_app!();

mod data;
mod db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pilcrow_start(pilcrow_router()).await
}
```

`pilcrow_app!()` expands to the generated router, which routekit writes to `OUT_DIR` at compile time. `pilcrow_start()` wires Axum, the FSR watcher, Redis, and standard middleware (CSRF, compression, timeout).

## `src/db.rs`

```rust
use sqlx::PgPool;
use std::sync::OnceLock;

pub static POOL: OnceLock<PgPool> = OnceLock::new();

pub fn pool() -> &'static PgPool {
    POOL.get().expect("DB pool not initialized")
}
```

## `hooks.rs` — init before serving

Pilcrow calls `hooks::init()` before the server accepts any requests.

```rust
pub async fn init() {
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&db_url).await.expect("DB connect failed");

    create_tables(&pool).await;
    crate::data::seed_if_empty(&pool).await.expect("seed failed");

    // Register FSR store — must run before any FSR route handles a request
    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
}

async fn create_tables(pool: &sqlx::PgPool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS contacts (
            id         TEXT PRIMARY KEY,
            first      TEXT NOT NULL DEFAULT '',
            last       TEXT NOT NULL DEFAULT '',
            avatar     TEXT NOT NULL DEFAULT '',
            twitter    TEXT NOT NULL DEFAULT '',
            notes      TEXT NOT NULL DEFAULT '',
            favorite   BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(pool)
    .await
    .expect("create contacts table");

    // pilcrow_fsr is created by the framework migration — see hooks.rs in source for full schema
}
```

## `src/data.rs` — types

```rust
#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct Contact {
    pub id: String,
    pub first: String,
    pub last: String,
    pub avatar: String,
    pub twitter: String,
    pub notes: String,
    pub favorite: bool,
    pub updated_label: String,
}

#[derive(Clone, serde::Serialize, sqlx::FromRow)]
pub struct ContactSummary {
    pub id: String,
    pub name: String,
    pub favorite: bool,
    pub href: String,
    #[sqlx(default)]
    pub active: bool,
}

#[derive(Default)]
pub struct ContactUpdate {
    pub first: String,
    pub last: String,
    pub twitter: String,
    pub avatar: String,
    pub notes: String,
}
```

## Run it

```bash
cargo run
```

You should see `pilcrow: listening on http://127.0.0.1:3010`. The server returns 404 for now — there are no pages yet.

---

Next: [[02 Root Layout]]
