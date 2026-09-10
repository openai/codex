use codex_history::RolloutItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultMetadata;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadHistoryMode;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn new_recorder(history: InitialHistory) -> ExecutedToolCalls {
    let mut features = Features::default();
    features.enable(Feature::ExecutedToolCallMetadata);
    ExecutedToolCalls::new(&features, &history)
}

fn output(call_id: &str) -> ResponseItem {
    ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload::from_text(String::new()),
    })
}

fn exec_output(call_id: &str) -> ResponseItem {
    ResponseItem::from(ResponseInputItem::CustomToolCallOutput {
        call_id: call_id.to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text(String::new()),
    })
}

fn exec_input(call_id: &str) -> ResponseItem {
    serde_json::from_value(json!({
        "type": "custom_tool_call", "call_id": call_id, "name": "exec", "input": "",
    }))
    .expect("exec input must deserialize")
}

fn wait_input(call_id: &str, cell: &CellId) -> ResponseItem {
    serde_json::from_value(json!({
        "type": "function_call", "call_id": call_id, "name": "wait",
        "arguments": json!({"cell_id": cell.as_str()}).to_string(),
    }))
    .expect("wait input must deserialize")
}

fn record_nested_call(
    recorder: &ExecutedToolCalls,
    cell: &CellId,
    call_id: &str,
) -> ExecutedToolCall {
    let call = ExecutedToolCall::new("nested_tool".to_string(), json!({}));
    recorder.record_nested_tool_call(
        cell.clone(),
        call_id.to_string(),
        call.clone(),
        /*original_bytes*/ 2,
    );
    call
}

fn tool_calls_complete(item: &ResponseItem) -> Option<bool> {
    item.executed_tool_call_metadata()
        .and_then(|metadata| metadata.tool_calls_complete)
}

#[tokio::test]
async fn recorder_refreshes_without_changing_execution_features_or_claiming_missing_history() {
    struct MetadataLookup<'a>(std::cell::Cell<usize>, JsonValue, &'a ExecutedToolCalls);

    impl ToolOutput for MetadataLookup<'_> {
        fn log_output(&self) -> String {
            panic!("recording must not read the diagnostic output")
        }

        fn success_for_logging(&self) -> bool {
            panic!("recording must not inspect execution success")
        }

        fn to_response_item(&self, _call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
            panic!("recording must not rebuild the tool result")
        }

        fn tool_result_metadata(&self) -> Option<&JsonValue> {
            assert!(
                self.2.state.try_lock().is_ok(),
                "result callbacks must run unlocked"
            );
            self.0.set(self.0.get() + 1);
            Some(&self.1)
        }
    }

    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let step = StepContext::for_test(Arc::new(turn));
    // The broker holds a clone before rollout enablement reaches the session.
    let calls = session.services.executed_tool_calls.clone();
    assert!(calls.lock_state().is_none());
    let mut next_config = (*session.get_config().await).clone();
    let mut retry_cache = HashMap::new();
    for (index, enabled) in [false, true, false, true].into_iter().enumerate() {
        next_config
            .features
            .set_enabled(Feature::ExecutedToolCallMetadata, enabled)
            .expect("test feature must be configurable");
        session.refresh_runtime_config(next_config.clone()).await;
        assert_eq!(calls.lock_state().is_some(), enabled);
        // A legacy file reload has no new rollout snapshot and must keep this setting.
        session.reload_user_config_layer().await;
        assert_eq!(calls.lock_state().is_some(), enabled);
        // An already-running turn and future turns keep their execution feature policy.
        assert!(!ExecutedToolCalls::is_enabled(&step.turn.config.features));
        assert!(!ExecutedToolCalls::is_enabled(
            &session.get_config().await.features
        ));

        let call = ToolCall {
            tool_name: codex_tools::ToolName::plain("test_tool"),
            call_id: format!("call-{index}"),
            payload: ToolPayload::Function {
                arguments: r#"{"value":7}"#.to_string(),
            },
            encrypted_function_args: None,
        };
        calls.record_tool_call(&call, &ToolCallSource::Direct, &step);
        let result = MetadataLookup(std::cell::Cell::new(0), json!({}), &calls);
        calls.record_accepted_result(&ToolCallSource::Direct, &call.call_id, &result);
        assert_eq!(result.0.get(), usize::from(enabled));

        let input = serde_json::from_value(json!({
            "type": "function_call",
            "call_id": call.call_id,
            "name": "test_tool",
            "arguments": "{\"value\":7}",
        }))
        .expect("tool input must deserialize");
        let mut prompt = vec![input, output(&call.call_id)];
        if !enabled {
            prompt[1].mark_tool_calls_complete();
        }
        let original = prompt.clone();
        calls.attach_to_prompt(&mut prompt, &mut retry_cache);
        if !enabled {
            assert_eq!(prompt, original);
        } else {
            assert_eq!(tool_calls_complete(&prompt[1]), None);
        }
        let mut expected_call = ExecutedToolCall::new("test_tool".to_string(), json!({"value": 7}));
        expected_call.set_tool_result_metadata(ToolResultMetadata::new(&json!({})));
        let expected_calls = vec![expected_call];
        assert_eq!(
            prompt[1]
                .executed_tool_call_metadata()
                .and_then(|metadata| metadata.executed_tool_calls.as_ref()),
            enabled.then_some(&expected_calls),
        );

        if enabled {
            // Reapplying an enabled config must preserve already-recorded calls.
            session.refresh_runtime_config(next_config.clone()).await;
            let mut replay = original;
            calls.attach_to_prompt(&mut replay, &mut HashMap::new());
            assert_eq!(replay, prompt);
        }

        let cell = CellId::new(format!("cell-{index}"));
        let origin = format!("exec-{index}");
        calls.start_cell(&cell, &origin);
        let nested = record_nested_call(&calls, &cell, "nested");
        calls.finish_cell_recording(&cell);
        let mut prompt = vec![exec_input(&origin), exec_output(&origin)];
        calls.attach_to_prompt(&mut prompt, &mut retry_cache);
        assert_eq!(tool_calls_complete(&prompt[1]), None);
        assert_eq!(
            prompt[1]
                .executed_tool_call_metadata()
                .and_then(|metadata| metadata.executed_tool_calls.as_ref()),
            enabled.then_some(&vec![nested]),
        );
    }
}

