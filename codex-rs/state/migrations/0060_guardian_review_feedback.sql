CREATE TABLE guardian_review_feedback (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    record BLOB NOT NULL
);
