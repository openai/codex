ALTER TABLE threads ADD COLUMN preview TEXT NOT NULL DEFAULT '';

UPDATE threads
SET preview = first_user_message
WHERE preview = '' AND first_user_message <> '';

UPDATE threads
SET preview = (
    SELECT thread_goals.objective
    FROM thread_goals
    WHERE thread_goals.thread_id = threads.id
)
WHERE preview = ''
    AND EXISTS (
        SELECT 1
        FROM thread_goals
        WHERE thread_goals.thread_id = threads.id
            AND thread_goals.objective <> ''
    );