#[test]
fn executed_tool_call_recorder_bounds_pending_calls_and_preserves_overflow() {
    let recorder = new_recorder(InitialHistory::Forked(Vec::new()));

    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.record_call(
            &ToolCall {
                tool_name: codex_tools::ToolName::plain("direct_tool"),
                call_id: format!("direct-{index}"),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
                encrypted_function_args: None,
            },
            &ToolCallSource::Direct,
            ToolMode::Direct,
        );
        recorder.record_call(
            &ToolCall {
                tool_name: codex_tools::ToolName::plain(crate::tools::code_mode::PUBLIC_TOOL_NAME),
                call_id: format!("failed-wrapper-{index}"),
                payload: ToolPayload::Custom {
                    input: "".to_string(),
                },
                encrypted_function_args: None,
            },
            &ToolCallSource::Direct,
            ToolMode::CodeMode,
        );
    }
    let metadata = json!({ "arbitrary-key": ["R1", null] });
    assert!(recorder.record_tool_result_metadata(&ToolCallSource::Direct, "direct-0", &metadata));

    let cell_id = CellId::new("bounded-cell".to_string());
    recorder.start_cell(&cell_id, "bounded-output");
    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.record_nested_tool_call(
            cell_id.clone(),
            format!("nested-{index}"),
            ExecutedToolCall::new("nested_tool".to_string(), json!({})),
            /*original_bytes*/ 2,
        );
    }

    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.register_cell(
            &CellId::new(format!("cell-{index}")),
            &format!("output-{index}"),
        );
    }

    {
        let state = recorder.lock_state();
        let state = state.as_ref().unwrap();
        assert_eq!(
            state.direct_calls.len(),
            MAX_PENDING_EXECUTED_TOOL_CALLS + 1
        );
        assert_eq!(
            serde_json::to_value(state.direct_calls.get("direct-0").unwrap()).unwrap()["tool_result_metadata"],
            metadata,
        );
        assert_eq!(
            serde_json::to_value(
                state
                    .direct_calls
                    .get(&format!("direct-{MAX_PENDING_EXECUTED_TOOL_CALLS}"))
                    .expect("first excess direct call must be marked"),
            )
            .expect("direct overflow marker must serialize"),
            json!({
                "name": "direct_tool",
                "arguments": {
                    "_codex_executed_tool_call_truncated": {
                        "original_bytes": 2,
                        "max_bytes": 0,
                    },
                },
            }),
        );
        assert_eq!(
            state.pending_nested_calls,
            MAX_PENDING_EXECUTED_TOOL_CALLS + 1
        );
        assert_eq!(
            state.pending_wrapper_origins.len(),
            MAX_PENDING_EXECUTED_TOOL_CALLS
        );
        assert_eq!(state.cells.len(), MAX_PENDING_EXECUTED_TOOL_CALLS);
        assert_eq!(state.output_cells.len(), MAX_PENDING_EXECUTED_TOOL_CALLS);
    }

    let mut items = [exec_input("bounded-output"), exec_output("bounded-output")];
    let mut retry_cache = HashMap::new();
    recorder.finish_cell_recording(&cell_id);
    recorder.attach_pending_to_prompt(&mut items, &mut retry_cache);

    assert_eq!(
        items[1]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.tool_calls_complete),
        None,
    );
    let calls = items[1]
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.executed_tool_calls.as_ref())
        .expect("bounded nested calls must attach to their own output");
    assert_eq!(calls.len(), MAX_PENDING_EXECUTED_TOOL_CALLS + 1);
    assert_eq!(
        serde_json::to_value(calls.last().expect("overflow marker must be retained"))
            .expect("nested overflow marker must serialize"),
        json!({
            "name": "nested_tool",
            "arguments": {
                "_codex_executed_tool_call_truncated": {
                    "original_bytes": 2,
                    "max_bytes": 0,
                },
            },
        }),
    );
    let expected_calls = calls.clone();

    {
        let state = recorder.lock_state();
        let state = state.as_ref().unwrap();
        assert!(state.pending_wrapper_origins.is_empty());
        assert_eq!(state.pending_nested_calls, 0);
        assert!(!state.cells.contains_key(&cell_id));
        assert_eq!(retry_cache.len(), 1);
    }

    let mut replayed_items = [exec_output("bounded-output")];
    let mut replay_retry_cache = HashMap::new();
    assert!(recorder.attach_pending_to_prompt(&mut replayed_items, &mut replay_retry_cache));
    assert_eq!(
        replayed_items[0]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.executed_tool_calls.as_ref()),
        Some(&expected_calls),
    );

    let mut compacted_retry_cache = HashMap::new();
    assert!(!recorder.attach_pending_to_prompt(&mut [], &mut compacted_retry_cache));
    let state = recorder.lock_state();
    let state = state.as_ref().unwrap();
    assert!(state.retained_calls.is_empty());
}

