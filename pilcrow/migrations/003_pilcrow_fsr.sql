-- FSR (Field-Selective Rendering) unified table.
--
-- slot = ''           → route-level row (html_path, hit_count, promoted live here)
-- slot = 'field_name' → watched field row
-- slot = 'list__rowid__field' → list row field
--
-- query + query_params store the SQL to re-execute when stale = TRUE.
-- Values are never stored — the real DB tables are always the source of truth.

CREATE TABLE IF NOT EXISTS pilcrow_fsr (
    route           TEXT        NOT NULL,
    slot            TEXT        NOT NULL  DEFAULT '',
    query           TEXT,
    query_params    JSONB,
    depends_on      TEXT[]      NOT NULL  DEFAULT '{}',
    stale           BOOLEAN     NOT NULL  DEFAULT FALSE,
    version         INTEGER     NOT NULL  DEFAULT 0,
    hit_count       INTEGER     NOT NULL  DEFAULT 0,
    promoted        BOOLEAN     NOT NULL  DEFAULT FALSE,
    promote_after   INTEGER,
    debounce_secs   INTEGER,
    html_path       TEXT,
    json_path       TEXT,
    checksum        TEXT,
    last_hit        TIMESTAMPTZ,
    purge_after     INTEGER,
    PRIMARY KEY (route, slot)
);

CREATE INDEX IF NOT EXISTS pilcrow_fsr_stale_idx
    ON pilcrow_fsr (stale)
    WHERE stale = TRUE;

CREATE INDEX IF NOT EXISTS pilcrow_fsr_depends_on_idx
    ON pilcrow_fsr USING GIN (depends_on);
