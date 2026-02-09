CREATE TABLE memory_consolidation_locks (
    cwd TEXT PRIMARY KEY,
    working_thread_id TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memory_consolidation_locks_updated_at
    ON memory_consolidation_locks(updated_at DESC);
