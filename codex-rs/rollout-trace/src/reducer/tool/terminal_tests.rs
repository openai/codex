use std::sync::Arc;

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::ToolDispatchInvocation;
use crate::ToolDispatchPayload;
use crate::ToolDispatchRequester;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::model::TerminalModelObservation;
use crate::model::TerminalObservationSource;
use crate::model::TerminalOperation;
use crate::model::TerminalOperationKind;
use crate::model::TerminalRequest;
use crate::model::TerminalResult;
use crate::model::TerminalSession;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::ROOT_THREAD_ID;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::generic_summary;
use crate::reducer::test_support::message;
use crate::reducer::test_support::start_thread;
use crate::reducer::test_support::start_turn;
use crate::reducer::test_support::start_turn_for_thread;
use crate::reducer::test_support::trace_context;
use crate::reducer::test_support::trace_context_for_thread;
use crate::replay_bundle;
use crate::tool_dispatch::ToolDispatchTraceContext;
use crate::writer::TraceWriter;

#[test]
fn exec_tool_reduces_to_terminal_operation_and_session() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    append_inference_with_tool_call(&writer)?;

    let invocation_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "exec_command",
            "tool_namespace": null,
            "payload": {
                "type": "function",
                "arguments": "{\"cmd\":\"cargo test\"}"
            }
        }),
    )?;
    let invocation_payload_id = invocation_payload.raw_payload_id.clone();
    let _tool_start = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "tool-1".to_string(),
            model_visible_call_id: Some("call-1".to_string()),
            code_mode_runtime_tool_id: None,
            requester: crate::raw_event::RawToolCallRequester::Model,
            kind: ToolCallKind::ExecCommand,
            summary: generic_summary("exec_command"),
            invocation_payload: Some(invocation_payload),
        },
    )?;

    let runtime_start_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "tool-1",
            "turn_id": "turn-1",
            "command": ["cargo", "test"],
            "cwd": "/repo"
        }),
    )?;
    let runtime_start_payload_id = runtime_start_payload.raw_payload_id.clone();
    let runtime_start = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeStarted {
            kind: ToolCallKind::ExecCommand,
            tool_call_id: "tool-1".to_string(),
            runtime_payload: runtime_start_payload,
        },
    )?;

    let runtime_end_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "tool-1",
            "process_id": "pty-1",
            "turn_id": "turn-1",
            "command": ["cargo", "test"],
            "cwd": "/repo",
            "aggregated_output": "ok\n",
            "exit_code": 0,
            "status": "completed"
        }),
    )?;
    let runtime_end_payload_id = runtime_end_payload.raw_payload_id.clone();
    let runtime_end = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: "tool-1".to_string(),
            status: ExecutionStatus::Completed,
            runtime_payload: runtime_end_payload,
        },
    )?;

    let result_payload = writer.write_json_payload(
        RawPayloadKind::ToolResult,
        &json!({
            "type": "direct_response",
            "response_item": {
                "type": "function_call_output",
                "call_id": "call-1",
                "output": "ok\n"
            }
        }),
    )?;
    let result_payload_id = result_payload.raw_payload_id.clone();
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "tool-1".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: Some(result_payload),
        },
    )?;

    start_turn(&writer, "turn-2")?;
    append_followup_with_tool_output(&writer)?;

    let rollout = replay_bundle(temp.path())?;
    let operation_id = "terminal_operation:1".to_string();
    let output_item_id = rollout.inference_calls["inference-2"]
        .request_item_ids
        .last()
        .expect("tool output item")
        .clone();

    assert_eq!(
        rollout.tool_calls["tool-1"].terminal_operation_id,
        Some(operation_id.clone()),
    );
    assert_eq!(
        rollout.tool_calls["tool-1"].raw_invocation_payload_id,
        Some(invocation_payload_id),
    );
    assert_eq!(
        rollout.tool_calls["tool-1"].raw_result_payload_id,
        Some(result_payload_id),
    );
    assert_eq!(
        rollout.tool_calls["tool-1"].raw_runtime_payload_ids,
        vec![
            runtime_start_payload_id.clone(),
            runtime_end_payload_id.clone()
        ],
    );
    assert_eq!(
        rollout.tool_calls["tool-1"].summary,
        ToolCallSummary::Terminal {
            operation_id: operation_id.clone(),
        },
    );
    assert_eq!(
        rollout.terminal_operations[&operation_id],
        TerminalOperation {
            operation_id: operation_id.clone(),
            terminal_id: Some("terminal_session:terminal_operation:1".to_string()),
            tool_call_id: "tool-1".to_string(),
            kind: TerminalOperationKind::ExecCommand,
            execution: ExecutionWindow {
                started_at_unix_ms: runtime_start.wall_time_unix_ms,
                started_seq: runtime_start.seq,
                ended_at_unix_ms: Some(runtime_end.wall_time_unix_ms),
                ended_seq: Some(runtime_end.seq),
                status: ExecutionStatus::Completed,
            },
            request: TerminalRequest::ExecCommand {
                command: vec!["cargo".to_string(), "test".to_string()],
                display_command: "cargo test".to_string(),
                cwd: "/repo".to_string(),
                yield_time_ms: None,
                max_output_tokens: None,
            },
            result: Some(TerminalResult {
                exit_code: Some(0),
                stdout: "ok\n".to_string(),
                stderr: String::new(),
                formatted_output: None,
                original_token_count: None,
                chunk_id: None,
            }),
            model_observations: vec![TerminalModelObservation {
                call_item_ids: rollout.inference_calls["inference-1"]
                    .response_item_ids
                    .clone(),
                output_item_ids: vec![output_item_id],
                source: TerminalObservationSource::DirectToolCall,
            }],
            raw_payload_ids: vec![runtime_start_payload_id, runtime_end_payload_id],
        },
    );
    assert_eq!(
        rollout.terminal_sessions["terminal_session:terminal_operation:1"],
        TerminalSession {
            terminal_id: "terminal_session:terminal_operation:1".to_string(),
            thread_id: "thread-root".to_string(),
            created_by_operation_id: operation_id.clone(),
            operation_ids: vec![operation_id],
            execution: ExecutionWindow {
                started_at_unix_ms: runtime_start.wall_time_unix_ms,
                started_seq: runtime_start.seq,
                ended_at_unix_ms: None,
                ended_seq: None,
                status: ExecutionStatus::Running,
            },
        },
    );

    Ok(())
}

