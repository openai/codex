CREATE TABLE thread_queued_turns (
    queued_turn_id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL,
    turn_start_params_jsonb BLOB NOT NULL,
    queue_order INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending', 'dispatching', 'failed')),
    dispatch_turn_id TEXT,
    failure_jsonb BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(thread_id, queue_order)
);

CREATE INDEX thread_queued_turns_thread_state_order_idx
    ON thread_queued_turns(thread_id, state, queue_order);
