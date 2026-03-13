use super::node::NodeVersion;
use super::*;
use crate::codex::make_session_and_context;
use crate::codex::make_session_and_context_with_dynamic_tools_and_rx;
use crate::codex::make_session_and_context_with_rx;
use crate::features::Feature;
use crate::protocol::AskForApproval;
use crate::protocol::EventMsg;
use crate::protocol::SandboxPolicy;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::InputModality;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn node_version_parses_v_prefix_and_suffix() {
    let version = NodeVersion::parse("v25.1.0-nightly.2024").unwrap();
    assert_eq!(
        version,
        NodeVersion {
            major: 25,
            minor: 1,
            patch: 0,
        }
    );
}

#[test]
fn clamp_poll_ms_defaults_to_background_window() {
    assert_eq!(
        clamp_poll_ms(None),
        crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS
    );
    assert_eq!(
        clamp_poll_ms(Some(JS_REPL_POLL_MIN_MS)),
        JS_REPL_POLL_MIN_MS
    );
    assert_eq!(
        clamp_poll_ms(Some(
            crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS * 2
        )),
        crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS
    );
}

#[test]
fn truncate_utf8_prefix_by_bytes_preserves_character_boundaries() {
    let input = "aé🙂z";
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 0), "");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 1), "a");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 2), "a");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 3), "aé");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 6), "aé");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 7), "aé🙂");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, 8), "aé🙂z");
}

#[test]
fn split_utf8_chunks_with_limits_respects_boundaries_and_limits() {
    let chunks = split_utf8_chunks_with_limits("éé🙂z", 3, 2);
    assert_eq!(chunks.len(), 2);
    assert_eq!(std::str::from_utf8(&chunks[0]).unwrap(), "é");
    assert_eq!(std::str::from_utf8(&chunks[1]).unwrap(), "é");
}

#[tokio::test]
async fn exec_buffer_output_deltas_honor_remaining_budget() {
    let (session, turn) = make_session_and_context().await;
    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        None,
        Arc::new(session),
        Arc::new(turn),
    );
    entry.emitted_deltas = MAX_EXEC_OUTPUT_DELTAS_PER_CALL - 1;

    let first = entry.output_delta_chunks_for_log_line("hello");
    assert_eq!(first.len(), 1);
    assert_eq!(String::from_utf8(first[0].clone()).unwrap(), "hello\n");

    let second = entry.output_delta_chunks_for_log_line("world");
    assert!(second.is_empty());
}

#[test]
fn stderr_tail_applies_line_and_byte_limits() {
    let mut lines = VecDeque::new();
    let per_line_cap = JS_REPL_STDERR_TAIL_LINE_MAX_BYTES.min(JS_REPL_STDERR_TAIL_MAX_BYTES);
    let long = "x".repeat(per_line_cap + 128);
    let bounded = push_stderr_tail_line(&mut lines, &long);
    assert_eq!(bounded.len(), per_line_cap);

    for i in 0..50 {
        let line = format!("line-{i}-{}", "y".repeat(200));
        push_stderr_tail_line(&mut lines, &line);
    }

    assert!(lines.len() <= JS_REPL_STDERR_TAIL_LINE_LIMIT);
    assert!(lines.iter().all(|line| line.len() <= per_line_cap));
    assert!(stderr_tail_formatted_bytes(&lines) <= JS_REPL_STDERR_TAIL_MAX_BYTES);
    assert_eq!(
        format_stderr_tail(&lines).len(),
        stderr_tail_formatted_bytes(&lines)
    );
}

#[test]
fn model_kernel_failure_details_are_structured_and_truncated() {
    let snapshot = KernelDebugSnapshot {
        pid: Some(42),
        status: "exited(code=1)".to_string(),
        stderr_tail: "s".repeat(JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES + 400),
    };
    let stream_error = "e".repeat(JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES + 200);
    let message = with_model_kernel_failure_message(
        "js_repl kernel exited unexpectedly",
        "stdout_eof",
        Some(&stream_error),
        &snapshot,
    );
    assert!(message.starts_with("js_repl kernel exited unexpectedly\n\njs_repl diagnostics: "));
    let (_prefix, encoded) = message
        .split_once("js_repl diagnostics: ")
        .expect("diagnostics suffix should be present");
    let parsed: serde_json::Value =
        serde_json::from_str(encoded).expect("diagnostics should be valid json");
    assert_eq!(
        parsed.get("reason").and_then(|v| v.as_str()),
        Some("stdout_eof")
    );
    assert_eq!(
        parsed.get("kernel_pid").and_then(serde_json::Value::as_u64),
        Some(42)
    );
    assert_eq!(
        parsed.get("kernel_status").and_then(|v| v.as_str()),
        Some("exited(code=1)")
    );
    assert!(
        parsed
            .get("kernel_stderr_tail")
            .and_then(|v| v.as_str())
            .expect("kernel_stderr_tail should be present")
            .len()
            <= JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES
    );
    assert!(
        parsed
            .get("stream_error")
            .and_then(|v| v.as_str())
            .expect("stream_error should be present")
            .len()
            <= JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES
    );
}

#[test]
fn write_error_diagnostics_only_attach_for_likely_kernel_failures() {
    let running = KernelDebugSnapshot {
        pid: Some(7),
        status: "running".to_string(),
        stderr_tail: "<empty>".to_string(),
    };
    let exited = KernelDebugSnapshot {
        pid: Some(7),
        status: "exited(code=1)".to_string(),
        stderr_tail: "<empty>".to_string(),
    };
    assert!(!should_include_model_diagnostics_for_write_error(
        "failed to flush kernel message: other io error",
        &running
    ));
    assert!(should_include_model_diagnostics_for_write_error(
        "failed to write to kernel: Broken pipe (os error 32)",
        &running
    ));
    assert!(should_include_model_diagnostics_for_write_error(
        "failed to write to kernel: some other io error",
        &exited
    ));
}

#[test]
fn js_repl_internal_tool_guard_matches_expected_names() {
    assert!(is_js_repl_internal_tool("js_repl"));
    assert!(is_js_repl_internal_tool("js_repl_poll"));
    assert!(is_js_repl_internal_tool("js_repl_reset"));
    assert!(!is_js_repl_internal_tool("shell_command"));
    assert!(!is_js_repl_internal_tool("list_mcp_resources"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_for_exec_tool_calls_map_drains_inflight_calls_without_hanging() {
    let exec_tool_calls = Arc::new(Mutex::new(HashMap::new()));

    for _ in 0..128 {
        let exec_id = Uuid::new_v4().to_string();
        exec_tool_calls
            .lock()
            .await
            .insert(exec_id.clone(), ExecToolCalls::default());
        assert!(
            JsReplManager::begin_exec_tool_call(&exec_tool_calls, &exec_id)
                .await
                .is_some()
        );

        let wait_map = Arc::clone(&exec_tool_calls);
        let wait_exec_id = exec_id.clone();
        let waiter = tokio::spawn(async move {
            JsReplManager::wait_for_exec_tool_calls_map(&wait_map, &wait_exec_id).await;
        });

        let finish_map = Arc::clone(&exec_tool_calls);
        let finish_exec_id = exec_id.clone();
        let finisher = tokio::spawn(async move {
            tokio::task::yield_now().await;
            JsReplManager::finish_exec_tool_call(&finish_map, &finish_exec_id).await;
        });

        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("wait_for_exec_tool_calls_map should not hang")
            .expect("wait task should not panic");
        finisher.await.expect("finish task should not panic");

        JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &exec_id).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_waits_for_exec_lock_before_clearing_exec_tool_calls() {
    let manager = JsReplManager::new(None, Vec::new())
        .await
        .expect("manager should initialize");
    let permit = manager
        .exec_lock
        .clone()
        .acquire_owned()
        .await
        .expect("lock should be acquirable");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;

    let reset_manager = Arc::clone(&manager);
    let mut reset_task = tokio::spawn(async move { reset_manager.reset().await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !reset_task.is_finished(),
        "reset should wait until execute lock is released"
    );
    assert!(
        manager.exec_tool_calls.lock().await.contains_key(&exec_id),
        "reset must not clear tool-call contexts while execute lock is held"
    );

    drop(permit);

    tokio::time::timeout(Duration::from_secs(1), &mut reset_task)
        .await
        .expect("reset should complete after execute lock release")
        .expect("reset task should not panic")
        .expect("reset should succeed");
    assert!(
        !manager.exec_tool_calls.lock().await.contains_key(&exec_id),
        "reset should clear tool-call contexts after lock acquisition"
    );
}

#[test]
fn summarize_tool_call_response_for_multimodal_function_output() {
    let response = ResponseInputItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,abcd".to_string(),
                detail: None,
            },
        ]),
    };

    let actual = JsReplManager::summarize_tool_call_response(&response);

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: Some("function_call_output".to_string()),
            payload_kind: Some(JsReplToolCallPayloadKind::FunctionContentItems),
            payload_text_preview: None,
            payload_text_length: None,
            payload_item_count: Some(1),
            text_item_count: Some(0),
            image_item_count: Some(1),
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[tokio::test]
async fn emitted_image_content_item_preserves_explicit_detail() {
    let (_session, turn) = make_session_and_context().await;
    let content_item = emitted_image_content_item(
        &turn,
        "data:image/png;base64,AAA".to_string(),
        Some(ImageDetail::Low),
    );
    assert_eq!(
        content_item,
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
            detail: Some(ImageDetail::Low),
        }
    );
}

#[tokio::test]
async fn emitted_image_content_item_uses_turn_original_detail_when_enabled() {
    let (_session, mut turn) = make_session_and_context().await;
    Arc::make_mut(&mut turn.config)
        .features
        .enable(Feature::ImageDetailOriginal)
        .expect("test config should allow feature update");
    turn.model_info.supports_image_detail_original = true;

    let content_item =
        emitted_image_content_item(&turn, "data:image/png;base64,AAA".to_string(), None);

    assert_eq!(
        content_item,
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
            detail: Some(ImageDetail::Original),
        }
    );
}

