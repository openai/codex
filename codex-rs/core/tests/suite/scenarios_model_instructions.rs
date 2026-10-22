//! Model-visible instructions and tools from catalog routing, refresh, and compaction.

use super::*;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use core_test_support::responses;
use pretty_assertions::assert_eq;

/// Originator routing supplies the actual turn's instructions and tools without changing legacy callers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn originator_catalogs_control_turn_instructions_and_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    for (originator, instructions, patch_tool) in [
        ("provider-default", "Default catalog instructions.", None),
        (
            "codex_work_cca",
            "Work catalog instructions.",
            Some(ApplyPatchToolType::Freeform),
        ),
        ("chatgpt_cca", "Chat catalog instructions.", None),
    ] {
        let mut model = model_info_from_slug("gpt-5.5");
        model.visibility = ModelVisibility::List;
        model.use_responses_lite = false;
        model.tool_mode = Some(ToolMode::Direct);
        model.apply_patch_tool_type = patch_tool;
        model
            .model_messages
            .get_or_insert_default()
            .instructions_template = Some(instructions.into());
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .and(wiremock::matchers::header("originator", originator))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(ModelsResponse {
                    models: vec![model],
                }),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    let mock = mount_sse_sequence(
        &server,
        (1..=5)
            .map(|index| sse(vec![ev_completed(&format!("response-{index}"))]))
            .collect(),
    )
    .await;
    let mut test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.model_catalog = None;
            config
                .model_provider
                .http_headers
                .get_or_insert_default()
                .insert("originator".into(), "provider-default".into());
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Describe the task.").await?;
    for originator in ["codex_work_cca", "chatgpt_cca", "unknown", "codex_work_cca"] {
        let started = test
            .thread_manager
            .start_thread(StartThreadOptions {
                metrics_service_name: Some(originator.into()),
                ..test.start_thread_options().await
            })
            .await?;
        test.codex = started.thread;
        test.session_configured = started.session_configured;
        test.submit_turn("Describe the task.").await?;
    }
    let requests = mock.requests();
    assert_eq!(requests.len(), 5);
    for (request, expected) in requests.iter().zip([
        "Default catalog instructions.",
        "Work catalog instructions.",
        "Chat catalog instructions.",
        "Default catalog instructions.",
        "Work catalog instructions.",
    ]) {
        assert_eq!(request.instructions_text(), expected);
    }
    insta::assert_snapshot!(
        "originator_catalog_instructions_and_tools",
        context_snapshot::format_request_history_snapshot(
            "Default, Work, Chat, unsupported, and repeated Work threads use their own catalog instructions and tools. Unsupported callers preserve the default request; repeated Work reuses its catalog.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    let tools = requests
        .iter()
        .map(|request| {
            request.body_json()["tools"]
                .as_array()
                .expect("Responses tool catalog")
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(tools[0], tools[3]);
    assert_eq!(tools[1], tools[4]);
    assert_eq!(
        tools
            .iter()
            .map(|declarations| declarations
                .iter()
                .any(|tool| tool["name"] == "apply_patch"))
            .collect::<Vec<_>>(),
        vec![false, true, false, false, true],
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_refresh_replaces_instructions_and_compaction_installs_latest_base() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let mut model = model_info_from_slug("gpt-5.4");
    model.use_responses_lite = true;
    model.visibility = ModelVisibility::List;
    model
        .model_messages
        .get_or_insert_default()
        .instructions_template = Some("Help the user complete their task.".to_string());
    responses::mount_models_once_with_etag(
        &server,
        ModelsResponse {
            models: vec![model.clone()],
        },
        "\"instructions-v1\"",
    )
    .await;
    let mock = mount_sse_sequence(
        &server,
        (1..=5)
            .map(|index| {
                let mut events = Vec::new();
                if index == 4 {
                    events.push(json!({
                        "type": "response.output_item.done",
                        "item": {
                            "type": "compaction",
                            "encrypted_content": "MODEL_INSTRUCTIONS_CHECKPOINT",
                        },
                    }));
                }
                events.push(ev_completed(&format!("resp-{index}")));
                sse(events)
            })
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model(&model.slug)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.workspace_roots = vec![config.cwd.clone()];
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Begin work.").await?;

    model
        .model_messages
        .get_or_insert_default()
        .instructions_template = Some("Check your work before responding.".to_string());
    responses::mount_models_once_with_etag(
        &server,
        ModelsResponse {
            models: vec![model],
        },
        "\"instructions-v2\"",
    )
    .await;
    test.thread_manager
        .get_models_manager()
        .list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    test.submit_turn("Continue with refreshed instructions.")
        .await?;
    test.submit_turn("Continue without further changes.")
        .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("Continue after compaction.").await?;

    insta::assert_snapshot!(
        "incremental_model_instructions",
        context_snapshot::format_request_history_snapshot(
            "A same-model catalog refresh appends replacement instructions once without changing tools or the prefix. Compaction starts a new window with the latest base.",
            &mock.requests(),
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
