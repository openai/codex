ALTER TABLE agent_jobs
ADD COLUMN kind TEXT NOT NULL DEFAULT 'csv_batch';

ALTER TABLE agent_jobs
ADD COLUMN max_items INTEGER;

ALTER TABLE agent_job_items
ADD COLUMN parent_item_id TEXT;

ALTER TABLE agent_job_items
ADD COLUMN dedupe_key TEXT;

CREATE INDEX idx_agent_job_items_row_index
    ON agent_job_items(job_id, row_index ASC);

CREATE INDEX idx_agent_job_items_parent
    ON agent_job_items(job_id, parent_item_id, row_index ASC);

CREATE UNIQUE INDEX idx_agent_job_items_dedupe_key
    ON agent_job_items(job_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
