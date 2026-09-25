-- Archived listings include empty previews and cannot use the visible-only indexes.
CREATE INDEX idx_threads_archive_created_at_ms
    ON threads(archived, created_at_ms DESC, id DESC)
    WHERE archived = 1;

CREATE INDEX idx_threads_archive_updated_at_ms
    ON threads(archived, updated_at_ms DESC, id DESC)
    WHERE archived = 1;

CREATE INDEX idx_threads_archive_recency_at_ms
    ON threads(archived, recency_at_ms DESC, id DESC)
    WHERE archived = 1;