#[test]
fn reused_process_ids_keep_sessions_and_stdin_joins_distinct() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    start_thread(&writer, "child", "/root/child")?;
    start_turn_for_thread(&writer, "child", "child-turn")?;
    let mut reused_stdin = None;

    // Real rollouts reuse process IDs both across threads and after an exec
    // finishes. Polls must stay with the corresponding exec, not the last
    // occurrence of the process number anywhere in the rollout.
    for (index, (thread, turn, kind)) in [
        ("thread-root", "turn-1", ToolCallKind::ExecCommand),
        ("child", "child-turn", ToolCallKind::ExecCommand),
        ("thread-root", "turn-1", ToolCallKind::WriteStdin),
        ("child", "child-turn", ToolCallKind::WriteStdin),
        ("thread-root", "turn-1", ToolCallKind::ExecCommand),
        ("thread-root", "turn-1", ToolCallKind::WriteStdin),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 4 {
            let response = writer.write_json_payload(
                RawPayloadKind::ToolRuntimeEvent,
                &json!({"process_id": "123", "exit_code": 0}),
            )?;
            writer.append_with_context(
                trace_context("turn-1"),
                RawTraceEventPayload::ToolCallRuntimeEnded {
                    tool_call_id: "tool-0".to_string(),
                    status: ExecutionStatus::Completed,
                    runtime_payload: response,
                },
            )?;
        }
        let tool_call_id = format!("tool-{index}");
        writer.append_with_context(
            trace_context_for_thread(thread, turn),
            RawTraceEventPayload::ToolCallStarted {
                tool_call_id: tool_call_id.clone(),
                model_visible_call_id: None,
                code_mode_runtime_tool_id: None,
                requester: crate::raw_event::RawToolCallRequester::Model,
                kind: kind.clone(),
                summary: generic_summary("terminal"),
                invocation_payload: None,
            },
        )?;
        let request = writer.write_json_payload(
            RawPayloadKind::ToolRuntimeEvent,
            &json!({
                "process_id": "123",
                "command": ["bash"],
                "cwd": "/repo",
                "interaction_input": (index == 5).then_some("echo hi\n"),
            }),
        )?;
        let request_id = request.raw_payload_id.clone();
        let runtime_start = writer.append_with_context(
            trace_context_for_thread(thread, turn),
            RawTraceEventPayload::ToolCallRuntimeStarted {
                kind,
                tool_call_id,
                runtime_payload: request,
            },
        )?;
        if index == 5 {
            reused_stdin = Some((runtime_start, request_id));
        }
    }

    let rollout = replay_bundle(temp.path())?;
    let groups = rollout
        .terminal_sessions
        .values()
        .map(|session| {
            let calls = session
                .operation_ids
                .iter()
                .map(|id| {
                    let operation = &rollout.terminal_operations[id];
                    assert_eq!(operation.terminal_id.as_ref(), Some(&session.terminal_id));
                    operation.tool_call_id.as_str()
                })
                .collect::<Vec<_>>();
            (session.thread_id.as_str(), calls)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        groups,
        vec![
            ("thread-root", vec!["tool-0", "tool-2"]),
            ("child", vec!["tool-1", "tool-3"]),
            ("thread-root", vec!["tool-4", "tool-5"]),
        ],
    );
    let (runtime_start, request_id) = reused_stdin.unwrap();
    assert_eq!(
        rollout.terminal_operations["terminal_operation:6"],
        TerminalOperation {
            operation_id: "terminal_operation:6".to_string(),
            terminal_id: Some("terminal_session:terminal_operation:5".to_string()),
            tool_call_id: "tool-5".to_string(),
            kind: TerminalOperationKind::WriteStdin,
            execution: ExecutionWindow {
                started_at_unix_ms: runtime_start.wall_time_unix_ms,
                started_seq: runtime_start.seq,
                ended_at_unix_ms: None,
                ended_seq: None,
                status: ExecutionStatus::Running,
            },
            request: TerminalRequest::WriteStdin {
                stdin: "echo hi\n".to_string(),
                yield_time_ms: None,
                max_output_tokens: None,
            },
            result: None,
            model_observations: Vec::new(),
            raw_payload_ids: vec![request_id],
        },
    );

    // Stdin started while tool-0 owned the handle, but dispatch was delayed
    // until tool-4 replaced it. Only its session membership should change.
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::StdinTargetResolved {
            tool_call_id: "tool-2".to_string(),
            exec_tool_call_id: "tool-4".to_string(),
        },
    )?;
    let mut expected = rollout;
    let previous = "terminal_session:terminal_operation:1";
    let resolved = "terminal_session:terminal_operation:5";
    expected
        .terminal_sessions
        .get_mut(previous)
        .unwrap()
        .operation_ids = vec!["terminal_operation:1".to_string()];
    expected
        .terminal_sessions
        .get_mut(resolved)
        .unwrap()
        .operation_ids
        .push("terminal_operation:3".to_string());
    expected
        .terminal_operations
        .get_mut("terminal_operation:3")
        .unwrap()
        .terminal_id = Some(resolved.to_string());
    assert_eq!(replay_bundle(temp.path())?, expected);
    Ok(())
}

