use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::model::ConversationChannel;
use crate::model::ConversationItemKind;
use crate::model::ConversationPart;
use crate::model::ExecutionStatus;
use crate::model::ProducerRef;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::append_inference_completion;
use crate::reducer::test_support::append_inference_start;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::expect_replay_error;
use crate::reducer::test_support::message;
use crate::reducer::test_support::start_turn;
use crate::reducer::test_support::trace_context;
use crate::replay_bundle;

#[test]
fn request_snapshots_reuse_history_without_deduping_new_identical_items() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let first_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "ok")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", first_request)?;
    start_turn(&writer, "turn-2")?;

    let second_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                message("user", "ok"),
                message("assistant", "ack"),
                message("user", "ok")
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", second_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"].request_item_ids;
    let second = &rollout.inference_calls["inference-2"].request_item_ids;

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 3);
    assert_eq!(second[0], first[0]);
    assert_ne!(second[2], first[0]);
    assert_eq!(rollout.conversation_items.len(), 3);
    assert_eq!(
        rollout.threads["thread-root"].conversation_item_ids,
        *second
    );

    Ok(())
}

#[test]
fn response_outputs_enter_thread_conversation_on_completion() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "run tests")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "tests passed"}]
                }
            ]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;

    let rollout = replay_bundle(temp.path())?;
    let inference = &rollout.inference_calls["inference-1"];
    let mut expected_thread_items = inference.request_item_ids.clone();
    expected_thread_items.extend(inference.response_item_ids.clone());

    assert_eq!(inference.response_item_ids.len(), 1);
    assert_eq!(
        rollout.threads["thread-root"].conversation_item_ids,
        expected_thread_items,
    );

    Ok(())
}

#[test]
fn later_full_request_reuses_prior_json_tool_call_by_position() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "run tests")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [{
                "type": "function_call",
                "name": "shell",
                "arguments": "{\"cmd\":\"cargo test\"}",
                "call_id": "call-1"
            }]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;
    start_turn(&writer, "turn-2")?;

    let next_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                message("user", "run tests"),
                {
                    "type": "function_call",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"cargo test\"}",
                    "call_id": "call-1"
                }
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", next_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];

    assert_eq!(
        second.request_item_ids,
        vec![
            first.request_item_ids[0].clone(),
            first.response_item_ids[0].clone(),
        ],
    );
    assert_eq!(rollout.conversation_items.len(), 2);

    Ok(())
}

#[test]
fn incremental_request_carries_prior_request_and_response_items_forward() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "run tests")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "token_usage": {
                "input_tokens": 10,
                "cached_input_tokens": 1,
                "output_tokens": 5,
                "reasoning_output_tokens": 2,
                "total_tokens": 15
            },
            "output_items": [
                {
                    "type": "function_call",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"cargo test\"}",
                    "call_id": "call-1"
                }
            ]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;
    start_turn(&writer, "turn-2")?;

    let incremental_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "type": "response.create",
            "previous_response_id": "resp-1",
            "input": [
                {
                    "type": "function_call_output",
                    "call_id": "call-1",
                    "output": "tests passed"
                }
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", incremental_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];

    assert_eq!(first.response_item_ids.len(), 1);
    assert_eq!(
        second.request_item_ids,
        vec![
            first.request_item_ids[0].clone(),
            first.response_item_ids[0].clone(),
            rollout.threads["thread-root"].conversation_item_ids[2].clone(),
        ],
    );
    assert_eq!(
        rollout.threads["thread-root"].conversation_item_ids,
        second.request_item_ids,
    );
    assert_eq!(
        first.usage.as_ref().map(|usage| usage.input_tokens),
        Some(10),
    );

    Ok(())
}

#[test]
fn full_request_snapshot_can_reorder_existing_items_and_insert_summary() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                message("developer", "follow the repo rules"),
                message("user", "count files")
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;
    start_turn(&writer, "turn-2")?;

    let compacted_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                message("user", "count files"),
                message("user", "summary from a compacted prior attempt"),
                message("developer", "follow the repo rules")
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", compacted_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"].request_item_ids;
    let second = &rollout.inference_calls["inference-2"].request_item_ids;

    assert_eq!(second[0], first[1]);
    assert_eq!(second[2], first[0]);
    assert_ne!(second[1], first[0]);
    assert_ne!(second[1], first[1]);
    assert_eq!(rollout.conversation_items.len(), 3);

    Ok(())
}

