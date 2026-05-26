CREATE TABLE security_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at INTEGER NOT NULL,
    kind TEXT NOT NULL,
    thread_id TEXT,
    turn_id TEXT,
    call_id TEXT,
    tool_name TEXT,
    resource TEXT,
    sandbox_type TEXT,
    reason TEXT,
    path TEXT,
    host TEXT,
    port INTEGER,
    protocol TEXT,
    method TEXT,
    decision TEXT,
    source TEXT,
    review_id TEXT,
    reviewer TEXT,
    review_decision TEXT,
    details_json TEXT
);

CREATE INDEX idx_security_events_thread_created
    ON security_events(thread_id, created_at DESC, id DESC);

CREATE INDEX idx_security_events_kind_created
    ON security_events(kind, created_at DESC, id DESC);