#[test]
fn validate_emitted_image_url_accepts_case_insensitive_data_scheme() {
    assert_eq!(
        validate_emitted_image_url("DATA:image/png;base64,AAA"),
        Ok(())
    );
}

#[test]
fn validate_emitted_image_url_rejects_non_data_scheme() {
    assert_eq!(
        validate_emitted_image_url("https://example.com/image.png"),
        Err("codex.emitImage only accepts data URLs".to_string())
    );
}

#[test]
fn summarize_tool_call_response_for_multimodal_custom_output() {
    let response = ResponseInputItem::CustomToolCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,abcd".to_string(),
                detail: None,
            },
        ]),
    };

    let actual = JsReplManager::summarize_tool_call_response(&response);

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: Some("custom_tool_call_output".to_string()),
            payload_kind: Some(JsReplToolCallPayloadKind::CustomContentItems),
            payload_text_preview: None,
            payload_text_length: None,
            payload_item_count: Some(1),
            text_item_count: Some(0),
            image_item_count: Some(1),
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[test]
fn summarize_tool_call_error_marks_error_payload() {
    let actual = JsReplManager::summarize_tool_call_error("tool failed");

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: None,
            payload_kind: Some(JsReplToolCallPayloadKind::Error),
            payload_text_preview: Some("tool failed".to_string()),
            payload_text_length: Some("tool failed".len()),
            payload_item_count: None,
            text_item_count: None,
            image_item_count: None,
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_clears_inflight_exec_tool_calls_without_waiting() {
    let manager = JsReplManager::new(None, Vec::new())
        .await
        .expect("manager should initialize");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;
    assert!(
        JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
            .await
            .is_some()
    );

    let wait_manager = Arc::clone(&manager);
    let wait_exec_id = exec_id.clone();
    let waiter = tokio::spawn(async move {
        JsReplManager::wait_for_exec_tool_calls_map(&wait_manager.exec_tool_calls, &wait_exec_id)
            .await;
    });
    tokio::task::yield_now().await;

    tokio::time::timeout(Duration::from_secs(1), manager.reset())
        .await
        .expect("reset should not hang")
        .expect("reset should succeed");

    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("waiter should be released")
        .expect("wait task should not panic");

    assert!(manager.exec_tool_calls.lock().await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_aborts_inflight_exec_tool_tasks() {
    let manager = JsReplManager::new(None, Vec::new())
        .await
        .expect("manager should initialize");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;
    let reset_cancel = JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
        .await
        .expect("exec should be registered");

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = reset_cancel.cancelled() => "cancelled",
            _ = tokio::time::sleep(Duration::from_secs(60)) => "timed_out",
        }
    });

    tokio::time::timeout(Duration::from_secs(1), manager.reset())
        .await
        .expect("reset should not hang")
        .expect("reset should succeed");

    let outcome = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("cancelled task should resolve promptly")
        .expect("task should not panic");
    assert_eq!(outcome, "cancelled");
}
#[tokio::test]
async fn exec_buffer_caps_all_logs_by_bytes() {
    let (session, turn) = make_session_and_context().await;
    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        None,
        Arc::new(session),
        Arc::new(turn),
    );
    let chunk = "x".repeat(16 * 1024);
    for _ in 0..96 {
        entry.push_log(chunk.clone());
    }
    assert!(entry.all_logs_truncated);
    assert!(entry.all_logs_bytes <= JS_REPL_POLL_ALL_LOGS_MAX_BYTES);
    assert!(
        entry
            .all_logs
            .last()
            .is_some_and(|line| line.contains("logs truncated"))
    );
}

#[tokio::test]
async fn exec_buffer_log_marker_keeps_newest_logs() {
    let (session, turn) = make_session_and_context().await;
    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        None,
        Arc::new(session),
        Arc::new(turn),
    );
    let filler = "x".repeat(8 * 1024);
    for i in 0..20 {
        entry.push_log(format!("id{i}:{filler}"));
    }

    let drained = entry.poll_logs();
    assert_eq!(
        drained.first().map(String::as_str),
        Some(JS_REPL_POLL_LOGS_TRUNCATED_MARKER)
    );
    assert!(drained.iter().any(|line| line.starts_with("id19:")));
    assert!(!drained.iter().any(|line| line.starts_with("id0:")));
}

#[tokio::test]
async fn exec_buffer_poll_final_output_only_returns_terminal_output() {
    let (session, turn) = make_session_and_context().await;
    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        None,
        Arc::new(session),
        Arc::new(turn),
    );
    entry.push_log("line 1".to_string());
    entry.push_log("line 2".to_string());
    entry.done = true;

    assert_eq!(entry.poll_final_output(), None);
}

#[tokio::test]
async fn complete_exec_in_store_suppresses_kernel_exit_when_host_terminating() {
    let (session, turn) = make_session_and_context().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        None,
        Arc::new(session),
        Arc::new(turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let kernel_exit_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::KernelExit,
        None,
        None,
        Some("js_repl kernel exited unexpectedly".to_string()),
    )
    .await;
    assert!(!kernel_exit_completed);

    {
        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert!(!entry.done);
        assert!(entry.terminal_kind.is_none());
        assert!(entry.error.is_none());
        assert!(entry.host_terminating);
    }

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(cancelled_completed);

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert!(entry.done);
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
    assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
    assert!(!entry.host_terminating);
}

#[tokio::test]
async fn complete_exec_in_store_caps_completed_exec_residency() {
    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let active_exec_id = "active-exec";
    exec_store.lock().await.insert(
        active_exec_id.to_string(),
        ExecBuffer::new(
            "call-active".to_string(),
            Some("session-1".to_string()),
            Arc::clone(&session),
            Arc::clone(&turn),
        ),
    );

    for idx in 0..=JS_REPL_POLL_MAX_COMPLETED_EXECS {
        let exec_id = format!("exec-{idx}");
        exec_store.lock().await.insert(
            exec_id.clone(),
            ExecBuffer::new(
                format!("call-{idx}"),
                Some("session-1".to_string()),
                Arc::clone(&session),
                Arc::clone(&turn),
            ),
        );

        let completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            &exec_id,
            ExecTerminalKind::Success,
            Some(format!("done-{idx}")),
            Some(Vec::new()),
            None,
        )
        .await;
        assert!(completed);
    }

    let store = exec_store.lock().await;
    assert!(store.contains_key(active_exec_id));
    assert!(
        !store
            .get(active_exec_id)
            .expect("active exec should still exist")
            .done
    );
    assert_eq!(
        store.values().filter(|entry| entry.done).count(),
        JS_REPL_POLL_MAX_COMPLETED_EXECS
    );
    assert!(
        !store.contains_key("exec-0"),
        "oldest completed exec should be pruned"
    );
    for idx in 1..=JS_REPL_POLL_MAX_COMPLETED_EXECS {
        assert!(
            store.contains_key(&format!("exec-{idx}")),
            "newer completed exec should still be retained"
        );
    }
}

#[tokio::test]
async fn wait_for_exec_terminal_or_protocol_reader_drained_allows_late_terminal_result_to_win() {
    let (session, turn) = make_session_and_context().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let protocol_reader_drained = CancellationToken::new();

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        Some("session-1".to_string()),
        Arc::new(session),
        Arc::new(turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let exec_store_for_task = Arc::clone(&exec_store);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = JsReplManager::complete_exec_in_store(
            &exec_store_for_task,
            exec_id,
            ExecTerminalKind::Success,
            Some("done".to_string()),
            Some(Vec::new()),
            None,
        )
        .await;
    });

    JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
        &exec_store,
        exec_id,
        &protocol_reader_drained,
    )
    .await;

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(!cancelled_completed);

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert!(entry.done);
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Success));
    assert_eq!(entry.final_output.as_deref(), Some("done"));
    assert_eq!(entry.error, None);
}

