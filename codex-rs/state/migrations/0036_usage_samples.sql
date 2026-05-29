CREATE TABLE usage_samples (
    sample_id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    turn_id TEXT NOT NULL,
    response_id TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    non_cached_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    blended_tokens INTEGER NOT NULL,
    prompt_estimated_tokens INTEGER NOT NULL
);

CREATE TABLE usage_sample_contributors (
    sample_id TEXT NOT NULL REFERENCES usage_samples(sample_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    contributor_id TEXT NOT NULL,
    label TEXT NOT NULL,
    source_estimated_tokens INTEGER NOT NULL,
    attributed_tokens INTEGER NOT NULL,
    PRIMARY KEY (sample_id, kind, contributor_id)
);

CREATE INDEX idx_usage_samples_occurred_at ON usage_samples(occurred_at);
CREATE INDEX idx_usage_samples_thread_occurred_at ON usage_samples(thread_id, occurred_at);
CREATE INDEX idx_usage_sample_contributors_kind ON usage_sample_contributors(kind, contributor_id);
