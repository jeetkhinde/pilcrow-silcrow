-- Pilcrow FSR (Field-Selective Rendering) metadata table.
--
-- One row per (route, slot) pair:
--   slot = ''           → route-level row (hit_count, promoted, html_path live here)
--   slot = 'field_name' → slot-level row (query, depends_on, stale live here)
--
-- Run via `sqlx migrate run` or embed with sqlx::migrate!("migrations").
CREATE TABLE IF NOT EXISTS pilcrow_fsr (
    route           TEXT        NOT NULL,
    slot            TEXT        NOT NULL  DEFAULT '',
    -- SQL to re-execute when stale = TRUE.  NULL on route-level rows.
    query           TEXT,
    -- Positional params for `query` as a JSON array, e.g. ["123"].
    query_params    JSONB,
    -- Array of "table:column=value" strings, e.g. ARRAY['tickets:id=123'].
    depends_on      TEXT[]      NOT NULL  DEFAULT '{}',
    -- Set to TRUE by invalidate!(); cleared by the watcher after re-baking.
    stale           BOOLEAN     NOT NULL  DEFAULT FALSE,
    -- Incremented on every stale→fresh transition so clients can detect gaps.
    version         INTEGER     NOT NULL  DEFAULT 0,
    -- Total request count for the route-level row.
    hit_count       INTEGER     NOT NULL  DEFAULT 0,
    -- TRUE once hit_count >= promote_after; triggers baked-HTML serving.
    promoted        BOOLEAN     NOT NULL  DEFAULT FALSE,
    -- Per-field override for the framework default promote_after_hits.
    -- NULL means "use framework default".  0 or NULL = SSG (bake at startup).
    promote_after   INTEGER,
    -- Per-field override for the framework default patch_debounce_secs.
    debounce_secs   INTEGER,
    -- Name of the SQL column to extract from the query result.
    -- NULL falls back to the slot name itself.
    column_name     TEXT,
    -- Absolute filesystem path to the baked HTML file (promoted routes only).
    html_path       TEXT,
    -- Absolute filesystem path to the baked JSON file (FSR_JSON opt-in only).
    json_path       TEXT,
    -- CRC32/SHA256 of the last baked HTML for cache-busting.
    checksum        TEXT,
    -- Timestamp of the most recent request that incremented hit_count.
    last_hit        TIMESTAMPTZ,
    -- Seconds after which baked artefacts are eligible for purge.
    -- NULL means never purge.
    purge_after     INTEGER,
    PRIMARY KEY (route, slot)
);

-- Fast lookup of all stale slots — used by the watcher on every tick.
CREATE INDEX IF NOT EXISTS pilcrow_fsr_stale_idx
    ON pilcrow_fsr (stale)
    WHERE stale = TRUE;

-- GIN index for `depends_on @> ARRAY[...]` queries used by invalidate!().
CREATE INDEX IF NOT EXISTS pilcrow_fsr_depends_on_idx
    ON pilcrow_fsr USING GIN (depends_on);
