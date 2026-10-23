ALTER TABLE threads ADD COLUMN attached_thread_parent_id TEXT;

CREATE INDEX idx_threads_attached_parent ON threads(attached_thread_parent_id)
    WHERE attached_thread_parent_id IS NOT NULL;