#[test]
fn executed_tool_call_recorder_bounds_retained_history_and_keeps_latest_calls() {
    let recorder = new_recorder(InitialHistory::Forked(Vec::new()));
    let mut history = Vec::new();
    let arguments = serde_json::to_string(&json!({ "payload": "x".repeat(1024) }))
        .expect("tool arguments must serialize");
    let mut prompt = Vec::new();

    for index in 0..512 {
        let call_id = format!("retained-{index}");
        recorder.record_call(
            &ToolCall {
                tool_name: codex_tools::ToolName::plain(format!("retained_tool_{index}")),
                call_id: call_id.clone(),
                payload: ToolPayload::Function {
                    arguments: arguments.clone(),
                },
                encrypted_function_args: None,
            },
            &ToolCallSource::Direct,
            ToolMode::Direct,
        );
        history.push(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(String::new()),
            internal_chat_message_metadata_passthrough: None,
        });
        prompt = history.clone();
        assert!(recorder.attach_pending_to_prompt(&mut prompt, &mut HashMap::new()));
        codex_protocol::models::bound_executed_tool_calls_for_prompt(&mut prompt);
        let latest_call = prompt
            .last()
            .and_then(ResponseItem::executed_tool_call_metadata)
            .and_then(|metadata| metadata.executed_tool_calls.as_ref())
            .and_then(|calls| calls.first())
            .map(serde_json::to_value)
            .transpose()
            .expect("latest tool call must serialize")
            .expect("latest tool call must remain in retained metadata");
        assert_eq!(latest_call["name"], format!("retained_tool_{index}"));
        assert_eq!(
            latest_call["arguments"],
            json!({ "payload": "x".repeat(1024) }),
        );
    }

    let state = recorder.lock_state();
    let state = state.as_ref().unwrap();
    let retained_bytes = state
        .retained_calls
        .values()
        .map(|retained| serialized_json_bytes(&retained.calls))
        .sum::<serde_json::Result<usize>>()
        .expect("retained calls must serialize");
    assert!(retained_bytes <= MAX_EXECUTED_TOOL_CALL_FULL_ARGUMENT_BYTES_PER_OUTPUT);

    let metadata = prompt
        .iter()
        .filter_map(ResponseItem::executed_tool_call_metadata)
        .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
        .flatten()
        .map(|call| serde_json::to_value(call).expect("retained call must serialize"))
        .collect::<Vec<_>>();
    assert!(metadata.len() < 512);
    // Dropped outputs do not add their omission counts to unrelated surviving calls.
    assert!(metadata.iter().all(|call| {
        call["arguments"]["_codex_executed_tool_call_truncated"]["omitted_calls"].is_null()
    }));
}

