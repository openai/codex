use super::default_output_path;
use super::process_local_analytics;
use codex_analytics::LOCAL_ANALYTICS_SCHEMA_VERSION;
use codex_analytics::LocalAnalyticsRecord;
use codex_analytics::LocalAnalyticsRecordType;
use duckdb::Connection;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

static NEXT_TEST_PATH_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn default_output_replaces_input_extension() {
    assert_eq!(
        default_output_path("/tmp/local-analytics.jsonl"),
        PathBuf::from("/tmp/local-analytics.duckdb")
    );
}

#[test]
fn materializes_threads_turns_events_responses_and_context_windows() {
    let input = test_path("local-analytics.jsonl");
    let output = test_path("local-analytics.duckdb");
    let records = [
        local_record(
            1,
            Some("session-1"),
            Some("thread-root"),
            None,
            json!({
                "event_type": "codex_thread_initialized",
                "event_params": {
                    "thread_id": "thread-root",
                    "session_id": "session-1",
                    "app_server_client": {
                        "product_client_id": "codex_cli_rs",
                        "client_name": "codex",
                        "client_version": "0.0.0",
                        "rpc_transport": "stdio",
                        "experimental_api_enabled": false
                    },
                    "runtime": {
                        "codex_rs_version": "0.0.0",
                        "runtime_os": "macos",
                        "runtime_os_version": "14",
                        "runtime_arch": "aarch64"
                    },
                    "model": "gpt-5",
                    "ephemeral": false,
                    "thread_source": "cli",
                    "initialization_mode": "new",
                    "subagent_source": null,
                    "parent_thread_id": null,
                    "forked_from_thread_id": null,
                    "created_at": 10
                }
            }),
        ),
        local_record(
            2,
            Some("session-1"),
            Some("thread-child"),
            None,
            json!({
                "event_type": "codex_thread_initialized",
                "event_params": {
                    "thread_id": "thread-child",
                    "session_id": "session-1",
                    "app_server_client": {
                        "product_client_id": "codex_cli_rs",
                        "client_name": "codex",
                        "client_version": "0.0.0",
                        "rpc_transport": "stdio",
                        "experimental_api_enabled": false
                    },
                    "runtime": {
                        "codex_rs_version": "0.0.0",
                        "runtime_os": "macos",
                        "runtime_os_version": "14",
                        "runtime_arch": "aarch64"
                    },
                    "model": "gpt-5",
                    "ephemeral": false,
                    "thread_source": "sub_agent",
                    "initialization_mode": "forked",
                    "subagent_source": "review",
                    "parent_thread_id": "thread-root",
                    "forked_from_thread_id": "thread-root",
                    "created_at": 11
                }
            }),
        ),
        local_record(
            3,
            Some("session-1"),
            Some("thread-child"),
            Some("turn-1"),
            turn_event_payload(),
        ),
        local_record(
            4,
            None,
            Some("thread-child"),
            Some("turn-1"),
            json!({
                "event_type": "codex_command_execution_event",
                "event_params": {
                    "thread_id": "thread-child",
                    "turn_id": "turn-1",
                    "review_count": 2,
                    "guardian_review_count": 1,
                    "user_review_count": 1,
                    "terminal_status": "failed",
                    "requested_network_access": true,
                    "requested_additional_permissions": false,
                    "duration_ms": 7
                }
            }),
        ),
        local_record(
            5,
            None,
            Some("thread-child"),
            Some("turn-1"),
            json!({
                "event_type": "codex_compaction_event",
                "event_params": {
                    "thread_id": "thread-child",
                    "turn_id": "turn-1",
                    "status": "completed",
                    "error": null
                }
            }),
        ),
        responses_record(
            6,
            json!({
                "responses_call_id": "call-1",
                "context_window_id": "thread-child:0",
                "transport": "http",
                "status": "completed",
                "request_started_at_epoch_millis": 100,
                "completed_at_epoch_millis": 110,
                "response_id": "response-1",
                "upstream_request_id": "request-1",
                "request_json": {
                    "model": "gpt-5",
                    "instructions": "Be concise.",
                    "input": [{
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "hello"}]
                    }]
                },
                "response_json": {"output_items": []},
                "token_usage_json": {"total_tokens": 3},
                "error_json": null
            }),
        ),
    ];
    write_jsonl(&input, &records);

    process_local_analytics(&input, &output).expect("materialization should succeed");

    let connection = Connection::open(&output).expect("DuckDB should open");
    let child_thread: (String, bool) = connection
        .query_row(
            "SELECT root_thread_id, is_root FROM viewer_threads_v1 WHERE thread_id = 'thread-child'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("child thread should exist");
    assert_eq!(child_thread, ("thread-root".to_string(), false));
    let turn: (i64, i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT turn_ordinal, compactions_count, tool_calls_count, tool_calls_failure_count, responses_api_calls_total_count, responses_api_calls_succeeded_count, responses_api_calls_total_latency_ms FROM viewer_turns_v1 WHERE turn_id = 'turn-1'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("turn should exist");
    assert_eq!(turn, (1, 1, 1, 1, 1, 1, 10));
    let turn_event: (String, i64) = connection
        .query_row(
            "SELECT session_id, event_seq FROM viewer_turn_events_v1 WHERE event_type = 'codex_command_execution_event'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("tool event should exist");
    assert_eq!(turn_event, ("session-1".to_string(), 2));
    let response_call: (i64, String, String) = connection
        .query_row(
            "SELECT call_ordinal, context_window_id, request_json FROM viewer_responses_calls_v1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("responses call should exist");
    assert_eq!(response_call.0, 1);
    assert_eq!(response_call.1, "thread-child:0");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&response_call.2)
            .expect("request json should parse"),
        json!({
            "model": "gpt-5",
            "instructions": "Be concise.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            }]
        })
    );
    let context_window: (i64, i64, String, bool, String) = connection
        .query_row(
            "SELECT context_window_ordinal, message_count, conversation_source, is_synthetic, conversation_json FROM viewer_context_windows_v1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("context window should exist");
    assert_eq!(
        (
            context_window.0,
            context_window.1,
            context_window.2,
            context_window.3
        ),
        (1, 2, "synthetic_local_responses_request".to_string(), true)
    );
    let conversation: serde_json::Value =
        serde_json::from_str(&context_window.4).expect("conversation should parse");
    assert_eq!(
        conversation_author_roles(&conversation),
        vec!["developer".to_string(), "user".to_string()]
    );
}

#[test]
fn materializes_incremental_responses_prompt_in_context_window() {
    let input = test_path("incremental-local-analytics.jsonl");
    let output = test_path("incremental-local-analytics.duckdb");
    let records = [
        responses_record(
            1,
            json!({
                "responses_call_id": "call-1",
                "context_window_id": "thread-child:0",
                "transport": "websocket",
                "status": "completed",
                "request_started_at_epoch_millis": 100,
                "completed_at_epoch_millis": 110,
                "response_id": "response-1",
                "upstream_request_id": "request-1",
                "request_json": {
                    "type": "response.create",
                    "model": "gpt-5",
                    "input": [{
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "hello"}]
                    }]
                },
                "response_json": {
                    "output_items": [{
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": "hi"}],
                        "phase": "final_answer"
                    }]
                },
                "token_usage_json": null,
                "error_json": null
            }),
        ),
        responses_record(
            2,
            json!({
                "responses_call_id": "call-2",
                "context_window_id": "thread-child:0",
                "transport": "websocket",
                "status": "completed",
                "request_started_at_epoch_millis": 120,
                "completed_at_epoch_millis": 130,
                "response_id": "response-2",
                "upstream_request_id": "request-2",
                "request_json": {
                    "type": "response.create",
                    "model": "gpt-5",
                    "previous_response_id": "response-1",
                    "input": [{
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "next"}]
                    }]
                },
                "response_json": {"output_items": []},
                "token_usage_json": null,
                "error_json": null
            }),
        ),
    ];
    write_jsonl(&input, &records);

    process_local_analytics(&input, &output).expect("materialization should succeed");

    let connection = Connection::open(&output).expect("DuckDB should open");
    let (message_count, conversation_json): (i64, String) = connection
        .query_row(
            "SELECT message_count, conversation_json FROM viewer_context_windows_v1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("context window should exist");
    assert_eq!(message_count, 3);
    let conversation: serde_json::Value =
        serde_json::from_str(&conversation_json).expect("conversation should parse");
    assert_eq!(
        conversation_message_texts(&conversation),
        vec!["hello".to_string(), "hi".to_string(), "next".to_string()]
    );
}

