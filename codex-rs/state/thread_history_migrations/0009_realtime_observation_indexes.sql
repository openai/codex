-- Support time ranges and position catch-up directly on the realtime projection.
CREATE INDEX idx_thread_realtime_observed
    ON thread_realtime_items(thread_id, json_extract(item_json, '$.observed_at_ms'), rollout_ordinal, item_id)
    WHERE json_extract(item_json, '$.observed_at_ms') IS NOT NULL;

CREATE INDEX idx_thread_realtime_session_observed
    ON thread_realtime_items(thread_id, json_extract(item_json, '$.realtime_session_id'), json_extract(item_json, '$.observed_at_ms'), rollout_ordinal, item_id)
    WHERE json_extract(item_json, '$.observed_at_ms') IS NOT NULL;

CREATE INDEX idx_thread_realtime_session_position
    ON thread_realtime_items(thread_id, json_extract(item_json, '$.realtime_session_id'), rollout_ordinal, item_id)
    WHERE json_extract(item_json, '$.observed_at_ms') IS NOT NULL;