#[test]
fn tool_call_completeness_requires_finished_lossless_recording() {
    for scenario in ["empty", "unobserved", "late_call"] {
        let recorder = new_recorder(InitialHistory::Forked(Vec::new()));
        let cell_id = CellId::new(scenario.to_string());
        if scenario == "unobserved" {
            recorder.register_cell(&cell_id, "output");
        } else {
            recorder.start_cell(&cell_id, "output");
        }
        if scenario == "late_call" {
            recorder.finish_cell_recording(&cell_id);
        }
        if scenario != "empty" {
            recorder.record_nested_tool_call(
                cell_id.clone(),
                "nested-call".to_string(),
                ExecutedToolCall::new("nested_tool".to_string(), json!({})),
                /*original_bytes*/ 2,
            );
        }
        recorder.finish_cell_recording(&cell_id);

        let mut items = [exec_input("output"), exec_output("output")];
        recorder.attach_pending_to_prompt(&mut items, &mut HashMap::new());
        assert_eq!(
            items[1]
                .executed_tool_call_metadata()
                .and_then(|metadata| metadata.tool_calls_complete),
            None,
            "{scenario} must not claim complete recording",
        );
    }
}

#[test]
fn tool_call_completeness_survives_waits_without_changing_deltas() {
    let metadata = json!({ "provider": { "ids": ["CLATE"] } });
    for truncated in [false, true] {
        let recorder = new_recorder(InitialHistory::New);
        let cell_id = CellId::new("multi-wait".to_string());
        recorder.start_cell(&cell_id, "exec");
        let mut history = Vec::new();
        let mut expected = Vec::new();
        let mut retry_cache = HashMap::new();
        for (index, call_id) in ["exec", "wait-1", "wait-2", "wait-3"]
            .into_iter()
            .enumerate()
        {
            if index > 0 {
                recorder.record_call(
                    &ToolCall {
                        tool_name: codex_tools::ToolName::plain(
                            crate::tools::code_mode::WAIT_TOOL_NAME,
                        ),
                        call_id: call_id.to_string(),
                        payload: ToolPayload::Function {
                            arguments: json!({"cell_id": cell_id.as_str()}).to_string(),
                        },
                        encrypted_function_args: None,
                    },
                    &ToolCallSource::Direct,
                    ToolMode::CodeModeOnly,
                );
                recorder.register_cell(&cell_id, call_id);
            }
            let input = if index == 0 {
                exec_input(call_id)
            } else {
                wait_input(call_id, &cell_id)
            };
            let mut expected_output = if index == 0 {
                exec_output(call_id)
            } else {
                output(call_id)
            };
            history.push(input.clone());
            history.push(expected_output.clone());
            expected.push(input);
            if index < 2 {
                // Identical calls are distinct submissions; truncation stays sticky across waits.
                let original_bytes = if truncated && index == 1 { 9_000 } else { 2 };
                let call = if original_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES {
                    ExecutedToolCall::truncated(
                        "nested_tool".to_string(),
                        original_bytes,
                        MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES,
                    )
                } else {
                    ExecutedToolCall::new("nested_tool".to_string(), json!({}))
                };
                recorder.record_nested_tool_call(
                    cell_id.clone(),
                    format!("nested-{index}"),
                    call.clone(),
                    original_bytes,
                );
                let mut call = call;
                if index == 1 {
                    let source = |cell_id: &str| ToolCallSource::CodeMode {
                        cell_id: cell_id.to_string(),
                        runtime_tool_call_id: "runtime-call".to_string(),
                    };
                    assert!(!recorder.record_tool_result_metadata(
                        &source("other-cell"),
                        "nested-0",
                        &json!({}),
                    ));
                    let source = source(cell_id.as_str());
                    assert!(recorder.record_tool_result_metadata(&source, "nested-0", &metadata));
                    let mut delayed_call =
                        ExecutedToolCall::new("nested_tool".to_string(), json!({}));
                    delayed_call.set_tool_result_metadata(ToolResultMetadata::new(&metadata));
                    let mut delayed_output = exec_output("exec");
                    delayed_output.append_executed_tool_calls(vec![delayed_call]);
                    delayed_output.set_tool_call_cell_id("exec");
                    expected[1] = delayed_output;
                    assert!(recorder.record_tool_result_metadata(&source, "nested-1", &json!({})));
                    call.set_tool_result_metadata(ToolResultMetadata::new(&json!({})));
                }
                expected_output.append_executed_tool_calls(vec![call]);
            } else if index == 3 {
                recorder.finish_cell_recording(&cell_id);
            }
            if index < 2 || index == 3 && !truncated {
                expected_output.set_tool_call_cell_id("exec");
            }
            if index == 3 && !truncated {
                expected_output.mark_tool_calls_complete();
            }
            expected.push(expected_output);
            for _ in 0..2 {
                let mut prompt = history.clone();
                assert!(recorder.attach_pending_to_prompt(&mut prompt, &mut retry_cache));
                assert_eq!(prompt, expected);
            }
        }
        let independent = CellId::new("independent-runtime".to_string());
        recorder.start_cell(&independent, "independent-exec");
        record_nested_call(&recorder, &independent, "independent-call");
        recorder.finish_cell_recording(&independent);
        history.extend([
            exec_input("independent-exec"),
            exec_output("independent-exec"),
        ]);
        // Damaged deltas revoke their terminal sibling, not an independent cell.
        history[2] = wait_input("wait-1", &independent);
        history[0] = wait_input("exec", &independent);
        recorder.attach_pending_to_prompt(&mut history, &mut retry_cache);
        assert_eq!(tool_calls_complete(&history[7]), None);
        assert_eq!(tool_calls_complete(&history[9]), Some(true));

        let state = recorder.lock_state();
        let state = state.as_ref().unwrap();
        assert!(state.cells.is_empty());
        assert_eq!(state.pending_nested_calls, 0);
    }
}

