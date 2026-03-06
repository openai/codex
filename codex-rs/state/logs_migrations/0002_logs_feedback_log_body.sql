ALTER TABLE logs ADD COLUMN feedback_log_body TEXT;

UPDATE logs
SET feedback_log_body = message
WHERE message IS NOT NULL;