#[tokio::test]
async fn wait_for_exec_terminal_or_protocol_reader_drained_ignores_non_terminal_notifications() {
    let (session, turn) = make_session_and_context().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let protocol_reader_drained = CancellationToken::new();

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        Some("session-1".to_string()),
        Arc::new(session),
        Arc::new(turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let exec_store_for_task = Arc::clone(&exec_store);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        {
            let mut store = exec_store_for_task.lock().await;
            let entry = store.get_mut(exec_id).expect("exec entry should exist");
            entry.push_log("still running".to_string());
            entry.notify.notify_waiters();
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = JsReplManager::complete_exec_in_store(
            &exec_store_for_task,
            exec_id,
            ExecTerminalKind::Success,
            Some("done".to_string()),
            Some(Vec::new()),
            None,
        )
        .await;
    });

    JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
        &exec_store,
        exec_id,
        &protocol_reader_drained,
    )
    .await;

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(!cancelled_completed);

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Success));
    assert_eq!(entry.final_output.as_deref(), Some("done"));
    assert_eq!(entry.error, None);
}

#[tokio::test]
async fn wait_for_exec_terminal_or_protocol_reader_drained_returns_after_reader_drained() {
    let (session, turn) = make_session_and_context().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let protocol_reader_drained = CancellationToken::new();

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        Some("session-1".to_string()),
        Arc::new(session),
        Arc::new(turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let protocol_reader_drained_for_task = protocol_reader_drained.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        protocol_reader_drained_for_task.cancel();
    });

    JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
        &exec_store,
        exec_id,
        &protocol_reader_drained,
    )
    .await;

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(cancelled_completed);

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
    assert_eq!(entry.final_output, None);
    assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
}

#[tokio::test]
async fn late_terminal_result_after_forced_cancel_is_ignored() {
    let (session, turn) = make_session_and_context().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        Some("session-1".to_string()),
        Arc::new(session),
        Arc::new(turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(cancelled_completed);

    let success_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Success,
        Some("done".to_string()),
        Some(Vec::new()),
        None,
    )
    .await;
    assert!(!success_completed);

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert!(entry.done);
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
    assert_eq!(entry.final_output, None);
    assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
}

#[tokio::test]
async fn late_terminal_result_after_forced_cancel_keeps_state_and_event_aligned() {
    let (session, turn, rx) = make_session_and_context_with_rx().await;
    let exec_id = "exec-1";
    let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let mut entry = ExecBuffer::new(
        "call-1".to_string(),
        Some("session-1".to_string()),
        Arc::clone(&session),
        Arc::clone(&turn),
    );
    entry.host_terminating = true;
    exec_store.lock().await.insert(exec_id.to_string(), entry);

    let cancelled_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Cancelled,
        None,
        None,
        Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
    )
    .await;
    assert!(cancelled_completed);

    let first_end = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = rx.recv().await.expect("event");
            if let EventMsg::ExecCommandEnd(end) = event.msg {
                break end;
            }
        }
    })
    .await
    .expect("timed out waiting for first exec end");
    assert_eq!(first_end.call_id, "call-1");
    assert_eq!(first_end.stderr, JS_REPL_CANCEL_ERROR_MESSAGE);

    let success_completed = JsReplManager::complete_exec_in_store(
        &exec_store,
        exec_id,
        ExecTerminalKind::Success,
        Some("done".to_string()),
        Some(Vec::new()),
        None,
    )
    .await;
    assert!(!success_completed);

    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err(),
        "expected no second exec end event after ignored late terminal result"
    );

    let store = exec_store.lock().await;
    let entry = store.get(exec_id).expect("exec entry should exist");
    assert!(entry.done);
    assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
    assert_eq!(entry.final_output, None);
    assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
}

#[test]
fn build_js_repl_exec_output_sets_timed_out() {
    let out = build_js_repl_exec_output("", Some("timeout"), Duration::from_millis(50), true);
    assert!(out.timed_out);
}

async fn can_run_js_repl_runtime_tests() -> bool {
    // These white-box runtime tests are required on macOS. Linux relies on
    // the codex-linux-sandbox arg0 dispatch path, which is exercised in
    // integration tests instead.
    cfg!(target_os = "macos")
}

async fn poll_until_done_with_logs(
    manager: &Arc<JsReplManager>,
    exec_id: &str,
    yield_time_ms: u64,
    timeout: Duration,
) -> anyhow::Result<(JsExecPollResult, Vec<String>)> {
    let deadline = Instant::now() + timeout;
    let mut observed_logs = Vec::new();
    loop {
        let result = manager.poll(exec_id, Some(yield_time_ms)).await?;
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            return Ok((result, observed_logs));
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for polled js_repl exec completion");
        }
    }
}

fn write_js_repl_test_package_source(base: &Path, name: &str, source: &str) -> anyhow::Result<()> {
    let pkg_dir = base.join("node_modules").join(name);
    fs::create_dir_all(&pkg_dir)?;
    fs::write(
        pkg_dir.join("package.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {{\n    \"import\": \"./index.js\"\n  }}\n}}\n"
        ),
    )?;
    fs::write(pkg_dir.join("index.js"), source)?;
    Ok(())
}

fn write_js_repl_test_package(base: &Path, name: &str, value: &str) -> anyhow::Result<()> {
    write_js_repl_test_package_source(base, name, &format!("export const value = \"{value}\";\n"))?;
    Ok(())
}

fn write_js_repl_test_module(base: &Path, relative: &str, contents: &str) -> anyhow::Result<()> {
    let module_path = base.join(relative);
    if let Some(parent) = module_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(module_path, contents)?;
    Ok(())
}

#[tokio::test]
async fn js_repl_timeout_does_not_deadlock() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        manager.execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "while (true) {}".to_string(),
                timeout_ms: Some(50),
                poll: false,
                session_id: None,
            },
        ),
    )
    .await
    .expect("execute should return, not deadlock")
    .expect_err("expected timeout error");

    assert_eq!(result, JsReplExecuteError::TimedOut);
    Ok(())
}

#[tokio::test]
async fn js_repl_timeout_kills_kernel_process() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    let process = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.process)
    };

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "while (true) {}".to_string(),
                timeout_ms: Some(50),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected timeout error");

    assert_eq!(result, JsReplExecuteError::TimedOut);

    assert!(
        process.has_exited(),
        "timed out js_repl execution should kill previous kernel process"
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_forced_kernel_exit_recovers_on_next_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    let process = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.process)
    };
    JsReplManager::kill_kernel_child(&process, "test_crash").await;
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let cleared = {
                let guard = manager.kernel.lock().await;
                guard
                    .as_ref()
                    .is_none_or(|state| !Arc::ptr_eq(&state.process, &process))
            };
            if cleared {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host should clear dead kernel state promptly");

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log('after-kill');".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("after-kill"));
    Ok(())
}

#[tokio::test]
async fn js_repl_uncaught_exception_returns_exec_error_and_recovers() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = crate::codex::make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    let process = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.process)
    };

    let err = tokio::time::timeout(
            Duration::from_secs(3),
            manager.execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "setTimeout(() => { throw new Error('boom'); }, 0);\nawait new Promise(() => {});".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            ),
        )
        .await
        .expect("uncaught exception should fail promptly")
        .expect_err("expected uncaught exception to fail the exec");

    let message = err.to_string();
    assert!(message.contains("js_repl kernel uncaught exception: boom"));
    assert!(message.contains("kernel reset."));
    assert!(message.contains("Catch or handle async errors"));
    assert!(!message.contains("js_repl kernel exited unexpectedly"));

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if process.has_exited() {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("uncaught exception should terminate the previous kernel process")?;

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let cleared = {
                let guard = manager.kernel.lock().await;
                guard
                    .as_ref()
                    .is_none_or(|state| !Arc::ptr_eq(&state.process, &process))
            };
            if cleared {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host should clear dead kernel state promptly");

    let next = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log('after reset');".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(next.output.contains("after reset"));
    Ok(())
}

#[tokio::test]
async fn js_repl_waits_for_unawaited_tool_calls_before_completion() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let marker = turn
        .cwd
        .join(format!("js-repl-unawaited-marker-{}.txt", Uuid::new_v4()));
    let marker_json = serde_json::to_string(&marker.to_string_lossy().to_string())?;
    let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: format!(
                        r#"
const marker = {marker_json};
void codex.tool("shell_command", {{ command: `sleep 0.35; printf js_repl_unawaited_done > "${{marker}}"` }});
console.log("cell-complete");
"#
                    ),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
    assert!(result.output.contains("cell-complete"));
    let marker_contents = tokio::fs::read_to_string(&marker).await?;
    assert_eq!(marker_contents, "js_repl_unawaited_done");
    let _ = tokio::fs::remove_file(&marker).await;
    Ok(())
}