#[test]
fn unrelated_raw_overflow_retains_a_complete_exec_within_the_request_budget() {
    let request_budget = 32 * 1024;
    let recorder = new_recorder(InitialHistory::New);
    let cell = CellId::new("clean-overflow-cell".to_string());
    recorder.start_cell(&cell, "exec-clean");
    let clean_call = record_nested_call(&recorder, &cell, "nested-clean");
    recorder.finish_cell_recording(&cell);

    let mut unrelated = output("direct-oversized");
    let arguments = json!("x".repeat(7 * 1024));
    assert!(serialized_json_bytes(&arguments).unwrap() <= MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES);
    unrelated.append_executed_tool_calls(
        (0..6)
            .map(|index| ExecutedToolCall::new(format!("direct-{index}"), arguments.clone()))
            .collect(),
    );
    let original = [
        unrelated,
        exec_input("exec-clean"),
        exec_output("exec-clean"),
    ];
    let mut prompt = original.clone();
    assert!(
        prompt
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            > request_budget
    );
    recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
    let metadata = prompt[2]
        .executed_tool_call_metadata()
        .expect("the later clean output must retain its calls");
    assert_eq!(metadata.executed_tool_calls, Some(vec![clean_call]));
    assert_eq!(metadata.tool_calls_complete, Some(true));
    assert!(
        prompt[0]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.executed_tool_calls.as_ref())
            .is_some_and(|calls| calls.iter().any(|call| matches!(
                call.arguments(),
                ExecutedToolCallArguments::Truncated { .. }
            )))
    );
    assert!(
        prompt
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            <= request_budget
    );

    let mut retry = original;
    recorder.attach_to_prompt(&mut retry, &mut HashMap::new());
    assert_eq!(retry, prompt);
}

#[test]
fn paginated_history_allows_fresh_origins_but_rejects_known_ids() {
    for origin in ["fresh-exec", "old-exec"] {
        let history = InitialHistory::Forked(vec![
            RolloutItem::SessionMeta(SessionMetaLine {
                meta: SessionMeta {
                    history_mode: ThreadHistoryMode::Paginated,
                    ..SessionMeta::default()
                },
                git: None,
            }),
            RolloutItem::ResponseItem(exec_input("old-exec").into()),
            RolloutItem::ResponseItem(exec_output("old-exec").into()),
        ]);
        let recorder = new_recorder(history);
        let cell = CellId::new("1".to_string());
        recorder.start_cell(&cell, origin);
        let call = record_nested_call(&recorder, &cell, "nested");
        recorder.finish_cell_recording(&cell);

        // The old call is only in inherited history, so duplicate prompt IDs cannot reject it.
        let mut prompt = [exec_input(origin), exec_output(origin)];
        recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
        assert_eq!(
            tool_calls_complete(&prompt[1]),
            (origin == "fresh-exec").then_some(true),
        );
        assert_eq!(
            prompt[1]
                .executed_tool_call_metadata()
                .unwrap()
                .executed_tool_calls,
            Some(vec![call]),
        );

        // Compaction may drop the input after the output has already been authenticated.
        let mut retry = [exec_output(origin)];
        recorder.attach_to_prompt(&mut retry, &mut HashMap::new());
        assert_eq!(retry[0], prompt[1]);
    }
}

#[test]
fn forked_wait_keeps_calls_without_completion() {
    let recorder = new_recorder(InitialHistory::Forked(vec![RolloutItem::ResponseItem(
        exec_input("old-exec").into(),
    )]));
    let cell = CellId::new("1".to_string());
    recorder.start_cell(&cell, "new-exec");
    let call = record_nested_call(&recorder, &cell, "nested");
    recorder.register_cell(&cell, "wait");
    recorder.finish_cell_recording(&cell);

    let mut prompt = [wait_input("wait", &cell), output("wait")];
    recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
    let mut expected = output("wait");
    expected.append_executed_tool_calls(vec![call]);
    expected.set_tool_call_cell_id("new-exec");
    assert_eq!(prompt[1], expected);

    let mut compacted = [output("wait")];
    recorder.attach_to_prompt(&mut compacted, &mut HashMap::new());
    assert_eq!(compacted[0], expected);
}

