ALTER TABLE stage1_outputs
ADD COLUMN selected_for_phase2_source_updated_at INTEGER;
ALTER TABLE threads ADD COLUMN memory_mode TEXT NOT NULL DEFAULT 'enabled';
