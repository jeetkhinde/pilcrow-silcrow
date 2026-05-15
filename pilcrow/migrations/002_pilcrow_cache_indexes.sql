-- GIN index for fast `depends_on @> ARRAY['key']` queries.
CREATE INDEX IF NOT EXISTS idx_pilcrow_cache_depends_on
    ON pilcrow_cache USING GIN (depends_on);

CREATE INDEX IF NOT EXISTS idx_pilcrow_cache_stale
    ON pilcrow_cache (route, stale)
    WHERE stale = FALSE;