#[test]
fn dispatch_write_stdin_payload_reduces_to_terminal_operation() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "write_stdin",
            "tool_namespace": null,
            "payload": {
                "type": "function",
                "arguments": json!({
                    "session_id": 123,
                    "chars": "echo hi\n",
                    "yield_time_ms": 250,
                    "max_output_tokens": 2000
                }).to_string()
            }
        }),
    )?;
    let request_payload_id = request_payload.raw_payload_id.clone();
    let tool_start = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "tool-stdin".to_string(),
            model_visible_call_id: Some("call-stdin".to_string()),
            code_mode_runtime_tool_id: None,
            requester: crate::raw_event::RawToolCallRequester::Model,
            kind: ToolCallKind::WriteStdin,
            summary: generic_summary("write_stdin"),
            invocation_payload: Some(request_payload),
        },
    )?;

    let response_payload = writer.write_json_payload(
        RawPayloadKind::ToolResult,
        &json!({
            "type": "direct_response",
            "response_item": {
                "type": "function_call_output",
                "call_id": "call-stdin",
                "output": "hi\n"
            }
        }),
    )?;
    let response_payload_id = response_payload.raw_payload_id.clone();
    let tool_end = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "tool-stdin".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: Some(response_payload),
        },
    )?;

    let rollout = replay_bundle(temp.path())?;
    let operation_id = "terminal_operation:1".to_string();

    assert_eq!(
        rollout.tool_calls["tool-stdin"].terminal_operation_id,
        Some(operation_id.clone()),
    );
    assert_eq!(
        rollout.tool_calls["tool-stdin"].summary,
        ToolCallSummary::Terminal {
            operation_id: operation_id.clone(),
        },
    );
    assert_eq!(
        rollout.terminal_operations[&operation_id],
        TerminalOperation {
            operation_id: operation_id.clone(),
            terminal_id: Some("terminal_session:terminal_operation:1".to_string()),
            tool_call_id: "tool-stdin".to_string(),
            kind: TerminalOperationKind::WriteStdin,
            execution: ExecutionWindow {
                started_at_unix_ms: tool_start.wall_time_unix_ms,
                started_seq: tool_start.seq,
                ended_at_unix_ms: Some(tool_end.wall_time_unix_ms),
                ended_seq: Some(tool_end.seq),
                status: ExecutionStatus::Completed,
            },
            request: TerminalRequest::WriteStdin {
                stdin: "echo hi\n".to_string(),
                yield_time_ms: Some(250),
                max_output_tokens: Some(2000),
            },
            result: Some(TerminalResult {
                exit_code: None,
                stdout: "hi\n".to_string(),
                stderr: String::new(),
                formatted_output: Some("hi\n".to_string()),
                original_token_count: None,
                chunk_id: None,
            }),
            model_observations: Vec::new(),
            raw_payload_ids: vec![request_payload_id, response_payload_id],
        },
    );
    assert_eq!(
        rollout.terminal_sessions["terminal_session:terminal_operation:1"],
        TerminalSession {
            terminal_id: "terminal_session:terminal_operation:1".to_string(),
            thread_id: "thread-root".to_string(),
            created_by_operation_id: operation_id.clone(),
            operation_ids: vec![operation_id],
            execution: ExecutionWindow {
                started_at_unix_ms: tool_start.wall_time_unix_ms,
                started_seq: tool_start.seq,
                ended_at_unix_ms: None,
                ended_seq: None,
                status: ExecutionStatus::Running,
            },
        },
    );

    Ok(())
}

