ALTER TABLE threads ADD COLUMN parent_thread_id TEXT;

CREATE INDEX idx_threads_parent_archived_created_at_ms
    ON threads(parent_thread_id, archived, created_at_ms DESC, id DESC);

CREATE INDEX idx_threads_parent_archived_updated_at_ms
    ON threads(parent_thread_id, archived, updated_at_ms DESC, id DESC);

UPDATE backfill_state
SET
    status = 'pending',
    last_watermark = NULL,
    updated_at = CAST(strftime('%s', 'now') AS INTEGER)
WHERE id = 1 AND status = 'complete';