#[test]
fn wrapper_mismatch_cannot_become_complete_on_a_later_wait() {
    for scenario in [
        "missing_exec",
        "wrong_output",
        "wrong_wait_cell",
        "changed_exec",
    ] {
        let recorder = new_recorder(InitialHistory::New);
        let cell = CellId::new("runtime".to_string());
        recorder.start_cell(&cell, "exec");
        let call = record_nested_call(&recorder, &cell, "nested");
        let mut prompt = match scenario {
            "missing_exec" => vec![exec_output("exec")],
            "changed_exec" => vec![exec_input("exec"), exec_output("exec")],
            "wrong_wait_cell" => {
                recorder.register_cell(&cell, "first-wait");
                vec![
                    wait_input("first-wait", &CellId::new("other-runtime".to_string())),
                    output("first-wait"),
                ]
            }
            // An exec custom call cannot authenticate a function output with the same ID.
            _ => vec![exec_input("exec"), output("exec")],
        };
        recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
        assert_eq!(
            prompt
                .last()
                .unwrap()
                .executed_tool_call_metadata()
                .unwrap()
                .executed_tool_calls,
            Some(vec![call]),
        );
        if scenario == "changed_exec" {
            // Invalidate already-retained evidence while its cell is still active.
            prompt[0] = wait_input("exec", &cell);
            recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
        }

        recorder.register_cell(&cell, "last-wait");
        recorder.finish_cell_recording(&cell);
        let mut last = [wait_input("last-wait", &cell), output("last-wait")];
        recorder.attach_to_prompt(&mut last, &mut HashMap::new());
        assert_eq!(tool_calls_complete(&last[1]), None);
    }
}

#[test]
fn completeness_rejects_reused_origin() {
    let recorder = new_recorder(InitialHistory::New);
    let first = CellId::new("first-runtime".to_string());
    recorder.start_cell(&first, "reused-exec");
    record_nested_call(&recorder, &first, "nested-first");
    recorder.finish_cell_recording(&first);
    let mut original = [exec_input("reused-exec"), exec_output("reused-exec")];
    recorder.attach_pending_to_prompt(&mut original, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&original[1]), Some(true));
    recorder.attach_pending_to_prompt(&mut [], &mut HashMap::new());

    let second = CellId::new("second-runtime".to_string());
    recorder.start_cell(&second, "reused-exec");
    record_nested_call(&recorder, &second, "nested-second");
    recorder.finish_cell_recording(&second);
    let mut prompt = [exec_input("reused-exec"), exec_output("reused-exec")];
    recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&prompt[1]), None);
}

#[test]
fn completeness_rejects_direct_id_collision_and_late_calls() {
    let recorder = new_recorder(InitialHistory::New);
    let cell = CellId::new("collision-runtime".to_string());
    recorder.start_cell(&cell, "exec-collision");
    recorder.record_call(
        &ToolCall {
            tool_name: codex_tools::ToolName::plain("direct_tool"),
            call_id: "exec-collision".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            encrypted_function_args: None,
        },
        &ToolCallSource::Direct,
        ToolMode::Direct,
    );
    record_nested_call(&recorder, &cell, "nested");
    recorder.finish_cell_recording(&cell);
    let mut collision = [exec_input("exec-collision"), exec_output("exec-collision")];
    recorder.attach_to_prompt(&mut collision, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&collision[1]), None);

    let recorder = new_recorder(InitialHistory::New);
    let cell = CellId::new("late-runtime".to_string());
    recorder.start_cell(&cell, "exec-late");
    record_nested_call(&recorder, &cell, "nested-first");
    recorder.finish_cell_recording(&cell);
    let mut first = [exec_input("exec-late"), exec_output("exec-late")];
    recorder.attach_to_prompt(&mut first, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&first[1]), Some(true));
    record_nested_call(&recorder, &cell, "nested-late");
    let mut late = [exec_input("exec-late"), exec_output("exec-late")];
    recorder.attach_to_prompt(&mut late, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&late[1]), None);
}

