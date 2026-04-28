use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::model::ConversationItemKind;
use crate::model::ExecutionStatus;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::append_inference_start;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::message;
use crate::reducer::test_support::start_turn;
use crate::replay_bundle;

#[test]
fn cancelled_inference_reduces_partial_response_items() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "draft")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let partial_response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": null,
            "token_usage": null,
            "output_items": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "partial"}]
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceCancelled {
        inference_call_id: "inference-1".to_string(),
        upstream_request_id: Some("req-cancelled".to_string()),
        reason: "test interruption".to_string(),
        partial_response_payload: Some(partial_response),
    })?;

    let rollout = replay_bundle(temp.path())?;
    let inference = &rollout.inference_calls["inference-1"];
    let response_item_id = &inference.response_item_ids[0];

    assert_eq!(inference.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(
        inference.upstream_request_id,
        Some("req-cancelled".to_string()),
    );
    assert_eq!(inference.response_item_ids.len(), 1);
    assert_eq!(
        rollout.conversation_items[response_item_id].kind,
        ConversationItemKind::Message,
    );
    assert_eq!(
        rollout.conversation_items[response_item_id].produced_by,
        vec![crate::model::ProducerRef::Inference {
            inference_call_id: "inference-1".to_string(),
        }],
    );

    Ok(())
}

#[test]
fn cancelled_turn_closes_running_inference_call() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "wait")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;
    let turn_end = writer.append(RawTraceEventPayload::CodexTurnEnded {
        codex_turn_id: "turn-1".to_string(),
        status: ExecutionStatus::Cancelled,
    })?;

    let rollout = replay_bundle(temp.path())?;
    let inference = &rollout.inference_calls["inference-1"];

    assert_eq!(inference.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(inference.execution.ended_seq, Some(turn_end.seq));

    Ok(())
}

#[test]
fn late_cancelled_inference_preserves_turn_end_status() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "interrupt")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;
    let turn_end = writer.append(RawTraceEventPayload::CodexTurnEnded {
        codex_turn_id: "turn-1".to_string(),
        status: ExecutionStatus::Failed,
    })?;

    let partial_response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": null,
            "token_usage": null,
            "output_items": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "late partial"}]
            }]
        }),
    )?;
    writer.append(RawTraceEventPayload::InferenceCancelled {
        inference_call_id: "inference-1".to_string(),
        upstream_request_id: Some("req-late-cancelled".to_string()),
        reason: "stream mapper noticed cancellation after turn end".to_string(),
        partial_response_payload: Some(partial_response.clone()),
    })?;

    let rollout = replay_bundle(temp.path())?;
    let inference = &rollout.inference_calls["inference-1"];
    assert_eq!(inference.execution.status, ExecutionStatus::Failed);
    assert_eq!(inference.execution.ended_seq, Some(turn_end.seq));
    assert_eq!(
        inference.raw_response_payload_id,
        Some(partial_response.raw_payload_id),
    );
    assert_eq!(
        inference.upstream_request_id,
        Some("req-late-cancelled".to_string()),
    );
    assert_eq!(inference.response_item_ids.len(), 1);
    let response_item_id = &inference.response_item_ids[0];
    assert_eq!(
        rollout.conversation_items[response_item_id].body.parts,
        vec![crate::model::ConversationPart::Text {
            text: "late partial".to_string(),
        }],
    );

    Ok(())
}
