CREATE TABLE thread_artifacts (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    identity_key TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (thread_id, artifact_type, identity_key)
);

CREATE INDEX idx_thread_artifacts_thread_created_id
    ON thread_artifacts(thread_id, created_at, id);
