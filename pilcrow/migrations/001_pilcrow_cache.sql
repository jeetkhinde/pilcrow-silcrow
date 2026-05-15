CREATE TABLE IF NOT EXISTS pilcrow_cache (
    route        TEXT    NOT NULL,
    slot         TEXT    NOT NULL,
    params       JSONB   NOT NULL DEFAULT '{}',
    value        JSONB   NOT NULL,
    depends_on   TEXT[]  NOT NULL DEFAULT '{}',
    version      INTEGER NOT NULL DEFAULT 1,
    stale        BOOLEAN NOT NULL DEFAULT FALSE,
    hit_count    INTEGER NOT NULL DEFAULT 0,
    last_hit     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (route, slot, params)
);

CREATE TABLE IF NOT EXISTS pilcrow_routes (
    route        TEXT    PRIMARY KEY,
    html_path    TEXT,
    checksum     TEXT,
    stale        BOOLEAN NOT NULL DEFAULT FALSE,
    hit_count    INTEGER NOT NULL DEFAULT 0,
    promoted     BOOLEAN NOT NULL DEFAULT FALSE,
    purge_after  INTEGER
);
