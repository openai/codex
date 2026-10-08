//! Tests Direct metadata admission and permit lifetimes.
//! Metadata limits must leave ordinary tool outputs unchanged.

use codex_protocol::ResponseItemId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn output(call_id: &str) -> ResponseItem {
    ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload::from_text("tool result".to_string()),
    })
}

#[test]
fn direct_request_uses_live_window_after_persisted_budget_is_exhausted() {
    let mut features = Features::default();
    features.enable(Feature::ExecutedToolCallMetadata);
    let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
    recorder
        .retained_direct_metadata_bytes
        .store(MAX_RETAINED_DIRECT_METADATA_BYTES, Ordering::Relaxed);
    let metadata = json!({"openai/resource_access": {"connector": "example"}});
    let make_output = |id: &str| {
        let mut call = ExecutedToolCall::new("connector_tool".to_string(), json!({}));
        call.set_tool_result_metadata(ToolResultMetadata::new(&metadata));
        let mut item = output(id);
        recorder.attach_direct_call_to_output(
            &mut item,
            Some((call, recorder.reserve_direct_call().unwrap())),
        );
        item
    };
    let old = make_output("old-window");
    let mut first = vec![old.clone()];
    recorder.attach_to_prompt(&mut first, &mut HashMap::new());
    let current = make_output("new-window");
    assert!(current.executed_tool_call_metadata().is_none());
    let serialized = serde_json::to_string(&codex_history::RolloutItem::ResponseItem(
        current.clone().into(),
    ))
    .unwrap();
    assert!(!serialized.contains("example"));

    // The input to the next window no longer contains the old output.
    let mut next = vec![current.clone()];
    recorder.attach_to_prompt(&mut next, &mut HashMap::new());
    for item in [&first[0], &next[0]] {
        let encoded = serde_json::to_value(item).unwrap();
        assert_eq!(encoded["output"], "tool result");
        assert_eq!(
            encoded["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0]["tool_result_metadata"],
            metadata,
        );
    }
    let mut pruned = vec![old];
    recorder.attach_to_compaction_prompt(&mut pruned);
    assert!(pruned[0].executed_tool_call_metadata().is_none());

    // Capture toggles must not revive a previous recorder's live-only metadata.
    recorder.refresh(&Features::default());
    recorder.refresh(&features);
    let mut after_refresh = vec![current];
    recorder.attach_to_prompt(&mut after_refresh, &mut HashMap::new());
    assert!(after_refresh[0].executed_tool_call_metadata().is_none());
}

#[test]
fn direct_metadata_follows_output_ids_with_reused_call_ids_and_reverse_completion() {
    for supplied_ids in [false, true] {
        let mut features = Features::default();
        features.enable(Feature::ExecutedToolCallMetadata);
        let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
        recorder
            .retained_direct_metadata_bytes
            .store(MAX_RETAINED_DIRECT_METADATA_BYTES, Ordering::Relaxed);
        let metadata = [json!({"result": "first"}), json!({"result": "second"})];
        let mut prepared = metadata
            .iter()
            .map(|metadata| {
                let mut call = ExecutedToolCall::new("connector_tool".to_string(), json!({}));
                call.set_tool_result_metadata(ToolResultMetadata::new(metadata));
                Some((call, recorder.reserve_direct_call().unwrap()))
            })
            .collect::<Vec<_>>();
        let mut outputs = [output("reused"), output("reused")];
        let expected_ids = [
            ResponseItemId::with_suffix("fco", "first"),
            ResponseItemId::with_suffix("fco", "second"),
        ];
        if supplied_ids {
            for (item, id) in outputs.iter_mut().zip(&expected_ids) {
                item.set_id(Some(id.clone()));
            }
        }
        // Completion order and prompt order must not choose which observation belongs here.
        for index in [1, 0] {
            recorder.attach_direct_call_to_output(&mut outputs[index], prepared[index].take());
            assert!(outputs[index].executed_tool_call_metadata().is_none());
            if supplied_ids {
                assert_eq!(outputs[index].id(), Some(&expected_ids[index]));
            }
        }
        let ids = outputs.clone().map(|item| item.id().unwrap().clone());
        assert_ne!(ids[0], ids[1]);
        let mut request = outputs;
        for reversed in [false, true] {
            if reversed {
                request.reverse();
            }
            recorder.attach_to_prompt(&mut request, &mut HashMap::new());
            for item in &request {
                let index = usize::from(item.id() == Some(&ids[1]));
                let encoded = serde_json::to_value(item).unwrap();
                assert_eq!(encoded["call_id"], "reused");
                assert_eq!(encoded["output"], "tool result");
                assert_eq!(
                    encoded["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0]
                        ["tool_result_metadata"],
                    metadata[index],
                );
            }
        }
    }
}

#[test]
fn direct_budget_keeps_an_omission_marker_when_it_fits() {
    for metadata in [
        json!({"provider": "x".repeat(1024)}),
        json!({"openai/resource_access": "x".repeat(1024)}),
    ] {
        let mut features = Features::default();
        features.enable(Feature::ExecutedToolCallMetadata);
        let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
        let mut call = ExecutedToolCall::new("test_tool".to_string(), json!({"argument": "kept"}));
        call.set_tool_result_metadata(ToolResultMetadata::new(&metadata));
        let mut expected = output("direct");
        expected.append_executed_tool_calls(vec![call.clone()]);
        expected.mark_tool_calls_complete();
        let available = 512;
        let overage = executed_tool_call_metadata_bytes(&expected) - available;
        let mut omitted =
            ExecutedToolCall::new("test_tool".to_string(), json!({"argument": "kept"}));
        omitted.set_tool_result_metadata(ToolResultMetadata::new(&json!(format!(
            "omitted_due_to_size_limit (overage_bytes={overage})"
        ))));
        expected.clear_executed_tool_calls();
        expected.append_executed_tool_calls(vec![omitted]);
        expected.mark_tool_calls_complete();
        let retained = MAX_RETAINED_DIRECT_METADATA_BYTES - available;
        recorder
            .retained_direct_metadata_bytes
            .store(retained, Ordering::Relaxed);

        let mut item = output("direct");
        recorder.attach_direct_call_to_output(
            &mut item,
            Some((call, recorder.reserve_direct_call().unwrap())),
        );

        expected.set_id(item.id().cloned());
        assert_eq!(item, expected);
        assert_eq!(
            recorder
                .retained_direct_metadata_bytes
                .load(Ordering::Relaxed),
            retained + executed_tool_call_metadata_bytes(&expected),
        );
    }
}

#[tokio::test]
async fn direct_pending_limit_releases_on_completion_or_dropped_future() {
    let (_, turn) = crate::session::tests::make_session_and_context().await;
    let step = StepContext::for_test(Arc::new(turn));
    let mut features = Features::default();
    features.enable(Feature::ExecutedToolCallMetadata);
    let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
    let call = ToolCall {
        tool_name: codex_tools::ToolName::plain("test_tool"),
        call_id: "direct".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "argument": "kept" }).to_string(),
        },
        encrypted_function_args: None,
    };
    let prepare = |recorder: &ExecutedToolCalls| {
        recorder.prepare_direct_call(&call, &ToolCallSource::Direct, &step)
    };
    let mut pending = (0..MAX_PENDING_EXECUTED_TOOL_CALLS)
        .map(|_| prepare(&recorder).expect("metadata slot"))
        .collect::<Vec<_>>();
    let cloned = recorder.clone();
    let mut overflow = output("overflow");
    let before = serde_json::to_value(&overflow).expect("serializable output");
    cloned.attach_direct_call_to_output(&mut overflow, prepare(&cloned));
    assert_eq!(
        serde_json::to_value(&overflow).expect("serializable output"),
        before,
    );

    let canceled = pending.pop().expect("pending call");
    let unpolled = async move {
        std::future::pending::<()>().await;
        drop(canceled);
    };
    assert!(prepare(&cloned).is_none());
    drop(unpolled);
    let replacement = prepare(&cloned).expect("dropped future released its slot");
    assert!(prepare(&recorder).is_none());

    let mut completed = output("completed");
    recorder.attach_direct_call_to_output(&mut completed, pending.pop());
    assert_eq!(
        completed
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.tool_calls_complete),
        Some(true),
    );
    let after_completion = prepare(&recorder).expect("completion released its slot");

    recorder.refresh(&Features::default());
    recorder.refresh(&features);
    assert!(prepare(&recorder).is_none());
    let mut stale = output("stale");
    recorder.attach_direct_call_to_output(&mut stale, Some(replacement));
    assert!(stale.executed_tool_call_metadata().is_none());
    let current = prepare(&recorder).expect("stale call released its slot");
    let mut current_output = output("current");
    recorder.attach_direct_call_to_output(&mut current_output, Some(current));
    assert_eq!(
        current_output
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.tool_calls_complete),
        Some(true),
    );
    drop(pending);
    drop(after_completion);
    assert_eq!(recorder.pending_direct_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn direct_budget_counts_the_encoded_argument_before_it_enters_history() {
    let (_, turn) = crate::session::tests::make_session_and_context().await;
    let step = StepContext::for_test(Arc::new(turn));
    let mut features = Features::default();
    features.enable(Feature::ExecutedToolCallMetadata);
    let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
    let invalid_json = "\"".repeat(5_000);
    let wire_bytes = serialized_json_bytes(&JsonValue::String(invalid_json.clone())).unwrap();
    assert!(invalid_json.len() < MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES);
    assert!(wire_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES);
    let call = ToolCall {
        tool_name: codex_tools::ToolName::plain("test_tool"),
        call_id: "direct".to_string(),
        payload: ToolPayload::Function {
            arguments: invalid_json,
        },
        encrypted_function_args: None,
    };
    let mut item = output("direct");
    let ordinary_output = serde_json::to_value(&item).unwrap()["output"].clone();
    recorder.attach_direct_call_to_output(
        &mut item,
        recorder.prepare_direct_call(&call, &ToolCallSource::Direct, &step),
    );
    let wire = serde_json::to_value(&item).unwrap();
    assert_eq!(wire["output"], ordinary_output);
    let metadata = &wire["internal_chat_message_metadata_passthrough"];
    let arguments = &metadata["executed_tool_calls"][0]["arguments"];
    assert!(serialized_json_bytes(arguments).unwrap() < MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES);
    assert_eq!(
        arguments["_codex_executed_tool_call_truncated"]["original_bytes"],
        wire_bytes,
    );
    assert_eq!(metadata["tool_calls_complete"], true);
}
