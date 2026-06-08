#![recursion_limit = "256"]

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_analytics::LOCAL_ANALYTICS_SCHEMA_VERSION;
use codex_analytics::LocalAnalyticsRecord;
use codex_analytics::LocalAnalyticsRecordType;
use duckdb::Connection;
use duckdb::params_from_iter;
use duckdb::types::Value as DuckValue;
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

const MATERIALIZE_SQL: &str = r#"
CREATE TEMP VIEW codex_events AS
SELECT
    *,
    json_extract_string(payload, '$.event_type') AS event_type
FROM local_records
WHERE record_type = 'codex_analytics_event';

CREATE TABLE viewer_threads_v1 AS
WITH RECURSIVE thread_base AS (
    SELECT
        json_extract_string(payload, '$.event_params.session_id') AS session_id,
        json_extract_string(payload, '$.event_params.thread_id') AS thread_id,
        json_extract_string(payload, '$.event_params.parent_thread_id') AS parent_thread_id,
        json_extract_string(payload, '$.event_params.forked_from_thread_id') AS forked_from_thread_id,
        json_extract_string(payload, '$.event_params.thread_source') AS thread_source,
        json_extract_string(payload, '$.event_params.subagent_source') AS subagent_source,
        json_extract_string(payload, '$.event_params.initialization_mode') AS initialization_mode,
        TRY_CAST(json_extract_string(payload, '$.event_params.created_at') AS BIGINT) AS thread_created_at_epoch_seconds,
        json_extract_string(payload, '$.event_params.model') AS model,
        json_extract_string(payload, '$.event_params.app_server_client.product_client_id') AS product_client_id,
        json_extract_string(payload, '$.event_params.app_server_client.client_name') AS client_name,
        json_extract_string(payload, '$.event_params.app_server_client.client_version') AS client_version,
        json_extract_string(payload, '$.event_params.app_server_client.rpc_transport') AS rpc_transport,
        TRY_CAST(json_extract_string(payload, '$.event_params.app_server_client.experimental_api_enabled') AS BOOLEAN) AS experimental_api_enabled,
        json_extract_string(payload, '$.event_params.runtime.codex_rs_version') AS codex_rs_version,
        json_extract_string(payload, '$.event_params.runtime.runtime_os') AS runtime_os,
        json_extract_string(payload, '$.event_params.runtime.runtime_os_version') AS runtime_os_version,
        json_extract_string(payload, '$.event_params.runtime.runtime_arch') AS runtime_arch,
        TRY_CAST(json_extract_string(payload, '$.event_params.ephemeral') AS BOOLEAN) AS ephemeral
    FROM codex_events
    WHERE event_type = 'codex_thread_initialized'
    QUALIFY row_number() OVER (
        PARTITION BY
            json_extract_string(payload, '$.event_params.session_id'),
            json_extract_string(payload, '$.event_params.thread_id')
        ORDER BY sink_line_number DESC
    ) = 1
),
thread_ancestors AS (
    SELECT session_id, thread_id, thread_id AS root_thread_id, parent_thread_id AS next_parent_thread_id, 0 AS depth
    FROM thread_base
    UNION ALL
    SELECT ancestors.session_id, ancestors.thread_id, parent.thread_id, parent.parent_thread_id, ancestors.depth + 1
    FROM thread_ancestors ancestors
    JOIN thread_base parent
      ON parent.session_id = ancestors.session_id
     AND parent.thread_id = ancestors.next_parent_thread_id
    WHERE ancestors.depth < 100
),
thread_roots AS (
    SELECT session_id, thread_id, root_thread_id
    FROM thread_ancestors
    QUALIFY row_number() OVER (PARTITION BY session_id, thread_id ORDER BY depth DESC) = 1
)
SELECT
    base.session_id,
    base.thread_id,
    roots.root_thread_id,
    base.parent_thread_id,
    base.forked_from_thread_id,
    base.thread_source,
    base.subagent_source,
    base.initialization_mode,
    base.thread_created_at_epoch_seconds,
    base.model,
    base.product_client_id,
    base.client_name,
    base.client_version,
    base.rpc_transport,
    base.experimental_api_enabled,
    base.codex_rs_version,
    base.runtime_os,
    base.runtime_os_version,
    base.runtime_arch,
    base.ephemeral,
    base.parent_thread_id IS NULL AS is_root