#[test]
fn closes_previous_context_window_when_new_codex_window_opens() {
    let input = test_path("compacted-local-analytics.jsonl");
    let output = test_path("compacted-local-analytics.duckdb");
    let records = [
        responses_record(
            1,
            completed_responses_payload("call-1", "thread-child:0", 100, "response-1"),
        ),
        responses_record(
            2,
            completed_responses_payload("call-2", "thread-child:1", 200, "response-2"),
        ),
    ];
    write_jsonl(&input, &records);

    process_local_analytics(&input, &output).expect("materialization should succeed");

    let connection = Connection::open(&output).expect("DuckDB should open");
    let mut statement = connection
        .prepare(
            "SELECT context_window_id, context_window_ordinal, closed_at_epoch_millis, close_reason FROM viewer_context_windows_v1 ORDER BY context_window_ordinal",
        )
        .expect("query should prepare");
    let context_windows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .expect("query should run")
        .collect::<duckdb::Result<Vec<_>>>()
        .expect("rows should collect");
    assert_eq!(
        context_windows,
        vec![
            (
                "thread-child:0".to_string(),
                1,
                Some(200),
                Some("compaction".to_string())
            ),
            ("thread-child:1".to_string(), 2, None, None)
        ]
    );
}

#[test]
fn rejects_unknown_incremental_responses_predecessor() {
    let input = test_path("missing-predecessor-local-analytics.jsonl");
    let output = test_path("missing-predecessor-local-analytics.duckdb");
    let records = [responses_record(
        1,
        json!({
            "responses_call_id": "call-1",
            "context_window_id": "thread-child:0",
            "transport": "websocket",
            "status": "completed",
            "request_started_at_epoch_millis": 100,
            "completed_at_epoch_millis": 110,
            "response_id": "response-1",
            "upstream_request_id": "request-1",
            "request_json": {
                "type": "response.create",
                "model": "gpt-5",
                "previous_response_id": "missing-response",
                "input": []
            },
            "response_json": {"output_items": []},
            "token_usage_json": null,
            "error_json": null
        }),
    )];
    write_jsonl(&input, &records);

    let err = process_local_analytics(&input, &output)
        .expect_err("unknown predecessor should fail materialization");
    assert!(
        err.to_string()
            .contains("unknown incremental Responses predecessor missing-response")
    );
}

