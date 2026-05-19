CREATE VIRTUAL TABLE thread_search USING fts5(
    thread_id UNINDEXED,
    body
);

UPDATE backfill_state
SET
    status = 'pending',
    last_watermark = NULL,
    last_success_at = NULL,
    updated_at = CAST(strftime('%s', 'now') AS INTEGER)
WHERE id = 1;
