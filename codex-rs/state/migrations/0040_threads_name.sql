ALTER TABLE threads ADD COLUMN name TEXT;

UPDATE threads
SET name = title
WHERE title <> ''
  AND (first_user_message = '' OR trim(title) <> trim(first_user_message));
