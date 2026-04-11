ALTER TABLE thread_timers RENAME TO thread_timers_old;

CREATE TABLE thread_timers (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    source TEXT NOT NULL,
    client_id TEXT NOT NULL,
    trigger_json TEXT NOT NULL,
    content TEXT NOT NULL,
    instructions TEXT,
    meta_json TEXT NOT NULL,
    delivery TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    next_run_at INTEGER,
    last_run_at INTEGER,
    pending_run INTEGER NOT NULL
);

INSERT INTO thread_timers (
    id,
    thread_id,
    source,
    client_id,
    trigger_json,
    content,
    instructions,
    meta_json,
    delivery,
    created_at,
    next_run_at,
    last_run_at,
    pending_run
)
SELECT
    id,
    thread_id,
    source,
    client_id,
    trigger_json,
    prompt,
    NULL,
    '{}',
    delivery,
    created_at,
    next_run_at,
    last_run_at,
    pending_run
FROM thread_timers_old;

DROP TABLE thread_timers_old;

CREATE INDEX idx_thread_timers_thread_created
    ON thread_timers(thread_id, created_at, id);

CREATE INDEX idx_thread_timers_thread_pending
    ON thread_timers(thread_id, pending_run, created_at, id);

CREATE INDEX idx_thread_timers_thread_next_run
    ON thread_timers(thread_id, next_run_at);

CREATE TABLE thread_messages (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    thread_id TEXT NOT NULL,
    source TEXT NOT NULL,
    content TEXT NOT NULL,
    instructions TEXT,
    meta_json TEXT NOT NULL,
    delivery TEXT NOT NULL,
    queued_at INTEGER NOT NULL
);

CREATE INDEX thread_messages_thread_order_idx
    ON thread_messages(thread_id, queued_at, seq);

CREATE INDEX thread_messages_thread_delivery_order_idx
    ON thread_messages(thread_id, delivery, queued_at, seq);
