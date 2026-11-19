//! Accepted history retains original inference provenance across retries and resets.

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use super::test_support::append_completed_inference;
use super::test_support::append_inference_completion;
use super::test_support::append_inference_request;
use super::test_support::create_started_writer;
use super::test_support::message;
use super::test_support::start_thread;
use super::test_support::start_turn;
use super::test_support::start_turn_for_thread;
use super::test_support::trace_context;
use crate::ConversationPart;
use crate::ProducerRef;
use crate::RawPayloadKind;
use crate::RawTraceEventPayload;
use crate::replay_bundle;

#[test]
fn identical_text_from_a_discarded_attempt_does_not_gain_acceptance() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    let input = message("user", "answer");
    let mut output = message("assistant", "same text");
    output["id"] = json!("discarded");
    append_completed_inference(
        &writer,
        "thread-root",
        "turn-1",
        "first",
        vec![input.clone()],
        vec![output.clone()],
    )?;
    append_inference_request(&writer, "thread-root", "turn-1", "retry", vec![input])?;
    output["id"] = json!("accepted");
    // Acceptance can arrive before the asynchronous stream mapper's terminal record.
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::HistoryItemAccepted {
            item_id: "accepted".into(),
            agent_message_payload: None,
            tool_output_payload: None,
        },
    )?;
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({"output_items": [output]}),
    )?;
    append_inference_completion(&writer, "retry", "resp-retry", response)?;

    let trace = replay_bundle(temp.path())?;
    let first = &trace.inference_calls["first"];
    let retry = &trace.inference_calls["retry"];
    assert_ne!(first.response_item_ids, retry.response_item_ids);
    let mut expected = retry.request_item_ids.clone();
    expected.extend(retry.response_item_ids.clone());
    assert_eq!(trace.threads["thread-root"].conversation_item_ids, expected);
    assert_eq!(
        trace.conversation_items[&first.response_item_ids[0]]
            .responses_item_id
            .as_deref(),
        Some("discarded")
    );
    Ok(())
}

#[test]
fn late_response_reuses_a_later_request_sighting_without_rewriting_its_snapshot()
-> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    let input = message("user", "answer");
    let mut output = message("assistant", "accepted");
    output["id"] = json!("msg-accepted");
    append_inference_request(
        &writer,
        "thread-root",
        "turn-1",
        "first",
        vec![input.clone()],
    )?;
    let later_input = vec![input, output.clone(), message("user", "continue")];
    append_inference_request(
        &writer,
        "thread-root",
        "turn-1",
        "second",
        later_input.clone(),
    )?;
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({"output_items": [output]}),
    )?;
    append_inference_completion(&writer, "first", "resp-first", response)?;
    append_inference_request(&writer, "thread-root", "turn-1", "third", later_input)?;

    let trace = replay_bundle(temp.path())?;
    let second = &trace.inference_calls["second"];
    assert_eq!(
        trace.inference_calls["first"].response_item_ids,
        vec![second.request_item_ids[1].clone()]
    );
    assert_eq!(
        trace.inference_calls["third"].request_item_ids,
        second.request_item_ids
    );
    assert_eq!(
        trace.threads["thread-root"].conversation_item_ids,
        second.request_item_ids
    );
    assert_eq!(
        trace.conversation_items[&second.request_item_ids[1]].produced_by,
        vec![ProducerRef::Inference {
            inference_call_id: "first".into()
        }]
    );
    Ok(())
}

#[test]
fn late_response_keeps_compaction_replacement_distinct() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    let input = message("user", "answer");
    let mut output = message("assistant", "accepted");
    output["id"] = json!("msg-accepted");
    append_inference_request(
        &writer,
        "thread-root",
        "turn-1",
        "first",
        vec![input.clone()],
    )?;
    let settings = json!({"type": "configuration_update", "reasoning": {"effort": "high"}});
    let checkpoint = writer.write_json_payload(
        RawPayloadKind::CompactionCheckpoint,
        &json!({
            "input_history": [settings, input, output], "replacement_history": [settings, output]
        }),
    )?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::CompactionInstalled {
            compaction_id: "reset".into(),
            checkpoint_payload: checkpoint,
        },
    )?;
    let response = writer.write_json_payload(
        RawPayloadKind::InferenceResponse,
        &json!({"output_items": [output]}),
    )?;
    append_inference_completion(&writer, "first", "resp-first", response)?;
    append_inference_request(&writer, "thread-root", "turn-1", "second", vec![output])?;

    let trace = replay_bundle(temp.path())?;
    let checkpoint = &trace.compactions["reset"];
    assert_eq!(
        trace.inference_calls["first"].response_item_ids,
        vec![checkpoint.input_item_ids[1].clone()]
    );
    assert_eq!(
        trace.inference_calls["second"].request_item_ids,
        checkpoint.replacement_item_ids
    );
    assert_eq!(checkpoint.input_item_ids.len(), 2);
    assert_eq!(checkpoint.replacement_item_ids.len(), 1);
    assert_ne!(
        checkpoint.input_item_ids[1],
        checkpoint.replacement_item_ids[0]
    );
    assert_eq!(
        trace.conversation_items[&checkpoint.replacement_item_ids[0]].produced_by,
        vec![ProducerRef::Compaction {
            compaction_id: "reset".into()
        }]
    );
    let mut expected = checkpoint.input_item_ids.clone();
    expected.push(checkpoint.marker_item_id.clone());
    expected.extend(checkpoint.replacement_item_ids.clone());
    assert_eq!(trace.threads["thread-root"].conversation_item_ids, expected);
    Ok(())
}

