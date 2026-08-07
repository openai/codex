CREATE TABLE rollout_migration_state (
    migration_id TEXT PRIMARY KEY,
    last_checked_thread_created_at INTEGER,
    last_checked_thread_id TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE rollout_migration_skipped_rollouts (
    migration_id TEXT NOT NULL,
    rollout_path TEXT NOT NULL,
    rollout_size_bytes INTEGER NOT NULL,
    rollout_modified_at_ns INTEGER NOT NULL,
    skip_reason TEXT NOT NULL,
    skipped_at INTEGER NOT NULL,
    PRIMARY KEY (migration_id, rollout_path)
);
