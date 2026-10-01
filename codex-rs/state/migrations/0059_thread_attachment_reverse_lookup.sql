CREATE INDEX idx_thread_attachments_identity_thread
    ON thread_attachments(attachment_type, identity_key, thread_id);
