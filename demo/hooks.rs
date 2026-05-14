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

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("failed to connect to DATABASE_URL");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tickets (
            id       BIGSERIAL PRIMARY KEY,
            title    TEXT NOT NULL,
            status   TEXT NOT NULL DEFAULT 'open',
            priority TEXT NOT NULL DEFAULT 'normal'
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create tickets table");

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
            PRIMARY KEY (route, slot)
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create pilcrow_fsr table");

    // Compatibility for local DBs created by an older demo schema.
    for statement in [
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS query TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS hit_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS promoted BOOLEAN NOT NULL DEFAULT FALSE",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS html_path TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS json_path TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS checksum TEXT",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS last_hit TIMESTAMPTZ",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS purge_after INTEGER",
        "ALTER TABLE pilcrow_fsr ALTER COLUMN query_params DROP NOT NULL",
        "ALTER TABLE pilcrow_fsr ADD COLUMN IF NOT EXISTS column_name TEXT",
        // Migrate old priority scheme (low/medium) to new (normal/very_high).
        "UPDATE tickets SET priority = 'normal' WHERE priority = 'low'",
        "UPDATE tickets SET priority = 'very_high' WHERE priority = 'medium'",
    ] {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("failed to migrate pilcrow_fsr table");
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS pilcrow_fsr_stale_idx ON pilcrow_fsr (stale) WHERE stale = TRUE",
    )
    .execute(&pool)
    .await
    .ok();

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS pilcrow_fsr_depends_on_gin ON pilcrow_fsr USING GIN (depends_on)",
    )
    .execute(&pool)
    .await
    .ok();

    // Seed 20 tickets if the table is empty.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tickets")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    if count.0 == 0 {
        let titles = [
            "Fix login timeout",
            "Update API docs",
            "Deploy to staging",
            "Improve error messages",
            "Add dark mode support",
            "Optimise DB indexes",
            "Write migration guide",
            "Fix mobile nav layout",
            "Add rate limiting",
            "Audit dependency versions",
            "Set up CI pipeline",
            "Improve test coverage",
            "Add CSV export",
            "Fix timezone handling",
            "Refactor auth middleware",
            "Add pagination to list views",
            "Implement file uploads",
            "Add email notifications",
            "Create admin dashboard",
            "Document FSR integration",
        ];
        let statuses = ["open", "open", "open", "closed"];
        let priorities = ["normal", "high", "very_high", "critical"];

        for (i, title) in titles.iter().enumerate() {
            let status = statuses[i % statuses.len()];
            let priority = priorities[i % priorities.len()];
            sqlx::query(
                "INSERT INTO tickets (title, status, priority) VALUES ($1, $2, $3)",
            )
            .bind(title)
            .bind(status)
            .bind(priority)
            .execute(&pool)
            .await
            .ok();
        }
        pilcrow_web::tracing::info!("seeded 20 tickets");
    }

    pilcrow_runtime::fsr::register_fsr_store(pool.clone());
    crate::db::POOL.set(pool).ok();
    pilcrow_web::tracing::info!("DB pool initialised");
}
