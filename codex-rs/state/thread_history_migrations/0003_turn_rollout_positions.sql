ALTER TABLE thread_turns ADD COLUMN rollout_byte_offset INTEGER;
ALTER TABLE thread_turns ADD COLUMN rollout_end_ordinal INTEGER;
ALTER TABLE thread_turns ADD COLUMN rollout_end_byte_offset INTEGER;
