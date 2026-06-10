ALTER TABLE threads
ADD COLUMN approvals_reviewer TEXT NOT NULL DEFAULT 'user';

ALTER TABLE threads
ADD COLUMN runtime_workspace_roots_json TEXT;
