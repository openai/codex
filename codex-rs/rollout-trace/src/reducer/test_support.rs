//! Shared reducer test fixtures.
//!
//! These helpers only write common trace scaffolding. Scenario-specific event
//! sequences stay in each test so the behavior under test remains visible.

use serde_json::json;
use tempfile::TempDir;

use crate::model::ToolCallSummary;
use crate::payload::RawPayloadKind;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::replay_bundle;
use crate::writer::TraceWriter;

pub(crate) const ROOT_THREAD_ID: &str = "thread-root";
pub(crate) const AGENT_ROOT_THREAD_ID: &str = "019d0000-0000-7000-8000-000000000001";

pub(crate) fn message(role: &str, text: &str) -> serde_json::Value {
    json!({
        "type": "message",
        "role": role,
        "content": [{"type": "input_text", "text": text}]
    })
}

pub(crate) fn generic_summary(label: &str) -> ToolCallSummary {
    ToolCallSummary::Generic {
        label: label.to_string(),
        input_preview: None,
        output_preview: None,
    }
}

pub(crate) fn create_started_writer(temp: &TempDir) -> anyhow::Result<TraceWriter> {
    create_started_writer_for_thread(temp, ROOT_THREAD_ID, "/root")
}

pub(crate) fn create_started_agent_writer(temp: &TempDir) -> anyhow::Result<TraceWriter> {
    create_started_writer_for_thread(temp, AGENT_ROOT_THREAD_ID, "/root")
}

pub(crate) fn create_started_writer_for_thread(
    temp: &TempDir,
    thread_id: &str,
    agent_path: &str,
) -> anyhow::Result<TraceWriter> {
    let writer = TraceWriter::create(
        temp.path(),
        "trace-1".to_string(),
        "rollout-1".to_string(),
        thread_id.to_string(),
    )?;
    start_thread(&writer, thread_id, agent_path)?;
    Ok(writer)
}

pub(crate) fn start_thread(
    writer: &TraceWriter,
    thread_id: &str,
    agent_path: &str,
) -> anyhow::Result<()> {
    writer.append(RawTraceEventPayload::ThreadStarted {
        thread_id: thread_id.to_string(),
        agent_path: agent_path.to_string(),
        metadata_payload: None,
    })?;
    Ok(())
}

pub(crate) fn start_turn(writer: &TraceWriter, turn_id: &str) -> anyhow::Result<()> {
    start_turn_for_thread(writer, ROOT_THREAD_ID, turn_id)
}

pub(crate) fn start_agent_turn(writer: &TraceWriter, turn_id: &str) -> anyhow::Result<()> {
    start_turn_for_thread(writer, AGENT_ROOT_THREAD_ID, turn_id)
}

pub(crate) fn start_turn_for_thread(
    writer: &TraceWriter,
    thread_id: &str,
    turn_id: &str,
) -> anyhow::Result<()> {
    writer.append(RawTraceEventPayload::CodexTurnStarted {
        codex_turn_id: turn_id.to_string(),
        thread_id: thread_id.to_string(),
    })?;
    Ok(())
}

pub(crate) fn trace_context(turn_id: &str) -> RawTraceEventContext {
    trace_context_for_thread(ROOT_THREAD_ID, turn_id)
}

pub(crate) fn trace_context_for_agent(turn_id: &str) -> RawTraceEventContext {
    trace_context_for_thread(AGENT_ROOT_THREAD_ID, turn_id)
}

pub(crate) fn trace_context_for_thread(thread_id: &str, turn_id: &str) -> RawTraceEventContext {
    RawTraceEventContext {
        thread_id: Some(thread_id.to_string()),
        codex_turn_id: Some(turn_id.to_string()),
    }
}

pub(crate) fn append_inference_start(
    writer: &TraceWriter,
    inference_call_id: &str,
    codex_turn_id: &str,
    request_payload: RawPayloadRef,
) -> anyhow::Result<()> {
    append_inference_start_for_thread(
        writer,
        ROOT_THREAD_ID,
        codex_turn_id,
        inference_call_id,
        request_payload,
    )
}

pub(crate) fn append_inference_start_for_thread(
    writer: &TraceWriter,
    thread_id: &str,
    codex_turn_id: &str,
    inference_call_id: &str,
    request_payload: RawPayloadRef,
) -> anyhow::Result<()> {
    writer.append(RawTraceEventPayload::InferenceStarted {
        inference_call_id: inference_call_id.to_string(),
        thread_id: thread_id.to_string(),
        codex_turn_id: codex_turn_id.to_string(),
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        request_payload,
    })?;
    Ok(())
}

pub(crate) fn append_inference_completion(
    writer: &TraceWriter,
    inference_call_id: &str,
    response_id: &str,
    response_payload: RawPayloadRef,
) -> anyhow::Result<()> {
    writer.append(RawTraceEventPayload::InferenceCompleted {
        inference_call_id: inference_call_id.to_string(),
        response_id: Some(response_id.to_string()),
        upstream_request_id: None,
        response_payload,
    })?;
    Ok(())
}

pub(crate) fn append_inference_request(
    writer: &TraceWriter,
    thread_id: &str,
    turn_id: &str,
    inference_id: &str,
    input: Vec<serde_json::Value>,
) -> anyhow::Result<()> {
    let request =
        writer.write_json_payload(RawPayloadKind::InferenceRequest, &json!({ "input": input }))?;
    append_inference_start_for_thread(writer, thread_id, turn_id, inference_id, request)
}

pub(crate) fn append_completed_inference(
    writer: &TraceWriter,
    thread_id: &str,
    turn_id: &str,
    inference_id: &str,
    input: Vec<serde_json::Value>,
    output_items: Vec<serde_json::Value>,
) -> anyhow::Result<()> {
    append_inference_request(writer, thread_id, turn_id, inference_id, input)?;
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": format!("resp-{inference_id}"),
            "output_items": output_items,
        }),
    )?;
    writer.append_with_context(
        trace_context_for_thread(thread_id, turn_id),
        RawTraceEventPayload::InferenceCompleted {
            inference_call_id: inference_id.to_string(),
            response_id: Some(format!("resp-{inference_id}")),
            upstream_request_id: None,
            response_payload: response,
        },
    )?;
    Ok(())
}

pub(crate) fn expect_replay_error(temp: &TempDir, expected: &str) -> anyhow::Result<()> {
    let Err(err) = replay_bundle(temp.path()) else {
        panic!("expected replay error containing {expected}");
    };
    let message = err.to_string();
    assert!(message.contains(expected), "unexpected error: {message}");
    Ok(())
}