#[test]
fn request_truncation_prevents_completion_after_compaction() {
    let recorder = new_recorder(InitialHistory::New);
    let cell_id = CellId::new("compacted-cell".to_string());
    recorder.start_cell(&cell_id, "exec");

    let arguments = json!({
        "payload": "x".repeat(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES - r#"{"payload":""}"#.len()),
    });
    for index in 0..4 {
        recorder.record_nested_tool_call(
            cell_id.clone(),
            format!("nested-{index}"),
            ExecutedToolCall::new("nested_tool".to_string(), arguments.clone()),
            MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES,
        );
    }

    let mut initial = [exec_input("exec"), exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut initial, &mut HashMap::new()));
    assert!(
        initial[1]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.executed_tool_calls.as_ref())
            .is_some_and(|calls| calls.iter().any(|call| matches!(
                call.arguments(),
                ExecutedToolCallArguments::Truncated { .. }
            )))
    );
    let source = ToolCallSource::CodeMode {
        cell_id: cell_id.as_str().to_string(),
        runtime_tool_call_id: "runtime-call".to_string(),
    };
    for index in 0..4 {
        assert!(!recorder.record_tool_result_metadata(
            &source,
            &format!("nested-{index}"),
            &json!({}),
        ));
    }

    recorder.attach_pending_to_prompt(&mut [], &mut HashMap::new());
    recorder.register_cell(&cell_id, "wait");
    recorder.finish_cell_recording(&cell_id);
    let mut final_output = [wait_input("wait", &cell_id), output("wait")];
    recorder.attach_pending_to_prompt(&mut final_output, &mut HashMap::new());
    assert_eq!(
        final_output[1]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.tool_calls_complete),
        None,
    );
}

#[test]
fn result_metadata_updates_the_exact_retained_call_and_marks_oversized_retries() {
    let recorder = new_recorder(InitialHistory::Forked(Vec::new()));
    let cell_id = CellId::new("metadata-cell".to_string());
    recorder.start_cell(&cell_id, "exec");
    let call = ExecutedToolCall::new("nested_tool".to_string(), json!({}));
    for call_id in ["first", "second"] {
        recorder.record_nested_tool_call(
            cell_id.clone(),
            call_id.to_string(),
            call.clone(),
            /*original_bytes*/ 2,
        );
    }
    recorder.finish_cell_recording(&cell_id);
    let mut retry_cache = HashMap::new();
    let mut initial = [exec_input("exec"), exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut initial, &mut retry_cache));

    let metadata = json!({ "all-keys": { "ids": ["R1"] }, "other": true });
    let source = |cell_id: &str| ToolCallSource::CodeMode {
        cell_id: cell_id.to_string(),
        runtime_tool_call_id: "runtime-call".to_string(),
    };
    assert!(!recorder.record_tool_result_metadata(&source("other-cell"), "first", &metadata));
    assert!(!recorder.record_tool_result_metadata(&source(cell_id.as_str()), "unknown", &metadata));
    assert!(recorder.record_tool_result_metadata(&source(cell_id.as_str()), "first", &metadata));

    let mut expected_call = call.clone();
    expected_call.set_tool_result_metadata(ToolResultMetadata::new(&metadata));
    let mut expected = exec_output("exec");
    expected.append_executed_tool_calls(vec![expected_call, call]);
    expected.set_tool_call_cell_id("exec");
    expected.mark_tool_calls_complete();
    let mut retry = [exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut retry, &mut retry_cache));
    assert_eq!(retry, [expected]);

    assert!(recorder.record_tool_result_metadata(
        &source(cell_id.as_str()),
        "first",
        &json!({ "oversized": "x".repeat(32 * 1024) }),
    ));
    let mut expected = serde_json::to_value(&initial[1..]).unwrap();
    expected[0]["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0]["tool_result_metadata"] =
        json!("omitted_due_to_size_limit");
    let mut retry = [exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut retry, &mut retry_cache));
    assert_eq!(serde_json::to_value(&retry).unwrap(), expected);
}

#[test]
fn untracked_history_budget_loss_keeps_later_wait_incomplete() {
    let recorder = new_recorder(InitialHistory::New);
    let cell = CellId::new("running-cell".to_string());
    recorder.start_cell(&cell, "exec");
    recorder.record_nested_tool_call(
        cell.clone(),
        "nested".to_string(),
        ExecutedToolCall::new("nested_tool".to_string(), json!("x".repeat(8190))),
        /*original_bytes*/ 8192,
    );
    let mut prompt = vec![exec_input("exec"), exec_output("exec")];
    for index in 0..64 {
        let mut item = output(&format!("untracked-{index}"));
        item.append_executed_tool_calls(vec![ExecutedToolCall::new(
            "other_tool".to_string(),
            json!("x".repeat(1024)),
        )]);
        prompt.push(item);
    }
    recorder.attach_to_prompt(&mut prompt, &mut HashMap::new());
    assert!(prompt[1].executed_tool_call_metadata().is_none());

    recorder.attach_to_prompt(&mut [], &mut HashMap::new());
    recorder.register_cell(&cell, "wait");
    recorder.finish_cell_recording(&cell);
    let mut wait = [wait_input("wait", &cell), output("wait")];
    recorder.attach_to_prompt(&mut wait, &mut HashMap::new());
    assert_eq!(tool_calls_complete(&wait[1]), None);
}

