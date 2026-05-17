-- Add tombstoned column to pilcrow_fsr.
--
-- A tombstoned route had its backing entity deleted after being promoted.
-- The next request to a tombstoned route returns 404 instead of serving
-- the stale baked artifact.
--
-- tombstoned = TRUE is set by FsrStore::tombstone() which also clears Redis
-- keys and schedules async removal of baked artifacts from disk.
ALTER TABLE pilcrow_fsr
    ADD COLUMN IF NOT EXISTS tombstoned BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS pilcrow_fsr_tombstoned_idx
    ON pilcrow_fsr (route, tombstoned)
    WHERE tombstoned = TRUE;
