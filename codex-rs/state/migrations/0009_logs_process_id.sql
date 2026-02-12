ALTER TABLE logs ADD COLUMN process_id TEXT;

CREATE INDEX idx_logs_process_id ON logs(process_id);
