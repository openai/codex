ALTER TABLE stage1_outputs
ADD COLUMN rollout_summary_filename TEXT;

CREATE UNIQUE INDEX idx_stage1_outputs_rollout_summary_filename
    ON stage1_outputs(rollout_summary_filename)
    WHERE rollout_summary_filename IS NOT NULL;
