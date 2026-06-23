ALTER TABLE threads ADD COLUMN name TEXT;
ALTER TABLE threads ADD COLUMN name_state TEXT NOT NULL DEFAULT 'legacy_unknown'
    CHECK (name_state IN ('legacy_unknown', 'unnamed', 'explicit'));
ALTER TABLE threads ADD COLUMN title_snapshot TEXT NOT NULL DEFAULT '';

UPDATE threads
SET name = title,
    name_state = 'explicit',
    title_snapshot = title
WHERE title <> ''
  AND (first_user_message = '' OR trim(title) <> trim(first_user_message));

UPDATE threads
SET title_snapshot = title
WHERE name_state = 'legacy_unknown';
