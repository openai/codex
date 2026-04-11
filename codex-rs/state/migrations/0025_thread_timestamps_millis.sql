CREATE TEMP TABLE thread_timestamp_migration AS
SELECT
    id,
    created_at,
    updated_at,
    archived_at,
    (
        SELECT COUNT(*)
        FROM threads AS prev
        WHERE prev.updated_at = threads.updated_at
          AND prev.id < threads.id
    ) AS updated_at_offset
FROM threads;

UPDATE threads
SET created_at = (
    SELECT created_at * 1000
    FROM thread_timestamp_migration
    WHERE thread_timestamp_migration.id = threads.id
)
WHERE created_at < 1577836800000;

UPDATE threads
SET updated_at = (
    SELECT updated_at * 1000 + updated_at_offset
    FROM thread_timestamp_migration
    WHERE thread_timestamp_migration.id = threads.id
)
WHERE updated_at < 1577836800000;

UPDATE threads
SET archived_at = (
    SELECT archived_at * 1000
    FROM thread_timestamp_migration
    WHERE thread_timestamp_migration.id = threads.id
)
WHERE archived_at IS NOT NULL
  AND archived_at < 1577836800000;

DROP TABLE thread_timestamp_migration;