FROM thread_base base
JOIN thread_roots roots USING (session_id, thread_id);

CREATE TABLE viewer_responses_calls_v1 AS
SELECT
    session_id,
    thread_id,
    turn_id,
    json_extract_string(payload, '$.responses_call_id') AS responses_call_id,
    row_number() OVER (
        PARTITION BY session_id, thread_id, turn_id
        ORDER BY TRY_CAST(json_extract_string(payload, '$.request_started_at_epoch_millis') AS BIGINT), json_extract_string(payload, '$.responses_call_id')
    ) AS call_ordinal,
    json_extract_string(payload, '$.transport') AS transport,
    json_extract_string(payload, '$.status') AS status,
    TRY_CAST(json_extract_string(payload, '$.request_started_at_epoch_millis') AS BIGINT) AS request_started_at_epoch_millis,
    TRY_CAST(json_extract_string(payload, '$.completed_at_epoch_millis') AS BIGINT) AS completed_at_epoch_millis,
    json_extract_string(payload, '$.response_id') AS response_id,
    json_extract_string(payload, '$.upstream_request_id') AS upstream_request_id,
    CAST(json_extract(payload, '$.request_json') AS VARCHAR) AS request_json,
    NULLIF(CAST(json_extract(payload, '$.response_json') AS VARCHAR), 'null') AS response_json,
    NULLIF(CAST(json_extract(payload, '$.token_usage_json') AS VARCHAR), 'null') AS token_usage_json,
    NULLIF(CAST(json_extract(payload, '$.error_json') AS VARCHAR), 'null') AS error_json
FROM local_records
WHERE record_type = 'responses_api_call';

