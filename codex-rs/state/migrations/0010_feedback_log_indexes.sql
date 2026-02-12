CREATE INDEX idx_logs_feedback_thread
    ON logs(thread_id, id DESC)
    WHERE thread_id IS NOT NULL
      AND message IS NOT NULL;

CREATE INDEX idx_logs_feedback_process_threadless
    ON logs(process_id, id DESC)
    WHERE thread_id IS NULL
      AND message IS NOT NULL;
