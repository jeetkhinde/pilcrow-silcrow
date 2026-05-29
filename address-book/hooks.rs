use pilcrow_web::{HookError, Next, Req, Response};

pub async fn handle(_req: Req, next: Next) -> Response {
    next.run().await
}

pub async fn handle_error(error: &HookError, _req: &Req) -> Option<Response> {
    pilcrow_web::tracing::error!(status = error.status, "server error: {}", error.message);
    None
}

pub async fn init() {
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in address-book/.env");
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("failed to connect to DATABASE_URL");

    create_contacts_table(&pool).await;
    create_fsr_table(&pool).await;
    crate::data::seed_if_empty(&pool)
        .await
        .expect("failed to seed contacts");

    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
}

async fn create_contacts_table(pool: &sqlx::PgPool) {
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
    .expect("failed to create contacts table");

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS contacts_name_idx ON contacts (last, first, created_at DESC)",
    )
    .execute(pool)
    .await
    .ok();
}

async fn create_fsr_table(pool: &sqlx::PgPool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pilcrow_fsr (
            route           TEXT        NOT NULL,
            slot            TEXT        NOT NULL DEFAULT '',
            query           TEXT,
            query_params    JSONB,
            depends_on      TEXT[]      NOT NULL DEFAULT '{}',
            stale           BOOLEAN     NOT NULL DEFAULT FALSE,
            version         INTEGER     NOT NULL DEFAULT 0,
            hit_count       INTEGER     NOT NULL DEFAULT 0,
            promoted        BOOLEAN     NOT NULL DEFAULT FALSE,
            promote_after   INTEGER,
            debounce_secs   INTEGER,
            html_path       TEXT,
            json_path       TEXT,
            column_name     TEXT,
            checksum        TEXT,
            last_hit        TIMESTAMPTZ,
            purge_after     INTEGER,
            tombstoned      BOOLEAN     NOT NULL DEFAULT FALSE,
            PRIMARY KEY (route, slot)
        )",
    )
    .execute(pool)
    .await
    .expect("failed to create pilcrow_fsr table");

    for statement in [
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS query TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS query_params JSONB",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS depends_on TEXT[] NOT NULL DEFAULT '{}'",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS stale BOOLEAN NOT NULL DEFAULT FALSE",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS hit_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS promoted BOOLEAN NOT NULL DEFAULT FALSE",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS promote_after INTEGER",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS debounce_secs INTEGER",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS html_path TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS json_path TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS column_name TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS checksum TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS last_hit TIMESTAMPTZ",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS purge_after INTEGER",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS tombstoned BOOLEAN NOT NULL DEFAULT FALSE",
        "ALTER TABLE pilcrow_fsr ALTER COLUMN query_params DROP NOT NULL",
        "CREATE INDEX IF NOT EXISTS pilcrow_fsr_stale_idx ON pilcrow_fsr (stale) WHERE stale = TRUE",
        "CREATE INDEX IF NOT EXISTS pilcrow_fsr_depends_on_gin ON pilcrow_fsr USING GIN (depends_on)",
        "CREATE INDEX IF NOT EXISTS pilcrow_fsr_tombstoned_idx ON pilcrow_fsr (route, tombstoned) WHERE tombstoned = TRUE",
    ] {
        sqlx::query(statement)
            .execute(pool)
            .await
            .expect("failed to migrate pilcrow_fsr table");
    }
}
