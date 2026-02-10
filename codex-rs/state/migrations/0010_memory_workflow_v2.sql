DROP TABLE IF EXISTS thread_memory;
DROP TABLE IF EXISTS memory_consolidation_locks;
DROP TABLE IF EXISTS memory_phase1_jobs;
DROP TABLE IF EXISTS memory_scope_dirty;
DROP TABLE IF EXISTS memory_phase2_jobs;

CREATE TABLE thread_memory (
    thread_id TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    raw_memory TEXT NOT NULL,
    memory_summary TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER,
    used_count INTEGER NOT NULL DEFAULT 0,
    invalidated_at INTEGER,
    invalid_reason TEXT,
    PRIMARY KEY (thread_id, scope_kind, scope_key),
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_thread_memory_scope_last_used_at
    ON thread_memory(scope_kind, scope_key, last_used_at DESC, thread_id DESC);
CREATE INDEX idx_thread_memory_scope_updated_at
    ON thread_memory(scope_kind, scope_key, updated_at DESC, thread_id DESC);

CREATE TABLE memory_phase1_jobs (
    thread_id TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    status TEXT NOT NULL,
    owner_session_id TEXT,
    started_at INTEGER,
    finished_at INTEGER,
    failure_reason TEXT,
    source_updated_at INTEGER NOT NULL,
    raw_memory_path TEXT,
    summary_hash TEXT,
    ownership_token TEXT,
    PRIMARY KEY (thread_id, scope_kind, scope_key),
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_memory_phase1_jobs_status_started_at
    ON memory_phase1_jobs(status, started_at DESC);
CREATE INDEX idx_memory_phase1_jobs_scope
    ON memory_phase1_jobs(scope_kind, scope_key);

CREATE TABLE memory_scope_dirty (
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    dirty INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope_kind, scope_key)
);

CREATE INDEX idx_memory_scope_dirty_dirty
    ON memory_scope_dirty(dirty, updated_at DESC);

CREATE TABLE memory_phase2_jobs (
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    status TEXT NOT NULL,
    owner_session_id TEXT,
    agent_thread_id TEXT,
    started_at INTEGER,
    last_heartbeat_at INTEGER,
    finished_at INTEGER,
    attempt INTEGER NOT NULL DEFAULT 0,
    failure_reason TEXT,
    ownership_token TEXT,
    PRIMARY KEY (scope_kind, scope_key)
);

CREATE INDEX idx_memory_phase2_jobs_status_heartbeat
    ON memory_phase2_jobs(status, last_heartbeat_at DESC);

CREATE TABLE memory_consolidation_locks (
    cwd TEXT PRIMARY KEY,
    working_thread_id TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memory_consolidation_locks_updated_at
    ON memory_consolidation_locks(updated_at DESC);