#[tokio::test]
async fn js_repl_persisted_tool_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let global_marker = turn
        .cwd
        .join(format!("js-repl-global-helper-{}.txt", Uuid::new_v4()));
    let lexical_marker = turn
        .cwd
        .join(format!("js-repl-lexical-helper-{}.txt", Uuid::new_v4()));
    let global_marker_json = serde_json::to_string(&global_marker.to_string_lossy().to_string())?;
    let lexical_marker_json = serde_json::to_string(&lexical_marker.to_string_lossy().to_string())?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: format!(
                    r#"
const globalMarker = {global_marker_json};
const lexicalMarker = {lexical_marker_json};
const savedTool = codex.tool;
globalThis.globalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf global_helper > "${{globalMarker}}"` }}),
}};
const lexicalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf lexical_helper > "${{lexicalMarker}}"` }}),
}};
"#
                ),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    let next = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: r#"
await globalToolHelper.run();
await lexicalToolHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    assert!(next.output.contains("helpers-ran"));
    assert_eq!(
        tokio::fs::read_to_string(&global_marker).await?,
        "global_helper"
    );
    assert_eq!(
        tokio::fs::read_to_string(&lexical_marker).await?,
        "lexical_helper"
    );
    let _ = tokio::fs::remove_file(&global_marker).await;
    let _ = tokio::fs::remove_file(&lexical_marker).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_persisted_emit_image_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: format!(
                    r#"
const dataUrl = "{data_url}";
const savedEmitImage = codex.emitImage;
globalThis.globalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
const lexicalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
"#
                ),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    let next = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: r#"
await globalEmitHelper.run();
await lexicalEmitHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;

    assert!(next.output.contains("helpers-ran"));
    assert_eq!(
        next.content_items,
        vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: None,
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: None,
            },
        ]
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_does_not_auto_attach_image_via_view_image_tool() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
console.log(out.type);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("function_call_output"));
    assert!(result.content_items.is_empty());
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_image_via_view_image_tool() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image-explicit.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
console.log(out.type);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("function_call_output"));
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_multiple_view_image_calls_attach_multiple_images() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
const imagePathA = path.join(codex.tmpDir, "js-repl-view-image-a.png");
const imagePathB = path.join(codex.tmpDir, "js-repl-view-image-b.png");
await fs.writeFile(imagePathA, png);
await fs.writeFile(imagePathB, png);
const outA = await codex.tool("view_image", { path: imagePathA });
const outB = await codex.tool("view_image", { path: imagePathB });
await codex.emitImage(outA);
await codex.emitImage(outB);
console.log("attached-two-images");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("attached-two-images"));
    assert_eq!(
        result.content_items.len(),
        2,
        "expected one input_image content item per nested view_image call"
    );
    for item in &result.content_items {
        let FunctionCallOutputContentItem::InputImage { image_url, .. } = item else {
            panic!("expected each content item to be an image");
        };
        assert!(image_url.starts_with("data:image/png;base64,"));
    }
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_multiple_view_image_calls_attach_multiple_images() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
const imagePathA = path.join(codex.tmpDir, "js-repl-poll-view-image-a.png");
const imagePathB = path.join(codex.tmpDir, "js-repl-poll-view-image-b.png");
await fs.writeFile(imagePathA, png);
await fs.writeFile(imagePathB, png);
const outA = await codex.tool("view_image", { path: imagePathA });
const outB = await codex.tool("view_image", { path: imagePathB });
await codex.emitImage(outA);
await codex.emitImage(outB);
console.log("attached-two-images");
"#;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-poll-two-view-images".to_string(),
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_logs = Vec::new();
    let result = loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            assert_eq!(result.session_id, submission.session_id);
            assert_eq!(result.error, None);
            let logs = observed_logs.join("\n");
            assert!(logs.contains("attached-two-images"));
            assert_eq!(result.final_output.as_deref(), Some(""));
            break result;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for polling multi-view_image exec completion");
        }
    };
    assert_eq!(
        result.content_items.len(),
        2,
        "expected one input_image content item per nested view_image call"
    );
    for item in &result.content_items {
        let FunctionCallOutputContentItem::InputImage { image_url, .. } = item else {
            panic!("expected each content item to be an image");
        };
        assert!(image_url.starts_with("data:image/png;base64,"));
    }
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_completed_multimodal_exec_is_replayable() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-poll-replay-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
console.log("replay-image-ready");
"#;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-poll-replay-image".to_string(),
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_logs = Vec::new();
    let first_result = loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            break result;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for polling replay-image exec completion");
        }
    };
    assert_eq!(first_result.session_id, submission.session_id);
    assert_eq!(first_result.error, None);
    assert!(
        observed_logs
            .iter()
            .any(|line| line.contains("replay-image-ready"))
    );
    assert_eq!(first_result.final_output.as_deref(), Some(""));
    assert_eq!(
            first_result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );

    let second_result = manager.poll(&submission.exec_id, Some(50)).await?;
    assert!(second_result.done);
    assert_eq!(second_result.session_id, submission.session_id);
    assert_eq!(second_result.error, None);
    assert!(second_result.logs.is_empty());
    assert_eq!(second_result.final_output, first_result.final_output);
    assert_eq!(second_result.content_items, first_result.content_items);
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_image_from_bytes_and_mime_type() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png" });
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_multiple_images_in_one_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
await codex.emitImage(
  "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert_eq!(
            result.content_items.as_slice(),
            [
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                            .to_string(),
                    detail: None,
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
                            .to_string(),
                    detail: None,
                },
            ]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_waits_for_unawaited_emit_image_before_completion() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
void codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
console.log("cell-complete");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("cell-complete"));
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_unawaited_emit_image_errors_fail_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
void codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
console.log("cell-complete");
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("unawaited invalid emitImage should fail");
    assert!(err.to_string().contains("expected non-empty bytes"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_caught_emit_image_error_does_not_fail_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
try {
  await codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
} catch (error) {
  console.log(error.message);
}
console.log("cell-complete");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("expected non-empty bytes"));
    assert!(result.output.contains("cell-complete"));
    assert!(result.content_items.is_empty());
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_requires_explicit_mime_type_for_bytes() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png });
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("missing mimeType should fail");
    assert!(err.to_string().contains("expected a non-empty mimeType"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_non_data_url() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage("https://example.com/image.png");
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("non-data URLs should fail");
    assert!(err.to_string().contains("only accepts data URLs"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_accepts_case_insensitive_data_url() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage("DATA:image/png;base64,AAA");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert_eq!(
        result.content_items.as_slice(),
        [FunctionCallOutputContentItem::InputImage {
            image_url: "DATA:image/png;base64,AAA".to_string(),
            detail: None,
        }]
        .as_slice()
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_invalid_detail() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png", detail: "ultra" });
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("invalid detail should fail");
    assert!(err.to_string().contains("expected detail to be one of"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_mixed_content() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn, rx_event) =
        make_session_and_context_with_dynamic_tools_and_rx(vec![DynamicToolSpec {
            name: "inline_image".to_string(),
            description: "Returns inline text and image content.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }])
        .await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const out = await codex.tool("inline_image", {});
await codex.emitImage(out);
"#;
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    let session_for_response = Arc::clone(&session);
    let response_watcher = async move {
        loop {
            let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv()).await??;
            if let EventMsg::DynamicToolCallRequest(request) = event.msg {
                session_for_response
                    .notify_dynamic_tool_response(
                        &request.call_id,
                        DynamicToolResponse {
                            content_items: vec![
                                DynamicToolCallOutputContentItem::InputText {
                                    text: "inline image note".to_string(),
                                },
                                DynamicToolCallOutputContentItem::InputImage {
                                    image_url: image_url.to_string(),
                                },
                            ],
                            success: true,
                        },
                    )
                    .await;
                return Ok::<(), anyhow::Error>(());
            }
        }
    };

    let (result, response_watcher_result) = tokio::join!(
        manager.execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
                poll: false,
                session_id: None,
            },
        ),
        response_watcher,
    );
    response_watcher_result?;
    let err = result.expect_err("mixed content should fail");
    assert!(
        err.to_string()
            .contains("does not accept mixed text and image content")
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}
#[tokio::test]
async fn js_repl_prefers_env_node_module_dirs_over_config() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let env_base = tempdir()?;
    write_js_repl_test_package(env_base.path(), "repl_probe", "env")?;

    let config_base = tempdir()?;
    let cwd_dir = tempdir()?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy.r#set.insert(
        "CODEX_JS_REPL_NODE_MODULE_DIRS".to_string(),
        env_base.path().to_string_lossy().to_string(),
    );
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![config_base.path().to_path_buf()],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("env"));
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_submit_and_complete() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-1".to_string(),
            JsReplArgs {
                code: "console.log('poll-ok');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    assert!(!submission.session_id.is_empty());

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_logs = Vec::new();
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        assert_eq!(result.session_id, submission.session_id);
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            let logs = observed_logs.join("\n");
            assert!(logs.contains("poll-ok"));
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for js_repl poll completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_session_reuse_preserves_state() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let first = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            "call-session-first".to_string(),
            JsReplArgs {
                code: "let persisted = 41;".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    loop {
        let result = manager.poll(&first.exec_id, Some(200)).await?;
        if result.done {
            break;
        }
    }

    let second = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-session-second".to_string(),
            JsReplArgs {
                code: "console.log(persisted + 1);".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(first.session_id.clone()),
            },
        )
        .await?;
    assert_eq!(second.session_id, first.session_id);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_logs = Vec::new();
    loop {
        let result = manager.poll(&second.exec_id, Some(200)).await?;
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            let logs = observed_logs.join("\n");
            assert!(logs.contains("42"));
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for reused polling session completion");
        }
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_persisted_tool_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let global_marker = turn
        .cwd
        .join(format!("js-repl-poll-global-helper-{}.txt", Uuid::new_v4()));
    let lexical_marker = turn.cwd.join(format!(
        "js-repl-poll-lexical-helper-{}.txt",
        Uuid::new_v4()
    ));
    let global_marker_json = serde_json::to_string(&global_marker.to_string_lossy().to_string())?;
    let lexical_marker_json = serde_json::to_string(&lexical_marker.to_string_lossy().to_string())?;

    let first = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            "call-poll-persisted-tool-first".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
const globalMarker = {global_marker_json};
const lexicalMarker = {lexical_marker_json};
const savedTool = codex.tool;
globalThis.globalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf global_helper > "${{globalMarker}}"` }}),
}};
const lexicalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf lexical_helper > "${{lexicalMarker}}"` }}),
}};
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    let _ =
        poll_until_done_with_logs(&manager, &first.exec_id, 200, Duration::from_secs(5)).await?;

    let second = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-poll-persisted-tool-second".to_string(),
            JsReplArgs {
                code: r#"
await globalToolHelper.run();
await lexicalToolHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(first.session_id.clone()),
            },
        )
        .await?;
    assert_eq!(second.session_id, first.session_id);

    let (result, observed_logs) =
        poll_until_done_with_logs(&manager, &second.exec_id, 200, Duration::from_secs(5)).await?;
    assert_eq!(result.error, None);
    assert_eq!(result.final_output.as_deref(), Some(""));
    assert!(observed_logs.join("\n").contains("helpers-ran"));
    assert_eq!(
        tokio::fs::read_to_string(&global_marker).await?,
        "global_helper"
    );
    assert_eq!(
        tokio::fs::read_to_string(&lexical_marker).await?,
        "lexical_helper"
    );
    let _ = tokio::fs::remove_file(&global_marker).await;
    let _ = tokio::fs::remove_file(&lexical_marker).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_persisted_emit_image_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    let first = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            "call-poll-persisted-image-first".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
const dataUrl = "{data_url}";
const savedEmitImage = codex.emitImage;
globalThis.globalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
const lexicalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    let _ =
        poll_until_done_with_logs(&manager, &first.exec_id, 200, Duration::from_secs(5)).await?;

    let second = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-poll-persisted-image-second".to_string(),
            JsReplArgs {
                code: r#"
await globalEmitHelper.run();
await lexicalEmitHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(first.session_id.clone()),
            },
        )
        .await?;
    assert_eq!(second.session_id, first.session_id);

    let (result, observed_logs) =
        poll_until_done_with_logs(&manager, &second.exec_id, 200, Duration::from_secs(5)).await?;
    assert_eq!(result.error, None);
    assert_eq!(result.final_output.as_deref(), Some(""));
    assert!(observed_logs.join("\n").contains("helpers-ran"));
    assert_eq!(
        result.content_items.as_slice(),
        [
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: None,
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: None,
            },
        ]
        .as_slice()
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_delayed_timer_tool_helper_keeps_exec_active() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let marker = turn.cwd.join(format!(
        "js-repl-poll-delayed-helper-{}.txt",
        Uuid::new_v4()
    ));
    let marker_json = serde_json::to_string(&marker.to_string_lossy().to_string())?;

    let first = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            "call-poll-delayed-tool-first".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