CREATE TEMP VIEW viewer_turn_base AS
SELECT
    json_extract_string(payload, '$.event_params.session_id') AS session_id,
    json_extract_string(payload, '$.event_params.thread_id') AS thread_id,
    json_extract_string(payload, '$.event_params.turn_id') AS turn_id,
    json_extract_string(payload, '$.event_params.parent_thread_id') AS parent_thread_id,
    json_extract_string(payload, '$.event_params.thread_source') AS thread_source,
    json_extract_string(payload, '$.event_params.subagent_source') AS subagent_source,
    json_extract_string(payload, '$.event_params.app_server_client.product_client_id') AS product_client_id,
    json_extract_string(payload, '$.event_params.app_server_client.client_name') AS client_name,
    json_extract_string(payload, '$.event_params.app_server_client.client_version') AS client_version,
    json_extract_string(payload, '$.event_params.app_server_client.rpc_transport') AS rpc_transport,
    TRY_CAST(json_extract_string(payload, '$.event_params.app_server_client.experimental_api_enabled') AS BOOLEAN) AS experimental_api_enabled,
    json_extract_string(payload, '$.event_params.runtime.codex_rs_version') AS codex_rs_version,
    json_extract_string(payload, '$.event_params.runtime.runtime_os') AS runtime_os,
    json_extract_string(payload, '$.event_params.runtime.runtime_os_version') AS runtime_os_version,
    json_extract_string(payload, '$.event_params.runtime.runtime_arch') AS runtime_arch,
    json_extract_string(payload, '$.event_params.model_provider') AS model_provider,
    json_extract_string(payload, '$.event_params.service_tier') AS service_tier,
    json_extract_string(payload, '$.event_params.approval_policy') AS approval_policy,
    json_extract_string(payload, '$.event_params.approvals_reviewer') AS approvals_reviewer,
    TRY_CAST(json_extract_string(payload, '$.event_params.sandbox_network_access') AS BOOLEAN) AS sandbox_network_access,
    TRY_CAST(json_extract_string(payload, '$.event_params.num_input_images') AS BIGINT) AS num_input_images,
    TRY_CAST(json_extract_string(payload, '$.event_params.is_first_turn') AS BOOLEAN) AS is_first_turn,
    TRY_CAST(json_extract_string(payload, '$.event_params.ephemeral') AS BOOLEAN) AS ephemeral,
    json_extract_string(payload, '$.event_params.initialization_mode') AS initialization_mode,
    json_extract_string(payload, '$.event_params.workspace_kind') AS workspace_kind,
    json_extract_string(payload, '$.event_params.submission_type') AS submission_type,
    json_extract_string(payload, '$.event_params.model') AS model,
    json_extract_string(payload, '$.event_params.sandbox_policy') AS sandbox_policy,
    json_extract_string(payload, '$.event_params.reasoning_effort') AS reasoning_effort,
    json_extract_string(payload, '$.event_params.reasoning_summary') AS reasoning_summary,
    json_extract_string(payload, '$.event_params.collaboration_mode') AS collaboration_mode,
    json_extract_string(payload, '$.event_params.personality') AS personality,
    json_extract_string(payload, '$.event_params.status') AS status,
    NULLIF(CAST(json_extract(payload, '$.event_params.turn_error') AS VARCHAR), 'null') AS turn_error,
    TRY_CAST(json_extract_string(payload, '$.event_params.steer_count') AS BIGINT) AS steer_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.total_tool_call_count') AS BIGINT) AS total_tool_call_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.shell_command_count') AS BIGINT) AS shell_command_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.file_change_count') AS BIGINT) AS file_change_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.mcp_tool_call_count') AS BIGINT) AS mcp_tool_call_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.dynamic_tool_call_count') AS BIGINT) AS dynamic_tool_call_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.subagent_tool_call_count') AS BIGINT) AS subagent_tool_call_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.web_search_count') AS BIGINT) AS web_search_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.image_generation_count') AS BIGINT) AS image_generation_count,
    TRY_CAST(json_extract_string(payload, '$.event_params.input_tokens') AS BIGINT) AS input_tokens,
    TRY_CAST(json_extract_string(payload, '$.event_params.cached_input_tokens') AS BIGINT) AS cached_input_tokens,
    TRY_CAST(json_extract_string(payload, '$.event_params.output_tokens') AS BIGINT) AS output_tokens,
    TRY_CAST(json_extract_string(payload, '$.event_params.reasoning_output_tokens') AS BIGINT) AS reasoning_output_tokens,
    TRY_CAST(json_extract_string(payload, '$.event_params.total_tokens') AS BIGINT) AS total_tokens,
    TRY_CAST(json_extract_string(payload, '$.event_params.duration_ms') AS BIGINT) AS duration_ms,
    TRY_CAST(json_extract_string(payload, '$.event_params.started_at') AS BIGINT) AS started_at_epoch_seconds,
    TRY_CAST(json_extract_string(payload, '$.event_params.completed_at') AS BIGINT) AS completed_at_epoch_seconds
FROM codex_events
WHERE event_type = 'codex_turn_event'
QUALIFY row_number() OVER (
    PARTITION BY
        json_extract_string(payload, '$.event_params.session_id'),
        json_extract_string(payload, '$.event_params.thread_id'),
        json_extract_string(payload, '$.event_params.turn_id')
    ORDER BY sink_line_number DESC
) = 1;

CREATE TEMP VIEW viewer_turn_tool_aggregates AS
SELECT
    thread_id,
    turn_id,
    count(*) AS tool_calls_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.review_count') AS BIGINT)), 0) AS tool_calls_total_review_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.guardian_review_count') AS BIGINT)), 0) AS tool_calls_guardian_review_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.user_review_count') AS BIGINT)), 0) AS tool_calls_user_review_count,
    sum(CASE WHEN json_extract_string(payload, '$.event_params.terminal_status') = 'failed' THEN 1 ELSE 0 END) AS tool_calls_failure_count,
    sum(CASE WHEN json_extract_string(payload, '$.event_params.requested_network_access') = 'true' THEN 1 ELSE 0 END) AS tool_calls_requested_network_access_count,
    sum(CASE WHEN json_extract_string(payload, '$.event_params.requested_additional_permissions') = 'true' THEN 1 ELSE 0 END) AS tool_calls_requested_additional_permissions_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.duration_ms') AS BIGINT)), 0) AS tool_calls_total_duration_ms