#[test]
fn reasoning_body_preserves_text_summary_and_encoded_content() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "think visibly")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [{
                "type": "reasoning",
                "content": [{"type": "reasoning_text", "text": "raw reasoning"}],
                "summary": [{"type": "summary_text", "text": "brief summary"}],
                "encrypted_content": "encoded-reasoning"
            }]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;

    let rollout = replay_bundle(temp.path())?;
    let reasoning_item_id = &rollout.inference_calls["inference-1"].response_item_ids[0];

    assert_eq!(
        rollout.conversation_items[reasoning_item_id].body.parts,
        vec![
            ConversationPart::Text {
                text: "raw reasoning".to_string(),
            },
            ConversationPart::Summary {
                text: "brief summary".to_string(),
            },
            ConversationPart::Encoded {
                label: "encrypted_content".to_string(),
                value: "encoded-reasoning".to_string(),
            },
        ],
    );

    Ok(())
}

#[test]
fn encrypted_reasoning_reuses_response_item_in_later_request() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let user = message("user", "count files");
    let function_call = json!({
        "type": "function_call",
        "name": "shell",
        "arguments": "{\"cmd\":\"find . -maxdepth 1 -type f | wc -l\"}",
        "call_id": "call-1"
    });
    let encrypted_reasoning = json!({
        "type": "reasoning",
        "summary": [],
        "encrypted_content": "encoded-reasoning"
    });
    let readable_reasoning = json!({
        "type": "reasoning",
        "content": [{"type": "text", "text": "need count"}],
        "summary": [],
        "encrypted_content": "encoded-reasoning"
    });

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [user]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [
                readable_reasoning,
                function_call
            ]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;
    start_turn(&writer, "turn-2")?;

    let followup = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                user,
                encrypted_reasoning,
                function_call,
                {
                    "type": "function_call_output",
                    "call_id": "call-1",
                    "output": "31\n"
                }
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", followup)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];
    let output_item_id = rollout.threads["thread-root"].conversation_item_ids[3].clone();

    assert_eq!(
        second.request_item_ids,
        vec![
            first.request_item_ids[0].clone(),
            first.response_item_ids[0].clone(),
            first.response_item_ids[1].clone(),
            output_item_id,
        ],
    );
    assert_eq!(
        rollout.conversation_items[&first.response_item_ids[0]]
            .body
            .parts,
        vec![
            ConversationPart::Text {
                text: "need count".to_string(),
            },
            ConversationPart::Encoded {
                label: "encrypted_content".to_string(),
                value: "encoded-reasoning".to_string(),
            },
        ],
    );
    assert_eq!(rollout.conversation_items.len(), 4);
    assert_eq!(
        rollout.threads["thread-root"].conversation_item_ids,
        second.request_item_ids,
    );

    Ok(())
}

#[test]
fn encrypted_reasoning_upgrades_when_later_sighting_has_more_readable_body() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let user = message("user", "count files");
    // Both sightings carry the same encrypted identity, but each has a
    // different kind of readable evidence. The reducer should keep both
    // observations because text and summary are complementary.
    let text_only_reasoning = json!({
        "type": "reasoning",
        "content": [{"type": "text", "text": "need count"}],
        "summary": [],
        "encrypted_content": "encoded-reasoning"
    });
    let summary_only_reasoning = json!({
        "type": "reasoning",
        "summary": [{"type": "summary_text", "text": "counting files"}],
        "encrypted_content": "encoded-reasoning"
    });

    let first_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [user, text_only_reasoning]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", first_request)?;
    start_turn(&writer, "turn-2")?;

    let second_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [user, summary_only_reasoning]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", second_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];
    let reasoning_item_id = &first.request_item_ids[1];

    // The reducer should keep one conversation item and merge the missing
    // readable kind without treating the second sighting as a conflict.
    assert_eq!(&second.request_item_ids[1], reasoning_item_id);
    assert_eq!(
        rollout.conversation_items[reasoning_item_id].body.parts,
        vec![
            ConversationPart::Text {
                text: "need count".to_string(),
            },
            ConversationPart::Summary {
                text: "counting files".to_string(),
            },
            ConversationPart::Encoded {
                label: "encrypted_content".to_string(),
                value: "encoded-reasoning".to_string(),
            },
        ],
    );
    assert_eq!(rollout.conversation_items.len(), 2);

    Ok(())
}