#[test]
fn invalid_write_stdin_arguments_remain_failed_tool_calls() -> anyhow::Result<()> {
    for arguments in [r#"{"session_id": "#, "{}", r#"{"session_id": null}"#] {
        let temp = TempDir::new()?;
        let writer = Arc::new(create_started_writer(&temp)?);
        start_turn(&writer, "turn-1")?;
        let dispatch = ToolDispatchTraceContext::start(
            Arc::clone(&writer),
            ToolDispatchInvocation {
                thread_id: ROOT_THREAD_ID.to_string(),
                codex_turn_id: "turn-1".to_string(),
                tool_call_id: "call-stdin".to_string(),
                tool_name: "write_stdin".to_string(),
                tool_namespace: None,
                requester: ToolDispatchRequester::Model {
                    model_visible_call_id: "call-stdin".to_string(),
                },
                payload: ToolDispatchPayload::Function {
                    arguments: arguments.to_string(),
                },
            },
        );
        dispatch.record_failed("invalid arguments");

        let rollout = replay_bundle(temp.path())?;
        let call = &rollout.tool_calls["call-stdin"];
        assert_eq!(call.execution.status, ExecutionStatus::Failed);
        assert_eq!(call.terminal_operation_id, None);
        assert!(rollout.terminal_operations.is_empty());
        assert!(rollout.terminal_sessions.is_empty());
        // The failed call still retains its original arguments and error.
        for (id, expected) in [
            (
                call.raw_invocation_payload_id.as_ref().unwrap(),
                json!({"tool_name": "write_stdin", "tool_namespace": null,
                    "payload": {"type": "function", "arguments": arguments}}),
            ),
            (
                call.raw_result_payload_id.as_ref().unwrap(),
                json!({"type": "error", "error": "invalid arguments"}),
            ),
        ] {
            let payload = std::fs::read(temp.path().join(&rollout.raw_payloads[id].path))?;
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&payload)?,
                expected
            );
        }
    }
    Ok(())
}