FROM codex_events
WHERE event_type IN ('codex_command_execution', 'codex_file_change', 'codex_mcp_tool_call', 'codex_dynamic_tool_call', 'codex_collab_agent_tool_call', 'codex_web_search', 'codex_image_generation')
GROUP BY thread_id, turn_id;

CREATE TEMP VIEW viewer_turn_compaction_aggregates AS
SELECT
    thread_id,
    turn_id,
    count(*) AS compactions_count,
    sum(CASE WHEN json_extract_string(payload, '$.event_params.status') = 'completed' THEN 1 ELSE 0 END) AS compactions_completed_count,
    sum(CASE WHEN json_extract_string(payload, '$.event_params.status') = 'failed' THEN 1 ELSE 0 END) AS compactions_failed_count,
    bool_or(NULLIF(CAST(json_extract(payload, '$.event_params.error') AS VARCHAR), 'null') IS NOT NULL) AS compactions_any_error
FROM codex_events
WHERE event_type = 'codex_compaction'
GROUP BY thread_id, turn_id;

CREATE TEMP VIEW viewer_turn_review_aggregates AS
SELECT
    thread_id,
    turn_id,
    count(*) AS reviews_event_count
FROM codex_events
WHERE event_type = 'codex_review_event'
GROUP BY thread_id, turn_id;

CREATE TEMP VIEW viewer_turn_guardian_review_aggregates AS
SELECT
    thread_id,
    turn_id,
    count(*) AS guardian_reviews_event_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.tool_call_count') AS BIGINT)), 0) AS guardian_reviews_tool_call_count,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.completion_latency_ms') AS BIGINT)), 0) AS guardian_reviews_total_completion_latency_ms,
    coalesce(sum(TRY_CAST(json_extract_string(payload, '$.event_params.total_tokens') AS BIGINT)), 0) AS guardian_reviews_total_tokens
FROM codex_events
WHERE event_type = 'codex_guardian_review'
GROUP BY thread_id, turn_id;