const marker = {marker_json};
const savedTool = codex.tool;
globalThis.delayedToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf delayed_helper > "${{marker}}"` }}),
}};
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    let _ =
        poll_until_done_with_logs(&manager, &first.exec_id, 200, Duration::from_secs(5)).await?;

    let second = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-poll-delayed-tool-second".to_string(),
            JsReplArgs {
                code: r#"
setTimeout(() => {
  console.log("timer-fired");
  void delayedToolHelper.run();
}, 50);
console.log("scheduled");
"#
                .to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(first.session_id.clone()),
            },
        )
        .await?;

    let (result, observed_logs) =
        poll_until_done_with_logs(&manager, &second.exec_id, 200, Duration::from_secs(5)).await?;
    let logs = observed_logs.join("\n");
    assert_eq!(result.error, None);
    assert_eq!(result.final_output.as_deref(), Some(""));
    assert!(logs.contains("scheduled"));
    assert!(logs.contains("timer-fired"));
    assert_eq!(tokio::fs::read_to_string(&marker).await?, "delayed_helper");
    let _ = tokio::fs::remove_file(&marker).await;

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_rejects_submit_with_unknown_session_id() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;
    let err = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-session-missing".to_string(),
            JsReplArgs {
                code: "console.log('should not run');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some("missing-session".to_string()),
            },
        )
        .await
        .expect_err("expected missing session submit rejection");
    assert_eq!(err.to_string(), "js_repl session id not found");

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_rejects_timeout_ms_on_submit() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;
    let err = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-session-timeout-unsupported".to_string(),
            JsReplArgs {
                code: "console.log('should not run');".to_string(),
                timeout_ms: Some(5_000),
                poll: true,
                session_id: None,
            },
        )
        .await
        .expect_err("expected timeout_ms polling submit rejection");
    assert_eq!(err.to_string(), JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_concurrent_submit_same_session_rejects_second_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;
    let seed_submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-concurrent-seed".to_string(),
            JsReplArgs {
                code: "console.log('seed');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    loop {
        let result = manager.poll(&seed_submission.exec_id, Some(200)).await?;
        if result.done {
            break;
        }
    }
    let shared_session_id = seed_submission.session_id.clone();

    let manager_a = Arc::clone(&manager);
    let session_a = Arc::clone(&session);
    let turn_a = Arc::clone(&turn);
    let shared_session_id_a = shared_session_id.clone();
    let submit_a = tokio::spawn(async move {
        Arc::clone(&manager_a)
            .submit(
                Arc::clone(&session_a),
                Arc::clone(&turn_a),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-concurrent-a".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 500));".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some(shared_session_id_a),
                },
            )
            .await
    });

    let manager_b = Arc::clone(&manager);
    let session_b = Arc::clone(&session);
    let turn_b = Arc::clone(&turn);
    let shared_session_id_b = shared_session_id.clone();
    let submit_b = tokio::spawn(async move {
        Arc::clone(&manager_b)
            .submit(
                Arc::clone(&session_b),
                Arc::clone(&turn_b),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-concurrent-b".to_string(),
                JsReplArgs {
                    code: "console.log('blocked');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some(shared_session_id_b),
                },
            )
            .await
    });

    let (result_a, result_b) = tokio::join!(submit_a, submit_b);
    let result_a = result_a.expect("task A should not panic");
    let result_b = result_b.expect("task B should not panic");
    let mut outcomes = vec![result_a, result_b];

    let first_error_index = outcomes.iter().position(Result::is_err);
    let Some(error_index) = first_error_index else {
        panic!("expected one submit to fail due to active exec in shared session");
    };
    assert_eq!(
        outcomes.iter().filter(|result| result.is_ok()).count(),
        1,
        "exactly one submit should succeed for a shared session id",
    );
    let err = outcomes
        .swap_remove(error_index)
        .expect_err("expected submit failure");
    assert!(
        err.to_string().contains("already has a running exec"),
        "unexpected concurrent-submit error: {err}",
    );
    let submission = outcomes
        .pop()
        .expect("one submission should remain")
        .expect("remaining submission should succeed");
    assert_eq!(submission.session_id, shared_session_id);

    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        if result.done {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for shared-session winner completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let _ = manager.reset_session(&shared_session_id).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_submit_enforces_capacity_during_concurrent_inserts() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;
    let template_kernel = manager
        .start_kernel(Arc::clone(&session), Arc::clone(&turn), None)
        .await
        .map_err(anyhow::Error::msg)?;

    let submit_a;
    let submit_b;
    {
        let mut sessions = manager.poll_sessions.lock().await;
        for idx in 0..(JS_REPL_POLL_MAX_SESSIONS - 1) {
            sessions.insert(
                format!("prefill-{idx}"),
                PollSessionState {
                    kernel: template_kernel.clone(),
                    active_exec: Some(format!("busy-{idx}")),
                    last_used: Instant::now(),
                },
            );
        }

        let manager_a = Arc::clone(&manager);
        let session_a = Arc::clone(&session);
        let turn_a = Arc::clone(&turn);
        submit_a = tokio::spawn(async move {
            Arc::clone(&manager_a)
                .submit(
                    Arc::clone(&session_a),
                    Arc::clone(&turn_a),
                    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                    "call-capacity-a".to_string(),
                    JsReplArgs {
                        code: "await new Promise((resolve) => setTimeout(resolve, 300));"
                            .to_string(),
                        timeout_ms: None,
                        poll: true,
                        session_id: None,
                    },
                )
                .await
        });

        let manager_b = Arc::clone(&manager);
        let session_b = Arc::clone(&session);
        let turn_b = Arc::clone(&turn);
        submit_b = tokio::spawn(async move {
            Arc::clone(&manager_b)
                .submit(
                    Arc::clone(&session_b),
                    Arc::clone(&turn_b),
                    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                    "call-capacity-b".to_string(),
                    JsReplArgs {
                        code: "await new Promise((resolve) => setTimeout(resolve, 300));"
                            .to_string(),
                        timeout_ms: None,
                        poll: true,
                        session_id: None,
                    },
                )
                .await
        });

        tokio::task::yield_now().await;
    }

    let (result_a, result_b) = tokio::join!(submit_a, submit_b);
    let result_a = result_a.expect("task A should not panic");
    let result_b = result_b.expect("task B should not panic");
    let outcomes = [result_a, result_b];
    assert_eq!(
        outcomes.iter().filter(|result| result.is_ok()).count(),
        1,
        "exactly one concurrent submit should succeed when one slot remains",
    );
    assert_eq!(
        outcomes.iter().filter(|result| result.is_err()).count(),
        1,
        "exactly one concurrent submit should fail when one slot remains",
    );
    let err = outcomes
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("one submission should fail");
    assert!(
        err.to_string()
            .contains("has reached the maximum of 16 active sessions"),
        "unexpected capacity error: {err}",
    );
    assert!(
        manager.poll_sessions.lock().await.len() <= JS_REPL_POLL_MAX_SESSIONS,
        "poll session map must never exceed configured capacity",
    );

    manager.reset().await?;
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_rejects_submit_when_session_has_active_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-session-active".to_string(),
            JsReplArgs {
                code: "await new Promise((resolve) => setTimeout(resolve, 10_000));".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let err = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-session-active-conflict".to_string(),
            JsReplArgs {
                code: "console.log('should not run');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(submission.session_id.clone()),
            },
        )
        .await
        .expect_err("expected active session submit rejection");
    assert_eq!(
        err.to_string(),
        format!(
            "js_repl session `{}` already has a running exec: `{}`",
            submission.session_id, submission.exec_id
        )
    );

    manager.reset_session(&submission.session_id).await?;
    let done = manager.poll(&submission.exec_id, Some(200)).await?;
    assert!(done.done);

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_emits_exec_output_delta_events() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn, rx) = crate::codex::make_session_and_context_with_rx().await;
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-delta-stream".to_string(),
            JsReplArgs {
                code: "console.log('delta-one'); console.log('delta-two');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_one = false;
    let mut saw_two = false;
    loop {
        if saw_one && saw_two {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for js_repl output delta events");
        }
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await
            && let EventMsg::ExecCommandOutputDelta(delta) = event.msg
            && delta.call_id == "call-delta-stream"
        {
            let text = String::from_utf8_lossy(&delta.chunk);
            if text.contains("delta-one") {
                saw_one = true;
            }
            if text.contains("delta-two") {
                saw_two = true;
            }
        }
        let result = manager.poll(&submission.exec_id, Some(50)).await?;
        if result.done && saw_one && saw_two {
            break;
        }
    }

    let completion_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&submission.exec_id, Some(100)).await?;
        if result.done {
            break;
        }
        if Instant::now() >= completion_deadline {
            panic!("timed out waiting for js_repl poll completion");
        }
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_submit_supports_parallel_execs() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let slow_submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                "call-slow".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 2000)); console.log('slow-done');".to_string(),
                    timeout_ms: None,
                    poll: true,
                session_id: None,
                },
            )
            .await?;

    let fast_submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-fast".to_string(),
            JsReplArgs {
                code: "console.log('fast-done');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;
    assert_ne!(slow_submission.session_id, fast_submission.session_id);

    let fast_start = Instant::now();
    let mut fast_logs = Vec::new();
    let fast_output = loop {
        let result = manager.poll(&fast_submission.exec_id, Some(200)).await?;
        fast_logs.extend(result.logs.iter().cloned());
        if result.done {
            assert_eq!(result.final_output.as_deref(), Some(""));
            break fast_logs.join("\n");
        }
        if fast_start.elapsed() > Duration::from_millis(1_500) {
            panic!("fast polled exec did not complete quickly; submit appears serialized");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(fast_output.contains("fast-done"));

    let slow_deadline = Instant::now() + Duration::from_secs(8);
    let mut slow_logs = Vec::new();
    loop {
        let result = manager.poll(&slow_submission.exec_id, Some(200)).await?;
        slow_logs.extend(result.logs.iter().cloned());
        if result.done {
            let logs = slow_logs.join("\n");
            assert!(logs.contains("slow-done"));
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= slow_deadline {
            panic!("timed out waiting for slow polled exec completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_completed_exec_is_replayable() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-replay".to_string(),
            JsReplArgs {
                code: "console.log('replay-ok');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_logs = Vec::new();
    let first_result = loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        observed_logs.extend(result.logs.iter().cloned());
        if result.done {
            break result;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for js_repl poll completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(observed_logs.iter().any(|line| line.contains("replay-ok")));
    assert_eq!(first_result.final_output.as_deref(), Some(""));
    assert_eq!(first_result.session_id, submission.session_id);

    let second_result = manager.poll(&submission.exec_id, Some(50)).await?;
    assert!(second_result.done);
    assert_eq!(second_result.session_id, submission.session_id);
    assert!(second_result.logs.is_empty());
    assert_eq!(second_result.final_output.as_deref(), Some(""));

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_timeout_resnapshots_state_before_returning() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let exec_id = format!("exec-missed-notify-{}", Uuid::new_v4());
    let poll_session_id = format!("session-missed-notify-{}", Uuid::new_v4());
    manager.exec_store.lock().await.insert(
        exec_id.clone(),
        ExecBuffer::new(
            "call-missed-notify".to_string(),
            Some(poll_session_id.clone()),
            Arc::clone(&session),
            Arc::clone(&turn),
        ),
    );

    let manager_for_poll = Arc::clone(&manager);
    let exec_id_for_poll = exec_id.clone();
    let poll_task =
        tokio::spawn(async move { manager_for_poll.poll(&exec_id_for_poll, Some(80)).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    {
        let mut store = manager.exec_store.lock().await;
        let entry = store
            .get_mut(&exec_id)
            .expect("exec entry should exist while polling");
        entry.push_log("late log".to_string());
        entry.final_output = Some("late log".to_string());
        entry.done = true;
        // Intentionally skip notify_waiters to emulate a missed wake window.
    }

    let result = poll_task
        .await
        .expect("poll task should not panic")
        .expect("poll should succeed");
    assert!(result.done);
    assert_eq!(result.session_id, poll_session_id);
    assert_eq!(result.logs, vec!["late log".to_string()]);
    assert_eq!(result.final_output.as_deref(), Some("late log"));

    Ok(())
}

#[tokio::test]
async fn js_repl_reset_session_succeeds_for_idle_session() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-reset-idle".to_string(),
            JsReplArgs {
                code: "console.log('idle');".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        if result.done {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for js_repl poll completion");
        }
    }

    let replay_before_reset = manager.poll(&submission.exec_id, Some(50)).await?;
    assert!(replay_before_reset.done);

    manager.reset_session(&submission.session_id).await?;
    let poll_err = manager
        .poll(&submission.exec_id, Some(50))
        .await
        .expect_err("expected completed poll state to be cleared by reset");
    assert_eq!(poll_err.to_string(), "js_repl exec id not found");
    let err = manager
        .reset_session(&submission.session_id)
        .await
        .expect_err("expected missing session id after reset");
    assert_eq!(err.to_string(), "js_repl session id not found");

    Ok(())
}

#[tokio::test]
async fn js_repl_resolves_from_first_config_dir() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let first_base = tempdir()?;
    let second_base = tempdir()?;
    write_js_repl_test_package(first_base.path(), "repl_probe", "first")?;
    write_js_repl_test_package(second_base.path(), "repl_probe", "second")?;

    let cwd_dir = tempdir()?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![
            first_base.path().to_path_buf(),
            second_base.path().to_path_buf(),
        ],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("first"));
    Ok(())
}

#[tokio::test]
async fn js_repl_falls_back_to_cwd_node_modules() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let config_base = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_package(cwd_dir.path(), "repl_probe", "cwd")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![config_base.path().to_path_buf()],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("cwd"));
    Ok(())
}

#[tokio::test]
async fn js_repl_accepts_node_modules_dir_entries() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let base_dir = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_package(base_dir.path(), "repl_probe", "normalized")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![base_dir.path().join("node_modules")],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("normalized"));
    Ok(())
}

#[tokio::test]
async fn js_repl_supports_relative_file_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "child.js",
        "export const value = \"child\";\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "parent.js",
        "import { value as childValue } from \"./child.js\";\nexport const value = `${childValue}-parent`;\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "local.mjs",
        "export const value = \"mjs\";\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const parent = await import(\"./parent.js\"); const other = await import(\"./local.mjs\"); console.log(parent.value); console.log(other.value);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
    assert!(result.output.contains("child-parent"));
    assert!(result.output.contains("mjs"));
    Ok(())
}

#[tokio::test]
async fn js_repl_supports_absolute_file_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let module_dir = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        module_dir.path(),
        "absolute.js",
        "export const value = \"absolute\";\n",
    )?;
    let absolute_path_json =
        serde_json::to_string(&module_dir.path().join("absolute.js").display().to_string())?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: format!(
                    "const mod = await import({absolute_path_json}); console.log(mod.value);"
                ),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("absolute"));
    Ok(())
}

#[tokio::test]
async fn js_repl_imported_local_files_can_access_repl_globals() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "globals.js",
        "console.log(codex.tmpDir === tmpDir);\nconsole.log(typeof codex.tool);\nconsole.log(\"local-file-console-ok\");\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./globals.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("true"));
    assert!(result.output.contains("function"));
    assert!(result.output.contains("local-file-console-ok"));
    Ok(())
}

#[tokio::test]
async fn js_repl_reimports_local_files_after_edit() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let helper_path = cwd_dir.path().join("helper.js");
    fs::write(&helper_path, "export const value = \"v1\";\n")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let first = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "const { value: firstValue } = await import(\"./helper.js\");\nconsole.log(firstValue);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
    assert!(first.output.contains("v1"));

    fs::write(&helper_path, "export const value = \"v2\";\n")?;

    let second = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log(firstValue);\nconst { value: secondValue } = await import(\"./helper.js\");\nconsole.log(secondValue);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
    assert!(second.output.contains("v1"));
    assert!(second.output.contains("v2"));
    Ok(())
}

#[tokio::test]
async fn js_repl_reimports_local_files_after_fixing_failure() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let helper_path = cwd_dir.path().join("broken.js");
    fs::write(&helper_path, "throw new Error(\"boom\");\n")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./broken.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected broken module import to fail");
    assert!(err.to_string().contains("boom"));

    fs::write(&helper_path, "export const value = \"fixed\";\n")?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log((await import(\"./broken.js\")).value);".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    assert!(result.output.contains("fixed"));
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_expose_node_like_import_meta() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let pkg_dir = cwd_dir.path().join("node_modules").join("repl_meta_pkg");
    fs::create_dir_all(&pkg_dir)?;
    fs::write(
        pkg_dir.join("package.json"),
        "{\n  \"name\": \"repl_meta_pkg\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {\n    \"import\": \"./index.js\"\n  }\n}\n",
    )?;
    fs::write(
        pkg_dir.join("index.js"),
        "import { sep } from \"node:path\";\nexport const value = `pkg:${typeof sep}`;\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "child.js",
        "export const value = \"child-export\";\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "meta.js",
        "console.log(import.meta.url);\nconsole.log(import.meta.filename);\nconsole.log(import.meta.dirname);\nconsole.log(import.meta.main);\nconsole.log(import.meta.resolve(\"./child.js\"));\nconsole.log(import.meta.resolve(\"repl_meta_pkg\"));\nconsole.log(import.meta.resolve(\"node:fs\"));\nconsole.log((await import(import.meta.resolve(\"./child.js\"))).value);\nconsole.log((await import(import.meta.resolve(\"repl_meta_pkg\"))).value);\n",
    )?;
    let child_path = fs::canonicalize(cwd_dir.path().join("child.js"))?;
    let child_url = url::Url::from_file_path(&child_path)
        .expect("child path should convert to file URL")
        .to_string();

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./meta.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await?;
    let cwd_display = cwd_dir.path().display().to_string();
    let meta_path_display = cwd_dir.path().join("meta.js").display().to_string();
    assert!(result.output.contains("file://"));
    assert!(result.output.contains(&meta_path_display));
    assert!(result.output.contains(&cwd_display));
    assert!(result.output.contains("false"));
    assert!(result.output.contains(&child_url));
    assert!(result.output.contains("repl_meta_pkg"));
    assert!(result.output.contains("node:fs"));
    assert!(result.output.contains("child-export"));
    assert!(result.output.contains("pkg:string"));
    Ok(())
}

#[tokio::test]
async fn js_repl_rejects_top_level_static_imports_with_clear_error() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "import \"./local.js\";".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected top-level static import to be rejected");
    assert!(
        err.to_string()
            .contains("Top-level static import \"./local.js\" is not supported in js_repl")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_reject_static_bare_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_package(cwd_dir.path(), "repl_counter", "pkg")?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "entry.js",
        "import { value } from \"repl_counter\";\nconsole.log(value);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./entry.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected static bare import to be rejected");
    assert!(
        err.to_string()
            .contains("Static import \"repl_counter\" is not supported from js_repl local files")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_rejects_unsupported_file_specifiers() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(cwd_dir.path(), "local.ts", "export const value = \"ts\";\n")?;
    write_js_repl_test_module(cwd_dir.path(), "local", "export const value = \"noext\";\n")?;
    fs::create_dir_all(cwd_dir.path().join("dir"))?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let unsupported_extension = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./local.ts\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected unsupported extension to be rejected");
    assert!(
        unsupported_extension
            .to_string()
            .contains("Only .js and .mjs files are supported")
    );

    let extensionless = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./local\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected extensionless import to be rejected");
    assert!(
        extensionless
            .to_string()
            .contains("Only .js and .mjs files are supported")
    );

    let directory = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./dir\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected directory import to be rejected");
    assert!(
        directory
            .to_string()
            .contains("Directory imports are not supported")
    );

    let unsupported_url = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"https://example.com/test.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected unsupported url import to be rejected");
    assert!(
        unsupported_url
            .to_string()
            .contains("Unsupported import specifier")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_blocks_sensitive_builtin_imports_from_local_files() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "blocked.js",
        "import process from \"node:process\";\nconsole.log(process.pid);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.path().to_path_buf();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./blocked.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected blocked builtin import to be rejected");
    assert!(
        err.to_string()
            .contains("Importing module \"node:process\" is not allowed in js_repl")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_do_not_escape_node_module_search_roots() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let parent_dir = tempdir()?;
    write_js_repl_test_package(parent_dir.path(), "repl_probe", "parent")?;
    let cwd_dir = parent_dir.path().join("workspace");
    fs::create_dir_all(&cwd_dir)?;
    write_js_repl_test_module(
        &cwd_dir,
        "entry.js",
        "const { value } = await import(\"repl_probe\");\nconsole.log(value);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.clone();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./entry.js\");".to_string(),
                timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
            },
        )
        .await
        .expect_err("expected parent node_modules lookup to be rejected");
    assert!(err.to_string().contains("repl_probe"));
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_does_not_auto_timeout_running_execs() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-timeout".to_string(),
            JsReplArgs {
                code: "await new Promise((resolve) => setTimeout(resolve, 5_000));".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let no_timeout_deadline = Instant::now() + Duration::from_millis(800);
    while Instant::now() < no_timeout_deadline {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        assert!(
            !result.done,
            "polling exec should remain running without reset"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    manager.reset_session(&submission.session_id).await?;

    let cancel_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        if result.done {
            assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
            break;
        }
        if Instant::now() >= cancel_deadline {
            panic!("timed out waiting for reset cancellation");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_session_cancels_inflight_tool_call_promptly() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let started_marker = turn.cwd.join(format!(
        "js-repl-poll-reset-timeout-race-started-{}.txt",
        Uuid::new_v4()
    ));
    let done_marker = turn.cwd.join(format!(
        "js-repl-poll-reset-timeout-race-done-{}.txt",
        Uuid::new_v4()
    ));
    let started_json = serde_json::to_string(&started_marker.to_string_lossy().to_string())?;
    let done_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
    let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-timeout-race".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_json};
const done = {done_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8; printf done > "${{done}}"` }});
console.log("unexpected");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

    let started_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::fs::metadata(&started_marker).await.is_ok() {
            break;
        }
        if Instant::now() >= started_deadline {
            panic!("timed out waiting for in-flight tool call to start");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    tokio::time::timeout(
        Duration::from_secs(2),
        manager.reset_session(&submission.session_id),
    )
    .await
    .expect("reset_session should complete promptly")
    .expect("reset_session should succeed");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        if result.done {
            assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for reset_session cancellation completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let _ = tokio::fs::remove_file(&started_marker).await;
    let _ = tokio::fs::remove_file(&done_marker).await;

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_all_cancels_inflight_tool_call_promptly() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let started_marker = turn.cwd.join(format!(
        "js-repl-poll-reset-all-timeout-race-started-{}.txt",
        Uuid::new_v4()
    ));
    let done_marker = turn.cwd.join(format!(
        "js-repl-poll-reset-all-timeout-race-done-{}.txt",
        Uuid::new_v4()
    ));
    let started_json = serde_json::to_string(&started_marker.to_string_lossy().to_string())?;
    let done_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
    let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-all-timeout-race".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_json};
const done = {done_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8; printf done > "${{done}}"` }});
console.log("unexpected");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

    let started_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::fs::metadata(&started_marker).await.is_ok() {
            break;
        }
        if Instant::now() >= started_deadline {
            panic!("timed out waiting for in-flight tool call to start");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    tokio::time::timeout(Duration::from_secs(2), manager.reset())
        .await
        .expect("reset should complete promptly")
        .expect("reset should succeed");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        if result.done {
            assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for reset-all cancellation completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let _ = tokio::fs::remove_file(&started_marker).await;
    let _ = tokio::fs::remove_file(&done_marker).await;

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_session_cancels_only_target_session_tool_calls() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let started_a = turn
        .cwd
        .join(format!("js-repl-poll-reset-scope-a-{}.txt", Uuid::new_v4()));
    let started_b = turn
        .cwd
        .join(format!("js-repl-poll-reset-scope-b-{}.txt", Uuid::new_v4()));
    let started_a_json = serde_json::to_string(&started_a.to_string_lossy().to_string())?;
    let started_b_json = serde_json::to_string(&started_b.to_string_lossy().to_string())?;

    let session_a = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-reset-scope-a".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
const started = {started_a_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8` }});
console.log("session-a-complete");
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let session_b = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-reset-scope-b".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
const started = {started_b_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 0.4` }});
console.log("session-b-complete");
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let started_deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_started_a = false;
    let mut saw_started_b = false;
    while !(saw_started_a && saw_started_b) {
        if tokio::fs::metadata(&started_a).await.is_ok() {
            saw_started_a = true;
        }
        if tokio::fs::metadata(&started_b).await.is_ok() {
            saw_started_b = true;
        }
        if Instant::now() >= started_deadline {
            panic!("timed out waiting for both sessions to start tool calls");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    tokio::time::timeout(
        Duration::from_secs(2),
        manager.reset_session(&session_a.session_id),
    )
    .await
    .expect("session-scoped reset should complete promptly")
    .expect("session-scoped reset should succeed");

    let session_a_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = manager.poll(&session_a.exec_id, Some(200)).await?;
        if result.done {
            assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
            break;
        }
        if Instant::now() >= session_a_deadline {
            panic!("timed out waiting for target session cancellation");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let session_b_deadline = Instant::now() + Duration::from_secs(8);
    let mut session_b_logs = Vec::new();
    loop {
        let result = manager.poll(&session_b.exec_id, Some(200)).await?;
        session_b_logs.extend(result.logs.iter().cloned());
        if result.done {
            assert_eq!(result.error, None);
            assert!(
                session_b_logs
                    .iter()
                    .any(|line| line.contains("session-b-complete"))
            );
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= session_b_deadline {
            panic!("timed out waiting for non-target session completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let _ = tokio::fs::remove_file(&started_a).await;
    let _ = tokio::fs::remove_file(&started_b).await;
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_unawaited_tool_result_preserves_session() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    let done_marker = turn.cwd.join(format!(
        "js-repl-poll-unawaited-done-{}.txt",
        Uuid::new_v4()
    ));
    let done_marker_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
    let first = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-unawaited-timeout-race".to_string(),
            JsReplArgs {
                code: format!(
                    r#"
let persisted = 7;
const done = {done_marker_json};
void codex.tool("shell_command", {{ command: `sleep 0.35; printf done > "${{done}}"` }});
console.log("main-complete");
"#
                ),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    let first_deadline = Instant::now() + Duration::from_secs(6);
    let mut first_logs = Vec::new();
    loop {
        let result = manager.poll(&first.exec_id, Some(200)).await?;
        first_logs.extend(result.logs.iter().cloned());
        if result.done {
            assert_eq!(result.error, None);
            assert!(
                first_logs.iter().any(|line| line.contains("main-complete")),
                "first exec should complete successfully before timeout teardown"
            );
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= first_deadline {
            panic!("timed out waiting for first exec completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let marker_deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if tokio::fs::metadata(&done_marker).await.is_ok() {
            break;
        }
        if Instant::now() >= marker_deadline {
            panic!("timed out waiting for unawaited tool call completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let second = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
            "call-unawaited-timeout-race-reuse".to_string(),
            JsReplArgs {
                code: "console.log(persisted);".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: Some(first.session_id.clone()),
            },
        )
        .await?;
    assert_eq!(second.session_id, first.session_id);

    let second_deadline = Instant::now() + Duration::from_secs(6);
    let mut second_logs = Vec::new();
    loop {
        let result = manager.poll(&second.exec_id, Some(200)).await?;
        second_logs.extend(result.logs.iter().cloned());
        if result.done {
            assert_eq!(result.error, None);
            assert!(
                second_logs.iter().any(|line| line.contains("7")),
                "session should remain reusable after first exec completion"
            );
            assert_eq!(result.final_output.as_deref(), Some(""));
            break;
        }
        if Instant::now() >= second_deadline {
            panic!("timed out waiting for second exec completion");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let _ = tokio::fs::remove_file(&done_marker).await;
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_session_marks_exec_canceled() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let manager = turn.js_repl.manager().await?;

    for attempt in 0..4 {
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                format!("call-cancel-{attempt}"),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 10_000));"
                        .to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        tokio::time::sleep(Duration::from_millis(100)).await;
        manager.reset_session(&submission.session_id).await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                let err = result.error.as_deref();
                assert_eq!(err, Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                assert!(!err.is_some_and(|message| message.contains("kernel exited unexpectedly")));
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll reset completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    Ok(())
}

#[tokio::test]
async fn js_repl_reset_session_rejects_unknown_session_id() -> anyhow::Result<()> {
    let (_session, turn) = make_session_and_context().await;
    let manager = turn.js_repl.manager().await?;
    let err = manager
        .reset_session("missing-session")
        .await
        .expect_err("expected missing session id error");
    assert_eq!(err.to_string(), "js_repl session id not found");
    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_marks_running_exec_canceled() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-reset".to_string(),
            JsReplArgs {
                code: "await new Promise((resolve) => setTimeout(resolve, 10_000));".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.reset().await?;

    let result = manager.poll(&submission.exec_id, Some(200)).await?;
    assert!(result.done);
    assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_reset_emits_exec_end_for_running_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn, rx) = crate::codex::make_session_and_context_with_rx().await;
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let submission = Arc::clone(&manager)
        .submit(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            "call-reset-end".to_string(),
            JsReplArgs {
                code: "await new Promise((resolve) => setTimeout(resolve, 10_000));".to_string(),
                timeout_ms: None,
                poll: true,
                session_id: None,
            },
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.reset().await?;

    let end = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("event");
            if let EventMsg::ExecCommandEnd(end) = event.msg
                && end.call_id == "call-reset-end"
            {
                break end;
            }
        }
    })
    .await
    .expect("timed out waiting for js_repl reset exec end event");
    assert_eq!(end.stderr, JS_REPL_CANCEL_ERROR_MESSAGE);

    let result = manager.poll(&submission.exec_id, Some(200)).await?;
    assert!(result.done);
    assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));

    Ok(())
}

#[tokio::test]
async fn js_repl_poll_rejects_unknown_exec_id() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (_session, turn) = make_session_and_context().await;
    let manager = turn.js_repl.manager().await?;
    let err = manager
        .poll("missing-exec-id", Some(50))
        .await
        .expect_err("expected missing exec id error");
    assert_eq!(err.to_string(), "js_repl exec id not found");
    Ok(())
}
