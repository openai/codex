//! Checks request-history trimming at the streamed remote compaction boundary.

use super::*;
use core_test_support::apps_test_server::configure_search_capable_model;
use pretty_assertions::assert_eq;

fn compact_response() -> String {
    sse(vec![
        json!({
            "type": "response.output_item.done",
            "item": {"type": "compaction", "encrypted_content": "TRIMMING_SUMMARY"},
        }),
        responses::ev_completed("compact"),
    ])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_token_estimate_ignores_message_bookkeeping_and_json_escaping()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_auto_env_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let escaped_text = "a \"quoted\" line\nand\\path";
    let plain_text = "x".repeat(escaped_text.len());
    let mut estimates = Vec::new();
    for (id, text, metadata) in [
        (
            "msg_1",
            plain_text.as_str(),
            json!({
                "turn_id": "turn-1",
                "create_time": 1,
                "content_item_kinds": ["unknown"],
            }),
        ),
        (
            "msg_019f0000-0000-7000-8000-000000000001",
            escaped_text,
            json!({
                "turn_id": "019f0000-0000-7000-8000-000000000002",
                "create_time": 1_700_000_000.123,
                "content_item_kinds": ["user.text"],
            }),
        ),
    ] {
        let codex = harness
            .test()
            .thread_manager
            .start_thread(StartThreadOptions {
                environments: Some(vec![
                    harness.test().executor_environment().selection().clone(),
                ]),
                ..StartThreadOptions::new(harness.test().config.clone())
            })
            .await?
            .thread;
        let message = json!({
            "type": "message",
            "id": id,
            "role": "user",
            "content": [{"type": "input_text", "text": text}],
            "internal_chat_message_metadata_passthrough": metadata,
        });
        codex
            .inject_response_items(vec![serde_json::from_value(message.clone())?])
            .await?;
        let mock = mount_sse_once(harness.server(), compact_response()).await;
        codex.submit(Op::Compact).await?;
        wait_for_turn_complete(&codex).await;

        estimates.push(codex.token_usage_info().await.context("token usage")?);
        assert_eq!(
            mock.single_request()
                .input()
                .into_iter()
                .find(|item| item["id"] == id),
            Some(message),
        );
        codex.shutdown_and_wait().await?;
    }

    assert_eq!(estimates[0], estimates[1]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_trims_tool_search_output_to_empty_tools_array() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "tool-search-1";
    let tools = json!([{
        "type": "namespace", "name": "codex_app", "description": "Codex app tools.",
        "tools": [{
            "type": "function", "name": "oversized_dynamic_tool", "description": "x".repeat(20_000),
            "parameters": {"type": "object", "properties": {}, "additionalProperties": false},
            "strict": false, "defer_loading": true,
        }],
    }]);
    for (context_window, expected_tools) in [(200_000, &tools), (2_000, &json!([]))] {
        let harness = TestCodexHarness::with_builder(
            test_codex()
                .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
                .with_config(move |config| {
                    configure_search_capable_model(config);
                    config.model_context_window = Some(context_window);
                }),
        )
        .await?;
        let codex = &harness.test().codex;
        let history = [
            json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "find the tool"}]}),
            json!({"type": "tool_search_call", "call_id": call_id, "execution": "client", "arguments": {"query": "oversized deferred tool"}}),
            json!({"type": "tool_search_output", "call_id": call_id, "status": "completed", "execution": "client", "tools": &tools}),
        ]
        .into_iter()
        .map(serde_json::from_value)
        .collect::<serde_json::Result<Vec<ResponseItem>>>()?;
        codex.inject_response_items(history).await?;
        let mock = mount_sse_once(harness.server(), compact_response()).await;
        codex.submit(Op::Compact).await?;
        wait_for_turn_complete(codex).await;

        let request = mock.single_request();
        assert_eq!(request.path(), "/v1/responses");
        assert_eq!(request.inputs_of_type("compaction_trigger").len(), 1);
        assert!(
            request
                .inputs_of_type("tool_search_call")
                .iter()
                .any(|item| item["call_id"] == call_id)
        );
        let output = request.tool_search_output(call_id);
        assert_eq!(&output["tools"], expected_tools);
        assert_eq!(output["call_id"], call_id);
        assert_eq!(output["status"], "completed");
        assert_eq!(output["execution"], "client");
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_trim_estimate_uses_session_base_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let short_instructions = "session base instructions";
    let long_instructions = format!("{short_instructions} {}", "x".repeat(24_000));
    let trailing_output = "x".repeat(12_000);
    for (instructions, expected_output) in [
        (short_instructions, trailing_output.as_str()),
        (
            long_instructions.as_str(),
            CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE,
        ),
    ] {
        let harness = TestCodexHarness::with_builder(
            test_codex()
                .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
                .with_config({
                    let instructions = instructions.to_string();
                    move |config| {
                        config.model_context_window = Some(8_000);
                        config.base_instructions = Some(instructions);
                    }
                }),
        )
        .await?;
        let codex = &harness.test().codex;
        let history = [
            json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "older user"}]}),
            json!({"type": "function_call", "call_id": "retained", "name": "exec_command", "arguments": "{}"}),
            json!({"type": "function_call_output", "call_id": "retained", "output": "retained output"}),
            json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "user boundary"}]}),
            json!({"type": "function_call", "call_id": "trailing", "name": "exec_command", "arguments": "{}"}),
            json!({"type": "function_call_output", "call_id": "trailing", "output": &trailing_output}),
        ]
        .into_iter()
        .map(serde_json::from_value)
        .collect::<serde_json::Result<Vec<ResponseItem>>>()?;
        codex.inject_response_items(history).await?;
        let mock = mount_sse_once(harness.server(), compact_response()).await;
        codex.submit(Op::Compact).await?;
        wait_for_turn_complete(codex).await;

        let request = mock.single_request();
        assert_eq!(request.path(), "/v1/responses");
        assert_eq!(request.inputs_of_type("compaction_trigger").len(), 1);
        assert_eq!(request.instructions_text(), instructions);
        assert!(request.has_function_call("retained"));
        assert!(request.has_function_call("trailing"));
        assert_eq!(
            request.function_call_output_text("retained").as_deref(),
            Some("retained output")
        );
        assert_eq!(
            request.function_call_output_text("trailing").as_deref(),
            Some(expected_output)
        );
    }
    Ok(())
}