#[test]
fn inherited_output_is_child_prefill_without_copying_parent_sampling_provenance()
-> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-parent")?;
    let mut output = message("assistant", "inherited");
    output["id"] = json!("msg-parent");
    append_completed_inference(
        &writer,
        "thread-root",
        "turn-parent",
        "parent",
        vec![message("user", "answer")],
        vec![output.clone()],
    )?;
    start_thread(&writer, "child", "/root/child")?;
    start_turn_for_thread(&writer, "child", "turn-child")?;
    append_inference_request(&writer, "child", "turn-child", "child", vec![output])?;

    let trace = replay_bundle(temp.path())?;
    let parent = &trace.inference_calls["parent"];
    let child = &trace.inference_calls["child"];
    assert_ne!(child.request_item_ids, parent.response_item_ids);
    assert_eq!(
        trace.threads["child"].conversation_item_ids,
        child.request_item_ids
    );
    assert_eq!(
        trace.conversation_items[&child.request_item_ids[0]].produced_by,
        Vec::<ProducerRef>::new()
    );
    // A sighting in the child must not imply acceptance in the parent's history.
    assert_eq!(
        trace.threads["thread-root"].conversation_item_ids,
        parent.request_item_ids
    );
    let accepted = RawTraceEventPayload::HistoryItemAccepted {
        item_id: "msg-parent".into(),
        agent_message_payload: None,
        tool_output_payload: None,
    };
    let context = trace_context("turn-parent");
    let first = writer.append_with_context(context.clone(), accepted.clone())?;
    let mut duplicate = writer.append_with_context(context, accepted)?;
    drop(writer);
    // Give the later event an earlier clock reading: sequence determines which pair wins.
    let path = temp.path().join("trace.jsonl");
    let original = serde_json::to_string(&duplicate)?;
    duplicate.wall_time_unix_ms = first.wall_time_unix_ms - 1;
    let events = std::fs::read_to_string(&path)?;
    assert!(events.contains(&original));
    std::fs::write(
        path,
        events.replace(&original, &serde_json::to_string(&duplicate)?),
    )?;
    let replayed = replay_bundle(temp.path())?;
    let local = &replayed.conversation_items[&parent.response_item_ids[0]];
    let inherited = &replayed.conversation_items[&child.request_item_ids[0]];
    assert_eq!(
        (local.accepted_at_unix_ms, local.accepted_seq),
        (Some(first.wall_time_unix_ms), Some(first.seq))
    );
    assert_eq!(
        (inherited.accepted_at_unix_ms, inherited.accepted_seq),
        (None, None)
    );
    Ok(())
}

#[test]
fn replay_rejects_a_mismatched_manifest_schema() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    create_started_writer(&temp)?;
    let manifest_path = temp.path().join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(&manifest_path)?)?;
    manifest["schema_version"] = json!(1);
    std::fs::write(manifest_path, serde_json::to_vec(&manifest)?)?;
    assert!(
        replay_bundle(temp.path())
            .unwrap_err()
            .to_string()
            .contains("unsupported manifest schema")
    );
    Ok(())
}

fn accept_output(writer: &crate::TraceWriter, item: &serde_json::Value) -> anyhow::Result<()> {
    let payload = writer.write_json_payload(RawPayloadKind::ProtocolEvent, item)?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::HistoryItemAccepted {
            item_id: item["id"].as_str().unwrap().into(),
            agent_message_payload: None,
            tool_output_payload: Some(payload),
        },
    )?;
    Ok(())
}

#[test]
fn accepted_tool_results_survive_without_inference_and_reuse_their_ids() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    let mut inputs = Vec::new();
    for kind in ["function_call_output", "custom_tool_call_output"] {
        let mut item = json!({"id": kind, "type": kind, "call_id": kind, "output": "full result"});
        accept_output(&writer, &item)?;
        item["output"] = json!("trimmed result");
        inputs.push(item);
    }
    let accepted = replay_bundle(temp.path())?;
    let ids = &accepted.threads["thread-root"].conversation_item_ids;
    assert_eq!(ids.len(), inputs.len());
    assert!(accepted.inference_calls.is_empty());
    for id in ids {
        assert!(accepted.conversation_items[id].accepted_seq.is_some());
    }

    append_inference_request(&writer, "thread-root", "turn-1", "next", inputs)?;
    let seen = replay_bundle(temp.path())?;
    assert_eq!(&seen.inference_calls["next"].request_item_ids, ids);
    assert_eq!(&seen.threads["thread-root"].conversation_item_ids, ids);
    for id in ids {
        assert_eq!(
            seen.conversation_items[id].accepted_seq,
            accepted.conversation_items[id].accepted_seq
        );
        for (state, text) in [(&accepted, "full result"), (&seen, "trimmed result")] {
            assert_eq!(
                state.conversation_items[id].body.parts,
                [ConversationPart::Text { text: text.into() }]
            );
        }
    }
    Ok(())
}

