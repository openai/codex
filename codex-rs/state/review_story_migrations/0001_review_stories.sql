CREATE TABLE IF NOT EXISTS review_story_snapshots (
    story_snapshot_id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL,
    step_count INTEGER NOT NULL,
    target_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    previous_story_snapshot_id TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS review_story_snapshots_thread_updated_idx
    ON review_story_snapshots(thread_id, updated_at_ms DESC, story_snapshot_id DESC);

CREATE INDEX IF NOT EXISTS review_story_snapshots_thread_fingerprint_idx
    ON review_story_snapshots(thread_id, source_fingerprint, updated_at_ms DESC);
