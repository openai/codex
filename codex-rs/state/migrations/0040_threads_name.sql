ALTER TABLE threads ADD COLUMN name TEXT;
ALTER TABLE threads ADD COLUMN name_state TEXT NOT NULL DEFAULT 'legacy_unknown'
    CHECK (name_state IN ('legacy_unknown', 'unnamed', 'cleared', 'explicit'));
ALTER TABLE threads ADD COLUMN title_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE threads ADD COLUMN title_state TEXT NOT NULL DEFAULT 'legacy_unknown'
    CHECK (title_state IN ('legacy_unknown', 'derived'));

UPDATE threads
SET name = title,
    name_state = 'explicit',
    title_snapshot = title
WHERE title <> ''
  AND (first_user_message = '' OR trim(title) <> trim(first_user_message));

UPDATE threads
SET title_snapshot = title
WHERE name_state = 'legacy_unknown';

CREATE TRIGGER threads_title_snapshot_after_insert
AFTER INSERT ON threads
WHEN NEW.title_snapshot = ''
 AND NEW.title <> ''
 AND NEW.title_state = 'legacy_unknown'
BEGIN
    UPDATE threads
    SET title_snapshot = NEW.title
    WHERE id = NEW.id;
END;
