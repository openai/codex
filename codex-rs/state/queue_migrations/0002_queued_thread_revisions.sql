CREATE TABLE queued_thread_revisions (
    revision INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id TEXT NOT NULL UNIQUE
);

INSERT INTO queued_thread_revisions (thread_id)
SELECT DISTINCT thread_id FROM queued_items ORDER BY thread_id;

CREATE TRIGGER queued_items_revision_after_insert
AFTER INSERT ON queued_items
BEGIN
    INSERT INTO queued_thread_revisions (thread_id)
    VALUES (NEW.thread_id)
    ON CONFLICT(thread_id) DO UPDATE
    SET revision = (SELECT COALESCE(MAX(revision), 0) + 1 FROM queued_thread_revisions);
END;

CREATE TRIGGER queued_items_revision_after_update
AFTER UPDATE ON queued_items
BEGIN
    INSERT INTO queued_thread_revisions (thread_id)
    VALUES (NEW.thread_id)
    ON CONFLICT(thread_id) DO UPDATE
    SET revision = (SELECT COALESCE(MAX(revision), 0) + 1 FROM queued_thread_revisions);
END;

CREATE TRIGGER queued_items_revision_after_delete
AFTER DELETE ON queued_items
BEGIN
    INSERT INTO queued_thread_revisions (thread_id)
    VALUES (OLD.thread_id)
    ON CONFLICT(thread_id) DO UPDATE
    SET revision = (SELECT COALESCE(MAX(revision), 0) + 1 FROM queued_thread_revisions);
END;
