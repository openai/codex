ALTER TABLE threads ADD COLUMN section_position INTEGER;
ALTER TABLE threads ADD COLUMN section_entered_at_ms INTEGER;

UPDATE threads
SET section_position = ranked.position,
    section_entered_at_ms = threads.recency_at_ms
FROM (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY thread_section_id
               ORDER BY recency_at_ms DESC, id DESC
           ) * 1000000 AS position
    FROM threads
    WHERE thread_section_id IS NOT NULL
) AS ranked
WHERE threads.id = ranked.id;

CREATE INDEX idx_threads_section_position
    ON threads(archived, thread_section_id, section_position ASC, id ASC)
    WHERE thread_section_id IS NOT NULL AND preview <> '';