#[test]
fn code_mode_write_stdin_result_projects_structured_exec_fields() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "write_stdin",
            "tool_namespace": null,
            "payload": {
                "type": "function",
                "arguments": json!({
                    "session_id": 456,
                    "chars": "",
                    "yield_time_ms": 1000,
                    "max_output_tokens": 4000
                }).to_string()
            }
        }),
    )?;
    let response_payload = writer.write_json_payload(
        RawPayloadKind::ToolResult,
        &json!({
            "type": "code_mode_response",
            "value": {
                "chunk_id": "abc123",
                "wall_time_seconds": 1.25,
                "exit_code": 0,
                "original_token_count": 3,
                "output": "done\n"
            }
        }),
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellStarted {
            runtime_cell_id: "cell-1".to_string(),
            model_visible_call_id: "call-code".to_string(),
            source_js: "await tools.write_stdin({ chars: '' })".to_string(),
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "tool-stdin".to_string(),
            model_visible_call_id: None,
            code_mode_runtime_tool_id: Some("runtime-tool-1".to_string()),
            requester: crate::raw_event::RawToolCallRequester::CodeCell {
                runtime_cell_id: "cell-1".to_string(),
            },
            kind: ToolCallKind::WriteStdin,
            summary: generic_summary("write_stdin"),
            invocation_payload: Some(request_payload),
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "tool-stdin".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: Some(response_payload),
        },
    )?;

    let rollout = replay_bundle(temp.path())?;
    assert_eq!(
        rollout.terminal_operations["terminal_operation:1"].result,
        Some(TerminalResult {
            exit_code: Some(0),
            stdout: "done\n".to_string(),
            stderr: String::new(),
            formatted_output: Some("done\n".to_string()),
            original_token_count: Some(3),
            chunk_id: Some("abc123".to_string()),
        }),
    );

    Ok(())
}

fn append_inference_with_tool_call(writer: &TraceWriter) -> anyhow::Result<()> {
    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "run tests")]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceStarted {
        inference_call_id: "inference-1".to_string(),
        thread_id: "thread-root".to_string(),
        codex_turn_id: "turn-1".to_string(),
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        request_payload: request,
    })?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [{
                "type": "function_call",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"cargo test\"}",
                "call_id": "call-1"
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceCompleted {
        inference_call_id: "inference-1".to_string(),
        response_id: Some("resp-1".to_string()),
        upstream_request_id: None,
        response_payload: response,
    })?;
    Ok(())
}

fn append_followup_with_tool_output(writer: &TraceWriter) -> anyhow::Result<()> {
    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "previous_response_id": "resp-1",
            "input": [{
                "type": "function_call_output",
                "call_id": "call-1",
                "output": "ok\n"
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceStarted {
        inference_call_id: "inference-2".to_string(),
        thread_id: "thread-root".to_string(),
        codex_turn_id: "turn-2".to_string(),
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        request_payload: request,
    })?;
    Ok(())
}