CREATE TEMP VIEW viewer_turn_response_aggregates AS
SELECT
    session_id,
    thread_id,
    turn_id,
    count(*) AS responses_api_calls_total_count,
    sum(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS responses_api_calls_failed_count,
    sum(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS responses_api_calls_succeeded_count,
    coalesce(sum(completed_at_epoch_millis - request_started_at_epoch_millis), 0) AS responses_api_calls_total_latency_ms
FROM viewer_responses_calls_v1
GROUP BY session_id, thread_id, turn_id;

CREATE TABLE viewer_turns_v1 AS
WITH turns_with_ordinal AS (
    SELECT
        *,
        row_number() OVER (
            PARTITION BY session_id, thread_id
            ORDER BY started_at_epoch_seconds NULLS LAST, turn_id
        ) AS turn_ordinal
    FROM viewer_turn_base
)
SELECT
    turns.session_id,
    turns.thread_id,
    turns.turn_id,
    turns.turn_ordinal,
    turns.parent_thread_id,
    turns.thread_source,
    turns.subagent_source,
    turns.product_client_id,
    turns.client_name,
    turns.client_version,
    turns.rpc_transport,
    turns.experimental_api_enabled,
    turns.codex_rs_version,
    turns.runtime_os,
    turns.runtime_os_version,
    turns.runtime_arch,
    turns.model_provider,
    turns.service_tier,
    turns.approval_policy,
    turns.approvals_reviewer,
    turns.sandbox_network_access,
    turns.num_input_images,
    turns.is_first_turn,
    turns.ephemeral,
    turns.initialization_mode,
    turns.workspace_kind,
    turns.submission_type,
    turns.model,
    turns.sandbox_policy,
    turns.reasoning_effort,
    turns.reasoning_summary,
    turns.collaboration_mode,
    turns.personality,
    turns.status,
    turns.turn_error,
    turns.steer_count,
    turns.total_tool_call_count,
    turns.shell_command_count,
    turns.file_change_count,
    turns.mcp_tool_call_count,
    turns.dynamic_tool_call_count,
    turns.subagent_tool_call_count,
    turns.web_search_count,
    turns.image_generation_count,
    turns.input_tokens,
    turns.cached_input_tokens,
    turns.output_tokens,
    turns.reasoning_output_tokens,
    turns.total_tokens,
    turns.duration_ms,
    turns.started_at_epoch_seconds,
    turns.completed_at_epoch_seconds,
    coalesce(compactions.compactions_count, 0) AS compactions_count,
    coalesce(compactions.compactions_completed_count, 0) AS compactions_completed_count,
    coalesce(compactions.compactions_failed_count, 0) AS compactions_failed_count,
    coalesce(compactions.compactions_any_error, false) AS compactions_any_error,
    coalesce(tools.tool_calls_count, 0) AS tool_calls_count,
    coalesce(tools.tool_calls_total_review_count, 0) AS tool_calls_total_review_count,
    coalesce(tools.tool_calls_guardian_review_count, 0) AS tool_calls_guardian_review_count,
    coalesce(tools.tool_calls_user_review_count, 0) AS tool_calls_user_review_count,
    coalesce(tools.tool_calls_failure_count, 0) AS tool_calls_failure_count,
    coalesce(tools.tool_calls_requested_network_access_count, 0) AS tool_calls_requested_network_access_count,
    coalesce(tools.tool_calls_requested_additional_permissions_count, 0) AS tool_calls_requested_additional_permissions_count,
    coalesce(tools.tool_calls_total_duration_ms, 0) AS tool_calls_total_duration_ms,
    coalesce(reviews.reviews_event_count, 0) AS reviews_event_count,
    coalesce(guardian.guardian_reviews_event_count, 0) AS guardian_reviews_event_count,
    coalesce(guardian.guardian_reviews_tool_call_count, 0) AS guardian_reviews_tool_call_count,
    coalesce(guardian.guardian_reviews_total_completion_latency_ms, 0) AS guardian_reviews_total_completion_latency_ms,
    coalesce(guardian.guardian_reviews_total_tokens, 0) AS guardian_reviews_total_tokens,
    coalesce(responses.responses_api_calls_total_count, 0) AS responses_api_calls_total_count,
    coalesce(responses.responses_api_calls_failed_count, 0) AS responses_api_calls_failed_count,
    coalesce(responses.responses_api_calls_succeeded_count, 0) AS responses_api_calls_succeeded_count,
    coalesce(responses.responses_api_calls_total_latency_ms, 0) AS responses_api_calls_total_latency_ms
FROM turns_with_ordinal turns
LEFT JOIN viewer_turn_compaction_aggregates compactions USING (thread_id, turn_id)
LEFT JOIN viewer_turn_tool_aggregates tools USING (thread_id, turn_id)
LEFT JOIN viewer_turn_review_aggregates reviews USING (thread_id, turn_id)
LEFT JOIN viewer_turn_guardian_review_aggregates guardian USING (thread_id, turn_id)
LEFT JOIN viewer_turn_response_aggregates responses USING (session_id, thread_id, turn_id);

CREATE TEMP VIEW unique_thread_sessions AS
SELECT thread_id, min(session_id) AS session_id
FROM viewer_threads_v1
GROUP BY thread_id
HAVING count(DISTINCT session_id) = 1;

CREATE TABLE viewer_turn_events_v1 AS
WITH resolved_events AS (
    SELECT
        coalesce(records.session_id, sessions.session_id) AS session_id,
        records.thread_id,
        records.turn_id,
        records.sink_line_number,
        records.recorded_at_epoch_millis,
        records.record_type,
        records.payload
    FROM local_records records
    LEFT JOIN unique_thread_sessions sessions USING (thread_id)
    WHERE records.turn_id IS NOT NULL
)
SELECT
    session_id,
    thread_id,
    turn_id,
    row_number() OVER (PARTITION BY session_id, thread_id, turn_id ORDER BY sink_line_number) AS event_seq,
    sink_line_number,
    recorded_at_epoch_millis,
    CASE WHEN record_type = 'responses_api_call' THEN 'responses_api' ELSE 'codex_analytics' END AS event_kind,
    CASE WHEN record_type = 'responses_api_call' THEN 'responses_api_call' ELSE json_extract_string(payload, '$.event_type') END AS event_type,
    CASE WHEN record_type = 'responses_api_call' THEN json_extract_string(payload, '$.responses_call_id') END AS responses_call_id,
    CASE WHEN record_type = 'responses_api_call' THEN payload END AS event_summary_json,
    CASE WHEN record_type = 'codex_analytics_event' THEN payload END AS analytics_event_json
FROM resolved_events;

CREATE TABLE viewer_context_windows_v1 (
    session_id VARCHAR,
    thread_id VARCHAR,
    context_window_id VARCHAR,
    context_window_ordinal BIGINT,
    first_turn_id VARCHAR,
    last_turn_id VARCHAR,
    first_responses_call_id VARCHAR,
    last_responses_call_id VARCHAR,
    opened_at_epoch_millis BIGINT,
    closed_at_epoch_millis BIGINT,
    close_reason VARCHAR,
    message_count BIGINT,
    conversation_json VARCHAR,
    conversation_source VARCHAR,
    is_synthetic BOOLEAN
);
"#;

pub fn default_output_path(input: impl AsRef<Path>) -> PathBuf {
    input.as_ref().with_extension("duckdb")
}

pub fn process_local_analytics(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let input = input.as_ref();
    let output = output.as_ref();
    if output.exists() {
        fs::remove_file(output)
            .with_context(|| format!("remove existing DuckDB file {}", output.display()))?;
    }

    let connection = Connection::open(output)
        .with_context(|| format!("open DuckDB file {}", output.display()))?;
    create_raw_records_table(&connection)?;
    insert_local_records(&connection, input)?;
    connection.execute_batch(MATERIALIZE_SQL)?;
    Ok(())
}

fn create_raw_records_table(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE local_records (
            sink_line_number BIGINT,
            schema_version BIGINT,
            recorded_at_epoch_millis BIGINT,
            record_type VARCHAR,
            session_id VARCHAR,
            thread_id VARCHAR,
            turn_id VARCHAR,
            payload VARCHAR
        );
        "#,
    )?;
    Ok(())
}

fn insert_local_records(connection: &Connection, input: &Path) -> Result<()> {
    let file = File::open(input)
        .with_context(|| format!("open local analytics JSONL {}", input.display()))?;
    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.with_context(|| format!("read JSONL line {line_number}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: LocalAnalyticsRecord = serde_json::from_str(&line)
            .with_context(|| format!("parse JSONL line {line_number}"))?;
        if record.schema_version != LOCAL_ANALYTICS_SCHEMA_VERSION {
            bail!(
                "unsupported local analytics schema version {} on line {line_number}",
                record.schema_version
            );
        }
        connection.execute(
            "INSERT INTO local_records VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params_from_iter([
                integer(i64::try_from(line_number)?),
                integer(i64::from(record.schema_version)),
                integer(to_i64(record.recorded_at_epoch_millis)),
                text(record_type(record.record_type)),
                optional_text(record.session_id),
                optional_text(record.thread_id),
                optional_text(record.turn_id),
                text(serde_json::to_string(&record.payload)?),
            ]),
        )?;
    }
    Ok(())
}

fn record_type(record_type: LocalAnalyticsRecordType) -> String {
    match record_type {
        LocalAnalyticsRecordType::CodexAnalyticsEvent => "codex_analytics_event",
        LocalAnalyticsRecordType::ResponsesApiCall => "responses_api_call",
    }
    .to_string()
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn text(value: String) -> DuckValue {
    DuckValue::Text(value)
}

fn optional_text(value: Option<String>) -> DuckValue {
    value.map_or(DuckValue::Null, DuckValue::Text)
}

fn integer(value: i64) -> DuckValue {
    DuckValue::BigInt(value)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