#[test]
fn capped_tool_output_keeps_the_next_stripped_id_in_its_original_slot() -> anyhow::Result<()> {
    let (temp, writer) =
        crate::writer::capture_tests::limited_trace(/*body*/ 512, /*bundle*/ 64 << 10)?;
    start_thread(&writer, "thread-root", "/root")?;
    start_turn(&writer, "turn-1")?;
    let request = json!({"input": [
        {"type": "function_call_output", "output": "x".repeat(1024)},
        {"type": "function_call_output", "output": "retained"}
    ]});
    writer.append(RawTraceEventPayload::InferenceStarted {
        inference_call_id: "sample".into(),
        thread_id: "thread-root".into(),
        codex_turn_id: "turn-1".into(),
        model: "model".into(),
        provider_name: "test".into(),
        request_payload: writer.write_json_payload(RawPayloadKind::InferenceRequest, &request)?,
        prepared_prompt_payload: writer
            .write_json_payload(RawPayloadKind::PreparedPrompt, &request)?,
        warmup_context_payload: None,
        stripped_tool_output_ids: vec![(0, "large".into()), (1, "small".into())],
    })?;
    let state = replay_bundle(temp.path())?;
    let ids = &state.inference_calls["sample"].request_item_ids;
    assert_eq!(ids.len(), 1);
    assert_eq!(
        state.conversation_items[&ids[0]]
            .responses_item_id
            .as_deref(),
        Some("small")
    );
    Ok(())
}

#[test]
fn injected_outputs_keep_their_request_positions_and_native_ids() -> anyhow::Result<()> {
    for accept_first in [true, false] {
        let temp = TempDir::new()?;
        let writer = create_started_writer(&temp)?;
        start_turn(&writer, "turn-1")?;
        let output = json!({"type": "function_call_output", "namespace": "codex_app", "name": "named", "output": "same"});
        let ids = ["fco_first", "legacy-1", "legacy-2"];
        // The first output was inherited. All four have the same wire body.
        let settings = json!({"type": "configuration_update", "reasoning": {"effort": "high"}});
        let mut inputs = vec![settings, output.clone()];
        let mut stripped_tool_output_ids = vec![(1, "legacy-inherited".into())];
        for (i, id) in ids.iter().enumerate() {
            let mut context = message("developer", &format!("context {i}"));
            context["id"] = json!(format!("msg_{i}"));
            inputs.push(context);
            let mut wire = output.clone();
            if i == 0 {
                wire["id"] = json!(id);
            } else {
                stripped_tool_output_ids.push((inputs.len(), (*id).to_owned()));
            }
            inputs.push(wire);
        }
        let accept = || {
            ids.iter().try_for_each(|id| {
                let mut accepted = output.clone();
                accepted["id"] = json!(id);
                accepted["output"] = json!("before trimming");
                accept_output(&writer, &accepted)
            })
        };
        if accept_first {
            accept()?;
        }
        let request = json!({"input": inputs});
        let payload = writer.write_json_payload(RawPayloadKind::InferenceRequest, &request)?;
        writer.append(RawTraceEventPayload::InferenceStarted {
            inference_call_id: "sample".into(),
            thread_id: "thread-root".into(),
            codex_turn_id: "turn-1".into(),
            model: "model".into(),
            provider_name: "test".into(),
            request_payload: payload.clone(),
            prepared_prompt_payload: writer
                .write_json_payload(RawPayloadKind::PreparedPrompt, &request)?,
            warmup_context_payload: None,
            stripped_tool_output_ids,
        })?;
        if !accept_first {
            accept()?;
        }
        let state = replay_bundle(temp.path())?;
        let thread = &state.threads["thread-root"].conversation_item_ids;
        assert_eq!(thread, &state.inference_calls["sample"].request_item_ids);
        let native_ids: Vec<_> = thread
            .iter()
            .map(|id| state.conversation_items[id].responses_item_id.as_deref())
            .collect();
        assert_eq!(
            native_ids,
            [
                "legacy-inherited",
                "msg_0",
                "fco_first",
                "msg_1",
                "legacy-1",
                "msg_2",
                "legacy-2"
            ]
            .map(Some)
        );
        assert!(state.conversation_items[&thread[0]].accepted_seq.is_none());
        for id in thread.iter().skip(2).step_by(2) {
            let item = &state.conversation_items[id];
            assert!(item.accepted_seq.is_some());
            assert_eq!(
                item.body.parts,
                [ConversationPart::Text {
                    text: "same".into()
                }]
            );
        }
        assert_eq!(
            crate::PayloadReader::new(temp.path()).read_json(&payload)?,
            request
        );
    }
    Ok(())
}
