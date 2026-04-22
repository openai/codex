use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::model::CodeCellRuntimeStatus;
use crate::model::ConversationItemKind;
use crate::model::ExecutionStatus;
use crate::model::ProducerRef;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawToolCallRequester;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::message;
use crate::reducer::test_support::start_turn;
use crate::reducer::test_support::start_turn_for_thread;
use crate::reducer::test_support::trace_context;
use crate::reducer::test_support::trace_context_for_thread;
use crate::replay_bundle;

#[test]
fn code_cell_lifecycle_links_nested_tools_waits_and_outputs() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "count files")]
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
                "type": "custom_tool_call",
                "name": "exec",
                "call_id": "call-code",
                "input": "text('hi')"
            }]
        }),
    )?;
    // Runtime tool dispatch starts before the stream-completion hook has
    // reduced the model response that requested `exec`.
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellStarted {
            runtime_cell_id: "1".to_string(),
            model_visible_call_id: "call-code".to_string(),
            source_js: "text('hi')".to_string(),
        },
    )?;
    writer.append(RawTraceEventPayload::InferenceCompleted {
        inference_call_id: "inference-1".to_string(),
        response_id: Some("resp-1".to_string()),
        response_payload: response,
    })?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellInitialResponse {
            runtime_cell_id: "1".to_string(),
            status: CodeCellRuntimeStatus::Yielded,
            response_payload: None,
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "nested-tool-1".to_string(),
            model_visible_call_id: None,
            code_mode_runtime_tool_id: Some("tool-1".to_string()),
            requester: RawToolCallRequester::CodeCell {
                runtime_cell_id: "1".to_string(),
            },
            kind: ToolCallKind::ExecCommand,
            summary: ToolCallSummary::Generic {
                label: "exec_command".to_string(),
                input_preview: Some("pwd".to_string()),
                output_preview: None,
            },
            invocation_payload: None,
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "nested-tool-1".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: None,
        },
    )?;

    start_turn(&writer, "turn-2")?;
    let followup = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "previous_response_id": "resp-1",
            "input": [{
                "type": "custom_tool_call_output",
                "call_id": "call-code",
                "output": "Script running with cell ID 1"
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceStarted {
        inference_call_id: "inference-2".to_string(),
        thread_id: "thread-root".to_string(),
        codex_turn_id: "turn-2".to_string(),
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        request_payload: followup,
    })?;
    let wait_request = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "wait",
            "tool_namespace": null,
            "payload": {
                "type": "function",
                "arguments": "{\"cell_id\":\"1\"}"
            }
        }),
    )?;
    writer.append_with_context(
        trace_context("turn-2"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "wait-tool-1".to_string(),
            model_visible_call_id: Some("wait-call".to_string()),
            code_mode_runtime_tool_id: None,
            requester: RawToolCallRequester::Model,
            kind: ToolCallKind::Other {
                name: "wait".to_string(),
            },
            summary: ToolCallSummary::Generic {
                label: "wait".to_string(),
                input_preview: Some("{\"cell_id\":\"1\"}".to_string()),
                output_preview: None,
            },
            invocation_payload: Some(wait_request),
        },
    )?;
    writer.append_with_context(
        trace_context("turn-2"),
        RawTraceEventPayload::CodeCellEnded {
            runtime_cell_id: "1".to_string(),
            status: CodeCellRuntimeStatus::Completed,
            response_payload: None,
        },
    )?;

    let rollout = replay_bundle(temp.path())?;
    let code_cell_id = test_reduced_code_cell_id("call-code");
    let cell = &rollout.code_cells[&code_cell_id];
    let output_item_id = rollout.inference_calls["inference-2"]
        .request_item_ids
        .last()
        .expect("exec output item");

    assert_eq!(cell.thread_id, "thread-root");
    assert_eq!(cell.runtime_status, CodeCellRuntimeStatus::Completed);
    assert_eq!(cell.execution.status, ExecutionStatus::Completed);
    assert_eq!(cell.runtime_cell_id, Some("1".to_string()));
    assert_eq!(cell.nested_tool_call_ids, vec!["nested-tool-1"]);
    assert_eq!(cell.wait_tool_call_ids, vec!["wait-tool-1"]);
    assert_eq!(cell.output_item_ids, vec![output_item_id.clone()]);
    assert_eq!(
        rollout.conversation_items[output_item_id].produced_by,
        vec![ProducerRef::CodeCell {
            code_cell_id: code_cell_id.clone(),
        }]
    );
    assert_eq!(
        rollout.conversation_items[&cell.source_item_id].kind,
        ConversationItemKind::CustomToolCall,
    );

    Ok(())
}