#[test]
fn same_encrypted_reasoning_with_different_text_reuses_first_readable_body() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let user = message("user", "count files");
    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [user]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    // The response is the first readable observation for this encrypted
    // reasoning blob, so it is the body later conflicting sightings must not
    // overwrite.
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({
            "response_id": "resp-1",
            "output_items": [{
                "type": "reasoning",
                "content": [{"type": "text", "text": "first text"}],
                "summary": [],
                "encrypted_content": "encoded-reasoning"
            }]
        }),
    )?;
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;
    start_turn(&writer, "turn-2")?;

    let conflicting_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                user,
                {
                    "type": "reasoning",
                    "content": [{"type": "text", "text": "different text"}],
                    "summary": [],
                    "encrypted_content": "encoded-reasoning"
                }
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", conflicting_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];
    let reasoning_item_id = &first.response_item_ids[0];

    // Same encrypted identity still reuses the response item, but conflicting
    // readable text is not a safe upgrade.
    assert_eq!(
        second.request_item_ids,
        vec![first.request_item_ids[0].clone(), reasoning_item_id.clone(),],
    );
    assert_eq!(
        rollout.conversation_items[reasoning_item_id].body.parts,
        vec![
            ConversationPart::Text {
                text: "first text".to_string(),
            },
            ConversationPart::Encoded {
                label: "encrypted_content".to_string(),
                value: "encoded-reasoning".to_string(),
            },
        ],
    );
    assert_eq!(rollout.conversation_items.len(), 2);

    Ok(())
}

#[test]
fn model_visible_call_id_reuse_with_different_content_is_reducer_error() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [{
                "type": "function_call",
                "name": "shell",
                "arguments": "{\"cmd\":\"cargo test\"}",
                "call_id": "call-1"
            }]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;
    start_turn(&writer, "turn-2")?;

    let conflicting_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [{
                "type": "function_call",
                "name": "shell",
                "arguments": "{\"cmd\":\"cargo check\"}",
                "call_id": "call-1"
            }]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", conflicting_request)?;

    expect_replay_error(
        &temp,
        "model-visible call id call-1 was reused with different content",
    )
}

#[test]
fn unsupported_model_item_is_reducer_error() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [
                {
                    "type": "new_unhandled_model_item",
                    "payload": "must not be silently skipped"
                }
            ]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    expect_replay_error(
        &temp,
        "unsupported model item type new_unhandled_model_item",
    )
}

#[test]
fn missing_request_input_is_reducer_error() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "model": "gpt-test"
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    expect_replay_error(&temp, "did not contain input")
}

#[test]
fn unknown_previous_response_id_is_reducer_error() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "previous_response_id": "resp-missing",
            "input": [message("user", "still here")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    expect_replay_error(&temp, "unknown previous_response_id resp-missing")
}

