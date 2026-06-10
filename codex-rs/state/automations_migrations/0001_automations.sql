CREATE TABLE automations (
    id TEXT PRIMARY KEY,
    owner_thread_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('cron', 'heartbeat')),
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'PAUSED')),
    rrule TEXT NOT NULL,
    next_run_at INTEGER,
    last_run_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    model TEXT,
    reasoning_effort TEXT,
    cron_cwds_json TEXT,
    target_thread_id TEXT,
    dispatch_workspace_roots_json TEXT,
    dispatch_approval_policy_json TEXT,
    dispatch_approvals_reviewer_json TEXT,
    dispatch_permission_profile_json TEXT,
    claimed_by TEXT,
    ownership_token TEXT,
    lease_until INTEGER,
    in_flight_run_at INTEGER,
    in_flight_dispatch_mode TEXT CHECK (in_flight_dispatch_mode IN ('scheduled', 'manual')),
    dispatch_cwd_index INTEGER NOT NULL DEFAULT 0,
    retry_at INTEGER,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error TEXT,
    last_dispatch_succeeded INTEGER,
    last_dispatch_started_at INTEGER,
    last_dispatch_completed_at INTEGER,
    last_dispatch_failed_at INTEGER
);

CREATE INDEX idx_automations_due
ON automations (next_run_at)
WHERE status = 'ACTIVE' AND next_run_at IS NOT NULL;

CREATE INDEX idx_automations_retry_ready
ON automations (retry_at)
WHERE retry_at IS NOT NULL;

CREATE UNIQUE INDEX idx_automations_active_heartbeat_thread
ON automations (target_thread_id)
WHERE kind = 'heartbeat' AND status = 'ACTIVE';
