ALTER TABLE thread_items ADD COLUMN item_type TEXT NOT NULL DEFAULT '';

UPDATE thread_items
SET item_type = json_extract(item_json, '$.type')
WHERE item_type = '';

CREATE INDEX idx_thread_items_user_messages
    ON thread_items(thread_id, rollout_ordinal)
    WHERE item_type = 'userMessage';
