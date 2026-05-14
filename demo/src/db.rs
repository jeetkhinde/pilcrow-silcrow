use sqlx::PgPool;
use std::sync::OnceLock;

pub static POOL: OnceLock<PgPool> = OnceLock::new();

pub fn pool() -> &'static PgPool {
    POOL.get().expect("DB pool not initialized")
}
