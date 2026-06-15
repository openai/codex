CREATE TABLE queued_items (
    queued_item_id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL,
    payload_jsonb BLOB NOT NULL,
    queue_order INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'claimed', 'failed')),
    claim_token TEXT,
    failure_jsonb BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK ((state = 'claimed') = (claim_token IS NOT NULL))
);

CREATE INDEX queued_items_thread_state_order_idx
    ON queued_items(thread_id, state, queue_order);

CREATE INDEX queued_items_thread_order_idx
    ON queued_items(thread_id, queue_order);
