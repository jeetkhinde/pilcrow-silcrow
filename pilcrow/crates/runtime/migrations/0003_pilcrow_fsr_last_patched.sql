-- Add last_patched_at to pilcrow_fsr for debounce coalescing.
-- The watcher sets this to NOW() each time it marks a slot fresh.
-- fetch_stale_slots uses it to skip slots still inside their debounce window.
ALTER TABLE pilcrow_fsr
    ADD COLUMN IF NOT EXISTS last_patched_at TIMESTAMPTZ;
