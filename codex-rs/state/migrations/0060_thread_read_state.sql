-- Independent of rebuildable thread metadata: only actual thread deletion removes receipts.
-- NULL means read; the empty string means manually unread from the start, even when empty.
CREATE TABLE thread_read_receipts (
    thread_id TEXT PRIMARY KEY NOT NULL,
    first_unread_turn TEXT,
    revision TEXT NOT NULL,
    last_published_turn TEXT
);
