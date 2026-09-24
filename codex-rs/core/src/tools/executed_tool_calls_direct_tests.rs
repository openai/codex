//! Tests Direct metadata admission and permit lifetimes.
//! Metadata limits must leave ordinary tool outputs unchanged.

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
    assert!(metadata.get("tool_calls_complete").is_none());
}