fn turn_event_payload() -> serde_json::Value {
    json!({
        "event_type": "codex_turn_event",
        "event_params": {
            "thread_id": "thread-child",
            "session_id": "session-1",
            "turn_id": "turn-1",
            "submission_type": "user",
            "app_server_client": {
                "product_client_id": "codex_cli_rs",
                "client_name": "codex",
                "client_version": "0.0.0",
                "rpc_transport": "stdio",
                "experimental_api_enabled": false
            },
            "runtime": {
                "codex_rs_version": "0.0.0",
                "runtime_os": "macos",
                "runtime_os_version": "14",
                "runtime_arch": "aarch64"
            },
            "ephemeral": false,
            "thread_source": "sub_agent",
            "initialization_mode": "forked",
            "subagent_source": "review",
            "parent_thread_id": "thread-root",
            "model": "gpt-5",
            "model_provider": "openai",
            "sandbox_policy": "workspace_write",
            "reasoning_effort": "medium",
            "reasoning_summary": "auto",
            "service_tier": "default",
            "approval_policy": "on_request",
            "approvals_reviewer": "user",
            "sandbox_network_access": false,
            "collaboration_mode": null,
            "personality": null,
            "workspace_kind": "git",
            "num_input_images": 0,
            "is_first_turn": true,
            "status": "completed",
            "turn_error": null,
            "codex_error_kind": null,
            "codex_error_subreason": null,
            "codex_error_http_status_code": null,
            "steer_count": 0,
            "total_tool_call_count": 1,
            "shell_command_count": 1,
            "file_change_count": 0,
            "mcp_tool_call_count": 0,
            "dynamic_tool_call_count": 0,
            "subagent_tool_call_count": 0,
            "web_search_count": 0,
            "image_generation_count": 0,
            "input_tokens": 1,
            "cached_input_tokens": 0,
            "output_tokens": 2,
            "reasoning_output_tokens": 0,
            "total_tokens": 3,
            "before_first_sampling_ms": 0,
            "sampling_ms": 0,
            "between_sampling_overhead_ms": 0,
            "tool_blocking_ms": 0,
            "after_last_sampling_ms": 0,
            "sampling_request_count": 1,
            "sampling_retry_count": 0,
            "duration_ms": 5,
            "started_at": 12,
            "completed_at": 13
        }
    })
}

