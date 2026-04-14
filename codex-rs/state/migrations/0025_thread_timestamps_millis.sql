ALTER TABLE threads ADD COLUMN created_at_ms INTEGER;
ALTER TABLE threads ADD COLUMN updated_at_ms INTEGER;

CREATE TEMP TABLE thread_timestamp_migration AS
SELECT
    id,
    CASE
        WHEN created_at < 1577836800000 THEN created_at * 1000
        ELSE created_at
    END AS created_at_base_ms,
    CASE
        WHEN updated_at < 1577836800000 THEN updated_at * 1000
        ELSE updated_at
    END AS updated_at_base_ms
FROM threads;

WITH RECURSIVE
ordered_created AS (
    SELECT
        id,
        created_at_base_ms,
        ROW_NUMBER() OVER (ORDER BY created_at_base_ms, id) AS row_number
    FROM thread_timestamp_migration
),
assigned_created(row_number, id, created_at_ms) AS (
    SELECT row_number, id, created_at_base_ms
    FROM ordered_created
    WHERE row_number = 1
    UNION ALL
    SELECT
        ordered_created.row_number,
        ordered_created.id,
        MAX(ordered_created.created_at_base_ms, assigned_created.created_at_ms + 1)
    FROM ordered_created
    JOIN assigned_created ON ordered_created.row_number = assigned_created.row_number + 1
)
UPDATE threads
SET created_at_ms = (
    SELECT created_at_ms
    FROM assigned_created
    WHERE assigned_created.id = threads.id
);

WITH RECURSIVE
ordered_updated AS (
    SELECT
        id,
        updated_at_base_ms,
        ROW_NUMBER() OVER (ORDER BY updated_at_base_ms, id) AS row_number
    FROM thread_timestamp_migration
),
assigned_updated(row_number, id, updated_at_ms) AS (
    SELECT row_number, id, updated_at_base_ms
    FROM ordered_updated
    WHERE row_number = 1
    UNION ALL
    SELECT
        ordered_updated.row_number,
        ordered_updated.id,
        MAX(ordered_updated.updated_at_base_ms, assigned_updated.updated_at_ms + 1)
    FROM ordered_updated
    JOIN assigned_updated ON ordered_updated.row_number = assigned_updated.row_number + 1
)
UPDATE threads
SET updated_at_ms = (
    SELECT updated_at_ms
    FROM assigned_updated
    WHERE assigned_updated.id = threads.id
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
