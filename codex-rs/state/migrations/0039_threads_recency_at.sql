ALTER TABLE threads ADD COLUMN recency_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE threads ADD COLUMN recency_at_ms INTEGER NOT NULL DEFAULT 0;

UPDATE threads
SET recency_at = updated_at,
    recency_at_ms = updated_at_ms;

-- Older binaries can open databases migrated by newer binaries. Seed recency
-- when one of those binaries inserts a thread without the new columns.
CREATE TRIGGER threads_recency_at_after_insert
AFTER INSERT ON threads
WHEN NEW.recency_at_ms = 0
BEGIN
    UPDATE threads
    SET recency_at = NEW.updated_at,
        recency_at_ms = COALESCE(NEW.updated_at_ms, NEW.updated_at * 1000)
    WHERE id = NEW.id;
END;

CREATE INDEX idx_threads_recency_at_ms
    ON threads(recency_at_ms DESC, id DESC);

CREATE INDEX idx_threads_archived_cwd_recency_at_ms
    ON threads(archived, cwd, recency_at_ms DESC, id DESC);

CREATE INDEX idx_threads_visible_recency_at_ms
    ON threads(archived, recency_at_ms DESC, id DESC)
    WHERE preview <> '';
