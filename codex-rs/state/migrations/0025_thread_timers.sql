CREATE TABLE thread_timers (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    source TEXT NOT NULL,
    client_id TEXT NOT NULL,
    trigger_json TEXT NOT NULL,
    prompt TEXT NOT NULL,
    delivery TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    next_run_at INTEGER,
    last_run_at INTEGER,
    pending_run INTEGER NOT NULL
);

CREATE INDEX idx_thread_timers_thread_created
    ON thread_timers(thread_id, created_at, id);

CREATE INDEX idx_thread_timers_thread_pending
    ON thread_timers(thread_id, pending_run, created_at, id);

CREATE INDEX idx_thread_timers_thread_next_run
    ON thread_timers(thread_id, next_run_at);
