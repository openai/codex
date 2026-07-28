CREATE TABLE thread_sections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

INSERT INTO thread_sections (id, name)
VALUES ('01984de2-8f74-7c91-a3b2-5c5e937cf318', 'Pinned');

ALTER TABLE threads ADD COLUMN thread_section_id TEXT
    REFERENCES thread_sections(id) ON DELETE SET NULL;

CREATE INDEX idx_threads_section_recency_at_ms
    ON threads(archived, thread_section_id, recency_at_ms DESC, id DESC)
    WHERE thread_section_id IS NOT NULL AND preview <> '';
