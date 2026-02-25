ALTER TABLE threads ADD COLUMN first_user_message TEXT NOT NULL DEFAULT '';

UPDATE threads
SET first_user_message = title
WHERE first_user_message = '' AND has_user_event = 1 AND title <> '';
