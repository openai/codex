CREATE TABLE thread_memory (
    thread_id TEXT PRIMARY KEY,
    trace_summary TEXT NOT NULL,
    memory_summary TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_thread_memory_updated_at ON thread_memory(updated_at DESC, thread_id DESC);