#[test]
fn result_metadata_shedding_preserves_completion_after_compaction() {
    let recorder = new_recorder(InitialHistory::New);
    let cell_id = CellId::new("compacted-cell".to_string());
    recorder.start_cell(&cell_id, "exec");

    let argument_bytes = MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES - 256;
    let arguments = json!({
        "payload": "x".repeat(argument_bytes - r#"{"payload":""}"#.len()),
    });
    for index in 0..4 {
        recorder.record_nested_tool_call(
            cell_id.clone(),
            format!("nested-{index}"),
            ExecutedToolCall::new("nested_tool".to_string(), arguments.clone()),
            argument_bytes,
        );
    }
    let source = ToolCallSource::CodeMode {
        cell_id: cell_id.as_str().to_string(),
        runtime_tool_call_id: "runtime-call".to_string(),
    };
    let mut retry_cache = HashMap::new();
    recorder.attach_pending_to_prompt(
        &mut [exec_input("exec"), exec_output("exec")],
        &mut retry_cache,
    );
    // This snapshot fits alone, but pushes the four calls over the aggregate budget.
    assert!(recorder.record_tool_result_metadata(
        &source,
        "nested-0",
        &json!({ "provider_data": "x".repeat(4 * 1024) }),
    ));

    assert!(recorder.record_tool_result_metadata(
        &source,
        "nested-1",
        &json!({ "status": "unavailable" })
    ));
    let mut initial = [exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut initial, &mut retry_cache));
    let calls = initial[0]
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.executed_tool_calls.as_ref())
        .expect("metadata shedding must preserve recorded calls");
    let mut expected_calls = vec![ExecutedToolCall::new("nested_tool".to_string(), arguments); 4];
    for call in &mut expected_calls[..2] {
        call.set_tool_result_metadata(ToolResultMetadata::new(&json!("omitted_due_to_size_limit")));
    }
    assert_eq!(calls, &expected_calls);
    let mut retry = [exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut retry, &mut retry_cache));
    assert_eq!(retry, initial);

    // A different call can still attach a small result after the oversized metadata is shed.
    let metadata = json!({ "id": "R2" });
    assert!(recorder.record_tool_result_metadata(&source, "nested-2", &metadata));
    let mut expected_calls = calls.clone();
    expected_calls[2].set_tool_result_metadata(ToolResultMetadata::new(&metadata));
    let mut expected = exec_output("exec");
    expected.append_executed_tool_calls(expected_calls);
    expected.set_tool_call_cell_id("exec");
    let mut retry = [exec_output("exec")];
    assert!(recorder.attach_pending_to_prompt(&mut retry, &mut retry_cache));
    assert_eq!(retry, [expected]);

    recorder.attach_pending_to_prompt(&mut [], &mut retry_cache);
    assert!(retry_cache.is_empty());
    assert!(!recorder.record_tool_result_metadata(&source, "nested-2", &metadata));
    recorder.register_cell(&cell_id, "wait");
    recorder.finish_cell_recording(&cell_id);
    let mut final_output = [wait_input("wait", &cell_id), output("wait")];
    recorder.attach_pending_to_prompt(&mut final_output, &mut HashMap::new());
    assert_eq!(
        final_output[1]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.tool_calls_complete),
        Some(true),
    );
}

#[test]
fn finished_cells_without_more_waits_do_not_block_new_calls() {
    let recorder = new_recorder(InitialHistory::Forked(Vec::new()));
    let call = ExecutedToolCall::new("nested_tool".to_string(), json!({}));
    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS {
        let cell = CellId::new(format!("cell-{index}"));
        recorder.start_cell(&cell, cell.as_str());
        recorder.record_nested_tool_call(
            cell.clone(),
            format!("nested-{index}"),
            call.clone(),
            /*original_bytes*/ 2,
        );
        recorder.attach_pending_to_prompt(
            &mut [exec_input(cell.as_str()), exec_output(cell.as_str())],
            &mut HashMap::new(),
        );
        recorder.finish_cell_recording(&cell);
    }
    let fresh = CellId::new("fresh".to_string());
    recorder.start_cell(&fresh, "fresh-output");
    recorder.record_nested_tool_call(
        fresh.clone(),
        "fresh-call".to_string(),
        call.clone(),
        /*original_bytes*/ 2,
    );
    recorder.finish_cell_recording(&fresh);
    let mut expected = exec_output("fresh-output");
    expected.append_executed_tool_calls(vec![call]);
    expected.set_tool_call_cell_id("fresh-output");
    expected.mark_tool_calls_complete();
    let mut items = [exec_input("fresh-output"), exec_output("fresh-output")];
    assert!(recorder.attach_pending_to_prompt(&mut items, &mut HashMap::new()));
    assert_eq!(items, [exec_input("fresh-output"), expected]);
}