#[test]
fn fast_code_cell_lifecycle_waits_for_source_item() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "count files")]
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
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellStarted {
            runtime_cell_id: "1".to_string(),
            model_visible_call_id: "call-code".to_string(),
            source_js: "not valid js".to_string(),
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellInitialResponse {
            runtime_cell_id: "1".to_string(),
            status: CodeCellRuntimeStatus::Failed,
            response_payload: None,
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellEnded {
            runtime_cell_id: "1".to_string(),
            status: CodeCellRuntimeStatus::Failed,
            response_payload: None,
        },
    )?;
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [{
                "type": "custom_tool_call",
                "name": "exec",
                "call_id": "call-code",
                "input": "not valid js"
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceCompleted {
        inference_call_id: "inference-1".to_string(),
        response_id: Some("resp-1".to_string()),
        response_payload: response,
    })?;

    let rollout = replay_bundle(temp.path())?;
    let code_cell_id = test_reduced_code_cell_id("call-code");
    let cell = &rollout.code_cells[&code_cell_id];

    assert_eq!(cell.thread_id, "thread-root");
    assert_eq!(cell.runtime_status, CodeCellRuntimeStatus::Failed);
    assert_eq!(cell.execution.status, ExecutionStatus::Failed);
    assert_eq!(cell.runtime_cell_id, Some("1".to_string()));
    assert_eq!(
        rollout.conversation_items[&cell.source_item_id].kind,
        ConversationItemKind::CustomToolCall,
    );

    Ok(())
}

#[test]
fn cancelled_turn_terminates_unfinished_code_cell() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "count files")]
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
                "type": "custom_tool_call",
                "name": "exec",
                "call_id": "call-code",
                "input": "await tools.exec_command({cmd: 'slow'});"
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceCompleted {
        inference_call_id: "inference-1".to_string(),
        response_id: Some("resp-1".to_string()),
        response_payload: response,
    })?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodeCellStarted {
            runtime_cell_id: "1".to_string(),
            model_visible_call_id: "call-code".to_string(),
            source_js: "await tools.exec_command({cmd: 'slow'});".to_string(),
        },
    )?;
    let turn_end = writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CodexTurnEnded {
            codex_turn_id: "turn-1".to_string(),
            status: ExecutionStatus::Cancelled,
        },
    )?;

    let rollout = replay_bundle(temp.path())?;
    let code_cell_id = test_reduced_code_cell_id("call-code");
    let cell = &rollout.code_cells[&code_cell_id];

    assert_eq!(cell.runtime_status, CodeCellRuntimeStatus::Terminated);
    assert_eq!(cell.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(cell.execution.ended_seq, Some(turn_end.seq));

    Ok(())
}

#[test]
fn runtime_code_cell_ids_can_repeat_across_threads() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    writer.append(RawTraceEventPayload::ThreadStarted {
        thread_id: "thread-child".to_string(),
        agent_path: "/root/child".to_string(),
        metadata_payload: None,
    })?;
    start_turn_for_thread(&writer, "thread-root", "turn-root")?;
    start_turn_for_thread(&writer, "thread-child", "turn-child")?;

    for (thread_id, turn_id, inference_call_id, call_id) in [
        ("thread-root", "turn-root", "inference-root", "call-root"),
        (
            "thread-child",
            "turn-child",
            "inference-child",
            "call-child",
        ),
    ] {
        let request = writer.write_json_payload(
            RawPayloadKind::InferenceRequest,
            &json!({
                "input": [message("user", "run code")]
            }),
        )?;
        writer.append(RawTraceEventPayload::InferenceStarted {
            inference_call_id: inference_call_id.to_string(),
            thread_id: thread_id.to_string(),
            codex_turn_id: turn_id.to_string(),
            model: "gpt-test".to_string(),
            provider_name: "test-provider".to_string(),
            request_payload: request,
        })?;
        writer.append_with_context(
            trace_context_for_thread(thread_id, turn_id),
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id: "1".to_string(),
                model_visible_call_id: call_id.to_string(),
                source_js: "text('hi')".to_string(),
            },
        )?;
        let response = writer.write_json_payload(
            RawPayloadKind::InferenceResponse,
            &json!({
                "response_id": format!("resp-{thread_id}"),
                "output_items": [{
                    "type": "custom_tool_call",
                    "name": "exec",
                    "call_id": call_id,
                    "input": "text('hi')"
                }]
            }),
        )?;
        writer.append(RawTraceEventPayload::InferenceCompleted {
            inference_call_id: inference_call_id.to_string(),
            response_id: Some(format!("resp-{thread_id}")),
            response_payload: response,
        })?;
        writer.append_with_context(
            trace_context_for_thread(thread_id, turn_id),
            RawTraceEventPayload::CodeCellEnded {
                runtime_cell_id: "1".to_string(),
                status: CodeCellRuntimeStatus::Completed,
                response_payload: None,
            },
        )?;
    }

    let rollout = replay_bundle(temp.path())?;
    let root_cell_id = test_reduced_code_cell_id("call-root");
    let child_cell_id = test_reduced_code_cell_id("call-child");

    assert_eq!(rollout.code_cells[&root_cell_id].thread_id, "thread-root");
    assert_eq!(rollout.code_cells[&child_cell_id].thread_id, "thread-child");
    assert_eq!(
        rollout.code_cells[&root_cell_id].runtime_cell_id,
        Some("1".to_string())
    );
    assert_eq!(
        rollout.code_cells[&child_cell_id].runtime_cell_id,
        Some("1".to_string())
    );

    Ok(())
}

fn test_reduced_code_cell_id(model_visible_call_id: &str) -> String {
    format!("code_cell:{model_visible_call_id}")
}
