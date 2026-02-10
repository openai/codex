DROP TABLE IF EXISTS thread_memory;
DROP TABLE IF EXISTS memory_phase1_jobs;
DROP TABLE IF EXISTS memory_scope_dirty;
DROP TABLE IF EXISTS memory_phase2_jobs;
DROP TABLE IF EXISTS memory_consolidation_locks;

CREATE TABLE IF NOT EXISTS stage1_outputs (
    thread_id TEXT PRIMARY KEY,
    source_updated_at INTEGER NOT NULL,
    raw_memory TEXT NOT NULL,
    summary TEXT NOT NULL,
    generated_at INTEGER NOT NULL,
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_stage1_outputs_source_updated_at
    ON stage1_outputs(source_updated_at DESC, thread_id DESC);

CREATE TABLE IF NOT EXISTS jobs (
    kind TEXT NOT NULL,
    job_key TEXT NOT NULL,
    status TEXT NOT NULL,
    worker_id TEXT,
    ownership_token TEXT,
    started_at INTEGER,
    finished_at INTEGER,
    lease_until INTEGER,
    retry_at INTEGER,
    retry_remaining INTEGER NOT NULL,
    last_error TEXT,
    input_watermark INTEGER,
    last_success_watermark INTEGER,
    PRIMARY KEY (kind, job_key)
);

CREATE INDEX IF NOT EXISTS idx_jobs_kind_status_retry_lease
    ON jobs(kind, status, retry_at, lease_until);
