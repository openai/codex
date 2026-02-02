CREATE TABLE memory_summaries (
    thread_id TEXT PRIMARY KEY,
    cwd TEXT NOT NULL,
    summary TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memory_summaries_cwd_updated_at
    ON memory_summaries(cwd, updated_at DESC, thread_id);

CREATE TABLE memory_summary_locks (
    cwd TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    acquired_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX idx_memory_summary_locks_expires_at
    ON memory_summary_locks(expires_at);
