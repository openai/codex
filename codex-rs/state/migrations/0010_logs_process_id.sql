ALTER TABLE logs ADD COLUMN process_uuid TEXT;

CREATE INDEX idx_logs_process_uuid ON logs(process_uuid);
