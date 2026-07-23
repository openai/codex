ALTER TABLE thread_items ADD COLUMN updated_at_ordinal INTEGER NOT NULL DEFAULT 0;

-- As of this migration, existing projected items originate from exactly one ItemCompleted rollout
-- event, so their creation ordinal is also their update ordinal.
UPDATE thread_items
SET updated_at_ordinal = rollout_ordinal;

-- Older writers can still append items with the zero default after this migration. Incremental
-- replay excludes those items until a newer writer projects an update for them.
-- Keep this index non-unique so mixed-version writers can continue to persist history.
CREATE INDEX idx_thread_items_updated_page
    ON thread_items(thread_id, updated_at_ordinal);

CREATE INDEX idx_thread_items_by_turn_updated_page
    ON thread_items(thread_id, turn_id, updated_at_ordinal);
