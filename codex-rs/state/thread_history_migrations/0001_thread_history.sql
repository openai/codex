CREATE TABLE thread_turns (
    turn_id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    turn_created_at_ms INTEGER NOT NULL,
    metadata_json TEXT NOT NULL,
    summary_first_user_item_id TEXT,
    summary_final_agent_item_id TEXT
);

CREATE INDEX idx_thread_turns_page
    ON thread_turns(thread_id, turn_created_at_ms, turn_id);

CREATE TABLE thread_items (
    item_id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    item_ordinal INTEGER NOT NULL,
    item_created_at_ms INTEGER NOT NULL,
    materialized_thread_item_json TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_thread_items_page
    ON thread_items(thread_id, item_ordinal);

CREATE INDEX idx_thread_items_by_turn_page
    ON thread_items(thread_id, turn_id, item_ordinal);

CREATE TABLE thread_history_projection_state (
    thread_id TEXT PRIMARY KEY,
    next_rollout_byte_offset INTEGER NOT NULL
);
