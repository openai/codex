CREATE TABLE thread_history_fork_bases (
    child_thread_id TEXT PRIMARY KEY,
    parent_thread_id TEXT NOT NULL,
    parent_start_thread_id TEXT,
    parent_start_ordinal INTEGER,
    parent_end_thread_id TEXT NOT NULL,
    parent_end_ordinal INTEGER NOT NULL,
    fork_depth INTEGER NOT NULL,
    inherited_history_view TEXT NOT NULL,
    boundary_turn_snapshot_json TEXT
);

CREATE INDEX idx_thread_history_fork_bases_parent
    ON thread_history_fork_bases(parent_thread_id);
