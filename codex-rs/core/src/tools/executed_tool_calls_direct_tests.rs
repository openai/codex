//! Tests Direct metadata admission and permit lifetimes.
//! Metadata limits must leave ordinary tool outputs unchanged.

use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::ResponseItemId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_thread_context_ids_test_support::thread_context_ids;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;
use crate::session::turn_context::TurnContext;
use codex_login::AuthManager;

fn output(call_id: &str) -> ResponseItem {
    ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload::from_text("tool result".to_string()),
    })
}

#[test]
fn capture_only_does_not_publish_general_direct_metadata_after_refresh() {
    let mut features = Features::default();
    features.enable(Feature::ConnectorToolResultPassthrough);
    let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
    for enabled in [false, true, false] {
        let permit = recorder.reserve_direct_call().unwrap();
        features.set_enabled(Feature::ExecutedToolCallMetadata, enabled);
        recorder.refresh(&features);
        let mut item = output("shell");
        let original = item.clone();
        let call = ExecutedToolCall::new("exec_command".into(), json!({"cmd": "pwd"}));
        recorder.attach_direct_call_to_output(
            &mut item,
            Some((call.clone(), permit)),
            &thread_context_ids(),
        );
        if enabled {
            assert_eq!(
                item.executed_tool_call_metadata()
                    .and_then(|metadata| metadata.executed_tool_calls.as_ref()),
                Some(&vec![call]),
            );
        } else {
            assert_eq!(item, original);
        }
        assert_eq!(recorder.pending_direct_calls.load(Ordering::Relaxed), 0);
    }
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
            &thread_context_ids(),
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

pub(super) async fn connector_result_turn() -> TurnContext {
    let (_, mut turn) = crate::session::tests::make_session_and_context().await;
    let manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    turn.provider = codex_model_provider::create_model_provider(
        codex_model_provider_info::ModelProviderInfo::create_openai_provider(
            /*base_url*/ None,
        ),
        Some(Arc::clone(&manager)),
    );
    turn.auth_manager = Some(manager);
    Arc::make_mut(&mut turn.config)
        .features
        .enable(Feature::ConnectorToolResultPassthrough)
        .unwrap();
    turn
}

#[test_case::test_case(512; "shed on attach")]
#[test_case::test_case(MAX_RETAINED_DIRECT_METADATA_BYTES; "pruned from sampling")]
#[tokio::test]
async fn direct_result_body_budget_reopens_when_the_output_is_lost(available: usize) {
    let turn = connector_result_turn().await;
    let recorder = ExecutedToolCalls::new(&turn.config.features, &InitialHistory::New);
    let capture = recorder.begin_turn(&turn).unwrap();
    recorder.retained_direct_metadata_bytes.store(
        MAX_RETAINED_DIRECT_METADATA_BYTES - available,
        Ordering::Relaxed,
    );
    let bodies = |items: &[ResponseItem]| {
        capture.for_input(items, &turn.auth_manager.as_ref().unwrap().auth_owner())
    };
    // Each body fits by itself. Only the first call's larger arguments can force row shedding.
    let result = serde_json::from_value(json!({
        "content": [{"type": "text", "text": "x".repeat(600 * 1024)}]
    }))
    .unwrap();
    let attach = |id: &str, args| {
        let mut call = ExecutedToolCall::new("read".into(), args);
        let body_id = capture
            .start_capture(/*nested_call_id*/ None)
            .and_then(|call| call.record(&result, |_| true))
            .and_then(|pending| pending.accept())
            .expect("room for the Direct body");
        call.set_tool_result_metadata(ToolResultMetadata::default().with_result_id(Some(body_id)));
        let mut item = output(id);
        recorder.attach_direct_call_to_output(
            &mut item,
            Some((call, recorder.reserve_direct_call().unwrap())),
            &thread_context_ids(),
        );
        item
    };
    let mut first = [attach("first", json!({"argument": "x".repeat(1024)}))];
    if available == MAX_RETAINED_DIRECT_METADATA_BYTES {
        recorder.attach_to_compaction_prompt(&mut []);
        recorder.attach_to_prompt(&mut first, &mut HashMap::new());
        assert!(matches!(bodies(&first).as_slice(), [(0, 0, _)]));
        recorder.attach_to_prompt(&mut [], &mut HashMap::new());
    } else {
        assert!(first[0].executed_tool_call_metadata().is_none());
    }

    let [first] = first;
    let mut retry = [first, attach("second", json!({}))];
    recorder.attach_to_prompt(&mut retry, &mut HashMap::new());
    assert!(matches!(bodies(&retry).as_slice(), [(1, 0, _)]));
}

#[test_case::test_case(1, 600 * 1024; "byte limit")]
#[test_case::test_case(MAX_PENDING_EXECUTED_TOOL_CALLS, 1; "count limit")]
#[tokio::test]
async fn direct_result_bodies_require_the_originating_output_turn_and_owner(
    capacity: usize,
    text_bytes: usize,
) -> anyhow::Result<()> {
    struct SelectedAuth(CodexAuth);

    impl ExternalAuth for SelectedAuth {
        fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
            Box::pin(async { Ok(self.0.clone()) })
        }

        fn refresh(
            &self,
            _context: ExternalAuthRefreshContext,
        ) -> ExternalAuthFuture<'_, CodexAuth> {
            self.resolve()
        }
    }

    let hosted = |token| {
        let mut headers = http::HeaderMap::new();
        headers.insert("authorization", http::HeaderValue::from_static(token));
        headers.insert("chatgpt-account-id", "workspace-a".parse().unwrap());
        CodexAuth::Headers(AuthHeaders::new(headers))
    };
    let mut turn = connector_result_turn().await;
    // Overflow keeps ordinary metadata under its separate rollout.
    Arc::make_mut(&mut turn.config)
        .features
        .enable(Feature::ExecutedToolCallMetadata)
        .unwrap();
    let recorder = ExecutedToolCalls::new(&turn.config.features, &InitialHistory::New);
    let manager = Arc::clone(turn.auth_manager.as_ref().unwrap());
    let provider = turn.provider.info().clone();
    for config in [
        json!({"auth": {"command": "get-token"}}),
        json!({"requires_openai_auth": true, "base_url": "https://example.com/backend-api/codex"}),
        json!({"requires_openai_auth": true, "http_headers": {"aUtHoRiZaTiOn": "independent"}}),
        json!({"requires_openai_auth": true, "env_http_headers": {"Authorization": "TEST_MODEL_TOKEN"}}),
        json!({"requires_openai_auth": true, "query_params": {"api-key": "alternate"}}),
    ] {
        turn.provider = codex_model_provider::create_model_provider(
            serde_json::from_value(config)?,
            Some(Arc::clone(&manager)),
        );
        assert!(recorder.begin_turn(&turn).is_none());
    }
    turn.provider =
        codex_model_provider::create_model_provider(provider, Some(Arc::clone(&manager)));
    let capture = recorder.begin_turn(&turn).unwrap();
    let generation = manager
        .auth_change_state_receiver()
        .borrow()
        .owner_generation;
    let owner = manager.auth_owner();
    let bodies = |items: &[ResponseItem]| capture.for_input(items, &owner);

    let body = json!({"content": [{"type": "text", "text": "x".repeat(text_bytes)}]});
    let result = serde_json::from_value(body.clone())?;
    let metadata = json!({"provider": "observed"});
    let attach = |id: &str| {
        let mut call = ExecutedToolCall::new("read".into(), json!({"id": id}));
        call.set_tool_result_metadata(
            ToolResultMetadata::new(&metadata).with_result_id(
                capture
                    .start_capture(/*nested_call_id*/ None)
                    .and_then(|call| call.record(&result, |_| true))
                    .and_then(|pending| pending.accept()),
            ),
        );
        let mut item = output(id);
        recorder.attach_direct_call_to_output(
            &mut item,
            Some((call, recorder.reserve_direct_call().unwrap())),
            &thread_context_ids(),
        );
        item
    };
    let mut history: Vec<_> = (0..=capacity).map(|i| attach(&i.to_string())).collect();
    assert_eq!(
        recorder.lock_state().as_ref().unwrap().direct_calls.len(),
        history.len().min(MAX_PENDING_EXECUTED_TOOL_CALLS)
    );
    recorder.attach_to_prompt(&mut history, &mut HashMap::new());
    let selected = bodies(&history[..capacity]);
    assert_eq!(selected.len(), capacity);
    assert_eq!(
        serde_json::from_str::<JsonValue>(selected[0].2.as_json())?,
        body
    );
    assert!(bodies(&history[capacity..]).is_empty());
    let overflow = serde_json::to_value(&history[capacity])?;
    assert_eq!(overflow["output"], "tool result");
    assert_eq!(
        overflow["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        json!([{
            "name": "read",
            "arguments": {"id": capacity.to_string()},
            "tool_result_metadata": metadata,
        }])
    );

    recorder.attach_to_prompt(&mut history[1..], &mut HashMap::new());
    let history = [
        history[0].clone(),
        attach("replacement"),
        attach("over-cap"),
    ];
    assert!(matches!(bodies(&history).as_slice(), [(1, 0, _)]));
    let mut foreign = [output("replacement")];
    foreign[0].set_id(history[1].id().cloned());
    recorder.attach_to_prompt(&mut foreign, &mut HashMap::new());
    assert!(bodies(&foreign).is_empty());
    assert!(matches!(bodies(&history).as_slice(), [(1, 0, _)]));

    let other_manager = AuthManager::from_auth_for_testing(manager.auth_cached().unwrap());
    assert_eq!(
        other_manager
            .auth_change_state_receiver()
            .borrow()
            .owner_generation,
        generation
    );
    assert!(
        capture
            .for_input(&history, &other_manager.auth_owner())
            .is_empty()
    );
    // Switching away and back invalidates the request's original owner generation.
    for auth in [hosted("Bearer rotated"), manager.auth_cached().unwrap()] {
        manager
            .set_external_auth(Arc::new(SelectedAuth(auth)))
            .await?;
    }
    assert!(bodies(&history).is_empty());
    let finished = Arc::downgrade(&capture);
    drop(capture);
    assert!(finished.upgrade().is_none());

    // The next turn starts with an empty result store.
    turn.sub_id = "next".into();
    let next = recorder.begin_turn(&turn).unwrap();
    assert!(next.for_input(&history, &manager.auth_owner()).is_empty());
    Ok(())
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
            recorder.attach_direct_call_to_output(
                &mut outputs[index],
                prepared[index].take(),
                &thread_context_ids(),
            );
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
            &thread_context_ids(),
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
        call_id: "direct".into(),
        payload: ToolPayload::Function {
            arguments: json!({ "argument": "kept" }).to_string(),
        },
        encrypted_function_args: None,
        native_call: None,
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
    cloned.attach_direct_call_to_output(&mut overflow, prepare(&cloned), &thread_context_ids());
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
    recorder.attach_direct_call_to_output(&mut completed, pending.pop(), &thread_context_ids());
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
    recorder.attach_direct_call_to_output(&mut stale, Some(replacement), &thread_context_ids());
    assert!(stale.id().is_none());
    assert!(stale.executed_tool_call_metadata().is_none());
    let current = prepare(&recorder).expect("stale call released its slot");
    let mut current_output = output("current");
    recorder.attach_direct_call_to_output(
        &mut current_output,
        Some(current),
        &thread_context_ids(),
    );
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
async fn direct_recording_preserves_encoded_arguments_in_history() {
    let (_, turn) = crate::session::tests::make_session_and_context().await;
    let step = StepContext::for_test(Arc::new(turn));
    let mut features = Features::default();
    features.enable(Feature::ExecutedToolCallMetadata);
    let recorder = ExecutedToolCalls::new(&features, &InitialHistory::New);
    let invalid_json = "\"".repeat(5_000);
    let wire_bytes = serialized_json_bytes(&JsonValue::String(invalid_json.clone())).unwrap();
    assert!(wire_bytes > 8 * 1024);
    let call = ToolCall {
        tool_name: codex_tools::ToolName::plain("test_tool"),
        call_id: "direct".into(),
        payload: ToolPayload::Function {
            arguments: invalid_json.clone(),
        },
        encrypted_function_args: None,
        native_call: None,
    };
    let mut item = output("direct");
    let ordinary_output = serde_json::to_value(&item).unwrap()["output"].clone();
    recorder.attach_direct_call_to_output(
        &mut item,
        recorder.prepare_direct_call(&call, &ToolCallSource::Direct, &step),
        &thread_context_ids(),
    );
    let wire = serde_json::to_value(&item).unwrap();
    assert_eq!(wire["output"], ordinary_output);
    let metadata = &wire["internal_chat_message_metadata_passthrough"];
    let arguments = &metadata["executed_tool_calls"][0]["arguments"];
    assert_eq!(arguments, &JsonValue::String(invalid_json));
    assert_eq!(serialized_json_bytes(arguments).unwrap(), wire_bytes);
    assert_eq!(metadata["tool_calls_complete"], true);
}
