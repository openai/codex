ALTER TABLE threads ADD COLUMN created_at_ms INTEGER;
ALTER TABLE threads ADD COLUMN updated_at_ms INTEGER;

CREATE TEMP TABLE thread_timestamp_migration AS
SELECT
    id,
    created_at,
    updated_at,
    (
        SELECT COUNT(*)
        FROM threads AS prev
        WHERE prev.updated_at = threads.updated_at
          AND prev.id < threads.id
    ) AS updated_at_offset
FROM threads;

UPDATE threads
SET created_at_ms = (
    SELECT
        CASE
            WHEN created_at < 1577836800000 THEN created_at * 1000
            ELSE created_at
        END
    FROM thread_timestamp_migration
    WHERE thread_timestamp_migration.id = threads.id
);

UPDATE threads
SET updated_at_ms = (
    SELECT
        CASE
            WHEN updated_at < 1577836800000 THEN updated_at * 1000 + updated_at_offset
            ELSE updated_at + updated_at_offset
        END
    FROM thread_timestamp_migration
    WHERE thread_timestamp_migration.id = threads.id
);

DROP TABLE thread_timestamp_migration;

CREATE TRIGGER threads_created_at_ms_after_insert
AFTER INSERT ON threads
WHEN NEW.created_at_ms IS NULL
BEGIN
    UPDATE threads
    SET created_at_ms = NEW.created_at * 1000
    WHERE id = NEW.id;
END;

CREATE TRIGGER threads_updated_at_ms_after_insert
AFTER INSERT ON threads
WHEN NEW.updated_at_ms IS NULL
BEGIN
    UPDATE threads
    SET updated_at_ms = NEW.updated_at * 1000
    WHERE id = NEW.id;
END;

CREATE TRIGGER threads_created_at_ms_after_update
AFTER UPDATE OF created_at ON threads
WHEN NEW.created_at != OLD.created_at
 AND NEW.created_at_ms IS OLD.created_at_ms
BEGIN
    UPDATE threads
    SET created_at_ms = NEW.created_at * 1000
    WHERE id = NEW.id;
END;

CREATE TRIGGER threads_updated_at_ms_after_update
AFTER UPDATE OF updated_at ON threads
WHEN NEW.updated_at != OLD.updated_at
 AND NEW.updated_at_ms IS OLD.updated_at_ms
BEGIN
    UPDATE threads
    SET updated_at_ms = NEW.updated_at * 1000
    WHERE id = NEW.id;
END;

CREATE INDEX idx_threads_created_at_ms ON threads(created_at_ms DESC, id DESC);
CREATE INDEX idx_threads_updated_at_ms ON threads(updated_at_ms DESC, id DESC);
