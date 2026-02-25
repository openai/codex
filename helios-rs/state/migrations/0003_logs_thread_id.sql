ALTER TABLE logs ADD COLUMN thread_id TEXT;

CREATE INDEX idx_logs_thread_id ON logs(thread_id);