#[test]
fn compaction_boundary_repeats_prefix_and_reuses_replacement_items() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let developer = message("developer", "follow repo rules");
    let user = message("user", "count files");
    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [developer, user]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

    let summary = message("user", "summary from compacted history");
    let compaction_summary = json!({
        "type": "compaction",
        "encrypted_content": "encrypted-summary",
    });
    let checkpoint = writer.write_json_payload(
        RawPayloadKind::CompactionCheckpoint,
        &json!({
            "input_history": [developer, user],
            "replacement_history": [user, summary, compaction_summary]
        }),
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CompactionInstalled {
            compaction_id: "compaction-1".to_string(),
            checkpoint_payload: checkpoint,
        },
    )?;

    start_turn(&writer, "turn-2")?;
    let post_compaction_request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [developer, user, summary, compaction_summary]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", post_compaction_request)?;

    let rollout = replay_bundle(temp.path())?;
    let first = &rollout.inference_calls["inference-1"];
    let second = &rollout.inference_calls["inference-2"];
    let compaction = &rollout.compactions["compaction-1"];

    assert_eq!(compaction.input_item_ids, first.request_item_ids);
    assert_eq!(second.request_item_ids.len(), 4);
    assert_eq!(
        &second.request_item_ids[1..],
        compaction.replacement_item_ids.as_slice()
    );
    let marker = &rollout.conversation_items[&compaction.marker_item_id];
    assert_eq!(marker.kind, ConversationItemKind::CompactionMarker);
    assert_eq!(marker.body.parts, Vec::<ConversationPart>::new());
    assert_eq!(
        marker.produced_by,
        vec![ProducerRef::Compaction {
            compaction_id: "compaction-1".to_string()
        }],
    );
    assert_ne!(second.request_item_ids[0], first.request_item_ids[0]);
    assert_ne!(
        compaction.replacement_item_ids[0],
        first.request_item_ids[1]
    );
    assert_eq!(
        rollout.conversation_items[&compaction.replacement_item_ids[0]].produced_by,
        vec![ProducerRef::Compaction {
            compaction_id: "compaction-1".to_string()
        }],
    );
    assert_eq!(
        rollout.conversation_items[&compaction.replacement_item_ids[1]].produced_by,
        vec![ProducerRef::Compaction {
            compaction_id: "compaction-1".to_string()
        }],
    );
    assert_eq!(
        rollout.conversation_items[&compaction.replacement_item_ids[2]].channel,
        Some(ConversationChannel::Summary),
    );
    assert_eq!(
        rollout.conversation_items[&compaction.replacement_item_ids[2]].kind,
        ConversationItemKind::Message,
    );
    assert_eq!(
        rollout.conversation_items[&compaction.replacement_item_ids[2]]
            .body
            .parts,
        vec![ConversationPart::Encoded {
            label: "encrypted_content".to_string(),
            value: "encrypted-summary".to_string(),
        }],
    );

    Ok(())
}

#[test]
fn tool_call_links_model_call_and_followup_output_items() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "run tests")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-1", request)?;

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
    append_inference_completion(&writer, "inference-1", "resp-1", response)?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "tool-1".to_string(),
            model_visible_call_id: Some("call-1".to_string()),
            code_mode_runtime_tool_id: None,
            requester: crate::raw_event::RawToolCallRequester::Model,
            kind: ToolCallKind::ExecCommand,
            summary: ToolCallSummary::Generic {
                label: "exec_command".to_string(),
                input_preview: Some("cargo test".to_string()),
                output_preview: None,
            },
            invocation_payload: None,
        },
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "tool-1".to_string(),
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
                "type": "function_call_output",
                "call_id": "call-1",
                "output": "tests passed"
            }]
        }),
    )?;
    append_inference_start(&writer, "inference-2", "turn-2", followup)?;

    let rollout = replay_bundle(temp.path())?;
    let first_inference = &rollout.inference_calls["inference-1"];
    let second_inference = &rollout.inference_calls["inference-2"];
    let tool_call = &rollout.tool_calls["tool-1"];
    let output_item_id = second_inference
        .request_item_ids
        .last()
        .expect("follow-up output item");

    assert_eq!(
        first_inference.tool_call_ids_started_by_response,
        vec!["tool-1".to_string()],
    );
    assert_eq!(
        tool_call.model_visible_call_item_ids,
        first_inference.response_item_ids,
    );
    assert_eq!(
        tool_call.model_visible_output_item_ids,
        vec![output_item_id.clone()],
    );
    assert_eq!(
        rollout.conversation_items[output_item_id].produced_by,
        vec![ProducerRef::Tool {
            tool_call_id: "tool-1".to_string(),
        }],
    );

    Ok(())
}

#[test]
fn inference_start_rejects_unknown_codex_turn() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;

    let request = writer.write_json_payload(
        RawPayloadKind::InferenceRequest,
        &json!({
            "input": [message("user", "hello")]
        }),
    )?;
    append_inference_start(&writer, "inference-1", "turn-missing", request)?;

    expect_replay_error(&temp, "referenced unknown codex turn turn-missing")
}
