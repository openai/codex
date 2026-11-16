-- Let attached-thread listings seek by both filters, even without planner statistics.
DROP INDEX idx_threads_attached_parent;
CREATE INDEX idx_threads_attached_parent
    ON threads(attached_thread_parent_id, archived)
    WHERE attached_thread_parent_id IS NOT NULL;

-- Keep the default visible chat listing in timestamp order without scanning subagents.
-- The literal source predicates match the default-source branch of push_list_threads_query.
CREATE INDEX idx_threads_interactive_created_at_ms
    ON threads(archived, created_at_ms DESC, id DESC)
    WHERE archived = 0 AND preview <> '' AND attached_thread_parent_id IS NULL
        AND source IN ('cli', 'vscode', '{"custom":"atlas"}', '{"custom":"chatgpt"}');

CREATE INDEX idx_threads_interactive_updated_at_ms
    ON threads(archived, updated_at_ms DESC, id DESC)
    WHERE archived = 0 AND preview <> '' AND attached_thread_parent_id IS NULL
        AND source IN ('cli', 'vscode', '{"custom":"atlas"}', '{"custom":"chatgpt"}');

CREATE INDEX idx_threads_interactive_recency_at_ms
    ON threads(archived, recency_at_ms DESC, id DESC)
    WHERE archived = 0 AND preview <> '' AND attached_thread_parent_id IS NULL
        AND source IN ('cli', 'vscode', '{"custom":"atlas"}', '{"custom":"chatgpt"}');