fn local_record(
    recorded_at_epoch_millis: u64,
    session_id: Option<&str>,
    thread_id: Option<&str>,
    turn_id: Option<&str>,
    payload: serde_json::Value,
) -> LocalAnalyticsRecord {
    LocalAnalyticsRecord {
        schema_version: LOCAL_ANALYTICS_SCHEMA_VERSION,
        recorded_at_epoch_millis,
        record_type: LocalAnalyticsRecordType::CodexAnalyticsEvent,
        session_id: session_id.map(std::string::ToString::to_string),
        thread_id: thread_id.map(std::string::ToString::to_string),
        turn_id: turn_id.map(std::string::ToString::to_string),
        payload,
    }
}

fn responses_record(
    recorded_at_epoch_millis: u64,
    payload: serde_json::Value,
) -> LocalAnalyticsRecord {
    LocalAnalyticsRecord {
        schema_version: LOCAL_ANALYTICS_SCHEMA_VERSION,
        recorded_at_epoch_millis,
        record_type: LocalAnalyticsRecordType::ResponsesApiCall,
        session_id: Some("session-1".to_string()),
        thread_id: Some("thread-child".to_string()),
        turn_id: Some("turn-1".to_string()),
        payload,
    }
}

fn completed_responses_payload(
    responses_call_id: &str,
    context_window_id: &str,
    request_started_at_epoch_millis: u64,
    response_id: &str,
) -> serde_json::Value {
    json!({
        "responses_call_id": responses_call_id,
        "context_window_id": context_window_id,
        "transport": "http",
        "status": "completed",
        "request_started_at_epoch_millis": request_started_at_epoch_millis,
        "completed_at_epoch_millis": request_started_at_epoch_millis + 10,
        "response_id": response_id,
        "upstream_request_id": null,
        "request_json": {
            "model": "gpt-5",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": context_window_id}]
            }]
        },
        "response_json": {"output_items": []},
        "token_usage_json": null,
        "error_json": null
    })
}

fn conversation_author_roles(conversation: &serde_json::Value) -> Vec<String> {
    conversation["messages"]
        .as_array()
        .expect("conversation should have messages")
        .iter()
        .map(|message| {
            message["author"]["role"]
                .as_str()
                .expect("message should have author role")
                .to_string()
        })
        .collect()
}

fn conversation_message_texts(conversation: &serde_json::Value) -> Vec<String> {
    conversation["messages"]
        .as_array()
        .expect("conversation should have messages")
        .iter()
        .map(|message| {
            message["content"]["parts"][0]
                .as_str()
                .expect("message should have first text part")
                .to_string()
        })
        .collect()
}

fn write_jsonl(path: &PathBuf, records: &[LocalAnalyticsRecord]) {
    let contents = records
        .iter()
        .map(|record| serde_json::to_string(record).expect("record should serialize"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{contents}\n")).expect("fixture should write");
}

fn test_path(file_name: &str) -> PathBuf {
    let test_id = NEXT_TEST_PATH_ID.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    std::env::temp_dir().join(format!(
        "codex-analytics-materializer-{process_id}-{test_id}-{file_name}"
    ))
}
