CREATE INDEX idx_thread_turns_end_page
    ON thread_turns(thread_id, rollout_end_ordinal, turn_id)
    WHERE rollout_end_ordinal IS NOT NULL;
