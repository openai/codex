//! Dynamic tool replacement takes effect between model requests, including after restart.

use super::*;
use core_test_support::responses;
use pretty_assertions::assert_eq;

fn configure_dynamic_tool_scenario(config: &mut Config) {
    config.model = Some("gpt-6-astra".into());
    configure_scenario_catalog(config);
    config.include_environment_context = false;
    config
        .features
        .disable(Feature::CodeMode)
        .expect("disable code mode");
    let model = config
        .model_catalog
        .as_mut()
        .expect("scenario model catalog")
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-6-astra")
        .expect("Astra model");
    model.use_responses_lite = false;
    model.tool_mode = Some(ToolMode::Direct);
}

fn tool(name: &str) -> DynamicToolSpec {
    DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: name.to_string(),
        description: format!("Check {name}."),
        input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        defer_loading: false,
    })
}

fn has_tool(body: serde_json::Value, name: &str) -> bool {
    body["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("request has no tools: {body}"))
        .iter()
        .any(|tool| tool["name"] == name)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_tools_update_preserves_pending_calls_and_survives_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let enable_updates = |config: &mut Config| config.allow_persistent_dynamic_tool_updates = true;
    let mut builder = test_codex()
        .with_config(configure_dynamic_tool_scenario)
        .with_config(enable_updates);
    let mut test = builder.build_with_auto_env(&server).await?;
    let first = responses::mount_sse_once(
        &server,
        sse(vec![
            responses::ev_function_call("release-call", "release", "{}"),
            ev_completed("first"),
        ]),
    )
    .await;
    let (reply, updated) = tokio::sync::oneshot::channel();
    test.codex
        .submit(Op::ReplaceDynamicTools {
            tools: vec![tool("release")],
            reply,
        })
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Check the release and deployment.",
        )]))
        .await?;
    updated.await??;
    let call = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(call) => Some(call.clone()),
        _ => None,
    })
    .await;
    assert_eq!(call.tool, "release");
    test.codex
        .replace_dynamic_tools(vec![tool("deployment")])
        .await?;
    let second = responses::mount_sse_once(
        &server,
        sse(vec![
            responses::ev_function_call("deployment-call", "deployment", "{}"),
            ev_completed("second"),
        ]),
    )
    .await;
    test.codex
        .submit(Op::DynamicToolResponse {
            id: call.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Release ready.".into(),
                }],
                success: true,
            },
        })
        .await?;
    let call = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(call) => Some(call.clone()),
        _ => None,
    })
    .await;
    assert_eq!(call.tool, "deployment");
    let third = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("ready", "Both checks passed."),
            ev_completed("third"),
        ]),
    )
    .await;
    test.codex
        .submit(Op::DynamicToolResponse {
            id: call.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Deployment ready.".into(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(has_tool(first.single_request().body_json(), "release"));
    assert!(!has_tool(second.single_request().body_json(), "release"));
    assert!(has_tool(second.single_request().body_json(), "deployment"));
    assert_eq!(
        second
            .single_request()
            .function_call_output_content_and_success("release-call"),
        Some((Some("Release ready.".into()), None))
    );
    assert_eq!(
        third
            .single_request()
            .function_call_output_content_and_success("deployment-call"),
        Some((Some("Deployment ready.".into()), None))
    );

    builder = builder.with_config(configure_dynamic_tool_scenario);
    test = builder.restart_with_auto_env(&server, &test).await?;
    let restored = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("restored", "The deployment tool is available."),
            ev_completed("restored"),
        ]),
    )
    .await;
    test.submit_text_turn("Which check is available now?")
        .await?;
    assert!(has_tool(
        restored.single_request().body_json(),
        "deployment"
    ));
    assert!(!has_tool(restored.single_request().body_json(), "release"));

    builder = builder
        .with_config(configure_dynamic_tool_scenario)
        .with_config(enable_updates);
    test = builder.restart_with_auto_env(&server, &test).await?;
    test.codex.replace_dynamic_tools(Vec::new()).await?;
    builder = builder.with_config(configure_dynamic_tool_scenario);
    test = builder.restart_with_auto_env(&server, &test).await?;
    let restarted = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("done", "The checks are complete."),
            ev_completed("restarted"),
        ]),
    )
    .await;
    test.submit_text_turn("Summarize the checks.").await?;
    assert!(!has_tool(restarted.single_request().body_json(), "release"));
    assert!(!has_tool(
        restarted.single_request().body_json(),
        "deployment"
    ));
    let requests = responses::received_responses_requests(&server).await;
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "A pending release call survives replacement with deployment. The next step uses deployment. Replacement and clearing each survive restart.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_tools_resume_override_stays_local_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(configure_dynamic_tool_scenario)
        .with_config(|config| {
            config.features.enable(Feature::TokenBudget).unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let start = async |initial_history, dynamic_tools| {
        test.thread_manager
            .start_thread(StartThreadOptions {
                initial_history,
                dynamic_tools,
                history_mode: Some(codex_protocol::protocol::ThreadHistoryMode::Legacy),
                ..test.start_thread_options().await
            })
            .await
    };
    let initial = start(codex_history::InitialHistory::New, vec![tool("release")]).await?;
    initial.thread.ensure_rollout_materialized().await;
    initial.thread.shutdown_and_wait().await?;
    let path = initial.thread.rollout_path().expect("rollout path");
    let load_history = || codex_rollout::RolloutRecorder::get_rollout_history(&path);
    let check_tools = async |thread: &codex_core::CodexThread, present: &str, absent: &str| {
        let response = responses::mount_sse_once(&server, responses::sse_completed(present)).await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
                "Check the release.",
            )]))
            .await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let request = response.single_request();
        assert!(has_tool(request.body_json(), present));
        assert!(!has_tool(request.body_json(), absent));
        Ok::<_, anyhow::Error>(())
    };

    let overridden = start(load_history().await?, vec![tool("deployment")])
        .await?
        .thread;
    check_tools(&overridden, "deployment", "release").await?;
    overridden.submit(Op::Compact).await?;
    wait_for_event(&overridden, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    overridden.shutdown_and_wait().await?;

    let history = load_history().await?;
    assert!(
        history
            .get_rollout_items()
            .iter()
            .any(|item| matches!(item, codex_history::RolloutItem::Compacted(_))),
        "resume must read the persisted compaction checkpoint"
    );
    let restored = start(history, Vec::new()).await?.thread;
    check_tools(&restored, "release", "deployment").await?;
    restored.shutdown_and_wait().await?;
    Ok(())
}
