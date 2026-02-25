CREATE TABLE backfill_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    status TEXT NOT NULL,
    last_watermark TEXT,
    last_success_at INTEGER,
    updated_at INTEGER NOT NULL
);

INSERT INTO backfill_state (id, status, last_watermark, last_success_at, updated_at)
VALUES (
    1,
    'pending',
    NULL,
    NULL,
    CAST(strftime('%s', 'now') AS INTEGER)
)
ON CONFLICT(id) DO NOTHING;
