ALTER TABLE threads ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_threads_pinned_recency_at_ms
    ON threads(archived, recency_at_ms DESC, id DESC)
    WHERE is_pinned = 1 AND preview <> '';
