//! Children inherit captured model settings and persistent environment attachments.

use super::*;
use codex_exec_server::CreateDirectoryOptions;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentRequests;
use codex_protocol::protocol::TurnEnvironmentSelection;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::environment_config_for_selection;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy)]
enum EnvironmentInheritanceCase {
    Configured,
    Legacy,
    Empty,
    Fork,
    Pending,
}

#[test_case(false, None, EnvironmentInheritanceCase::Configured; "v1 inherits captured settings")]
#[test_case(true, None, EnvironmentInheritanceCase::Configured; "v2 inherits captured settings")]
#[test_case(false, Some(ReasoningEffort::Medium), EnvironmentInheritanceCase::Configured; "v1 validates effort against captured model")]
#[test_case(true, Some(ReasoningEffort::Medium), EnvironmentInheritanceCase::Configured; "v2 validates effort against captured model")]
#[test_case(false, None, EnvironmentInheritanceCase::Empty; "v1 excludes turn-only attachments")]
#[test_case(true, None, EnvironmentInheritanceCase::Empty; "v2 excludes turn-only attachments")]
#[test_case(false, None, EnvironmentInheritanceCase::Fork; "v1 fork inherits configured attachments")]
#[test_case(true, None, EnvironmentInheritanceCase::Fork; "v2 fork inherits configured attachments")]
#[test_case(false, None, EnvironmentInheritanceCase::Pending; "v1 retains replaced pending attachment")]
#[test_case(true, None, EnvironmentInheritanceCase::Pending; "v2 retains replaced pending attachment")]
#[test_case(false, None, EnvironmentInheritanceCase::Legacy; "v1 retains turn attachments before rollout")]
#[test_case(true, None, EnvironmentInheritanceCase::Legacy; "v2 retains turn attachments before rollout")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_inherits_captured_settings_after_a_turn_update(
    multi_agent_v2: bool,
    requested_effort: Option<ReasoningEffort>,
    inheritance: EnvironmentInheritanceCase,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut spawn_arguments = json!({ "message": "Complete the delegated task." });
    let namespace = if multi_agent_v2 {
        spawn_arguments["task_name"] = json!("worker");
        spawn_arguments["fork_turns"] =
            json!(if matches!(inheritance, EnvironmentInheritanceCase::Fork) {
                "all"
            } else {
                "none"
            });
        "collaboration"
    } else {
        spawn_arguments["fork_context"] =
            json!(matches!(inheritance, EnvironmentInheritanceCase::Fork));
        "multi_agent_v1"
    };
    if let Some(effort) = &requested_effort {
        spawn_arguments["reasoning_effort"] = json!(effort);
    }
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-original", "pause-before-spawn"),
            sse(vec![
                ev_response_created("resp-spawn"),
                ev_function_call_with_namespace(
                    "spawn-worker",
                    namespace,
                    "spawn_agent",
                    &spawn_arguments.to_string(),
                ),
                ev_completed("resp-spawn"),
            ]),
            sse_completed("resp-first-completion"),
            sse_completed("resp-second-completion"),
        ],
    )
    .await;
    // Child completion can trigger another parent request after the parent finishes.
    mount_sse_once(&server, sse_completed("resp-child-notification")).await;
    let test = step_settings_test()
        .with_config(move |config| {
            if !matches!(inheritance, EnvironmentInheritanceCase::Legacy) {
                config
                    .features
                    .enable(Feature::SubagentThreadEnvironments)
                    .expect("enable configured environment inheritance");
            }
            config
                .features
                .enable(Feature::DeferredExecutor)
                .expect("enable deferred executor");
            config
                .features
                .enable(Feature::Collab)
                .expect("enable collab");
            if multi_agent_v2 {
                config
                    .features
                    .enable(Feature::MultiAgentV2)
                    .expect("enable V2");
            } else {
                config
                    .features
                    .disable(Feature::MultiAgentV2)
                    .expect("disable V2");
            }
            for model in &mut config.model_catalog.as_mut().expect("model catalog").models {
                // An effort-only override must be validated against B, not the turn's initial A.
                model.supported_reasoning_levels = if model.slug == MODEL_B {
                    vec![ReasoningEffort::Medium, ReasoningEffort::High]
                } else {
                    vec![ReasoningEffort::Low]
                }
                .into_iter()
                .map(|effort| ReasoningEffortPreset {
                    description: effort.to_string(),
                    effort,
                })
                .collect();
                model.supports_reasoning_summary_parameter = true;
                model.use_responses_lite = false;
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    if matches!(inheritance, EnvironmentInheritanceCase::Empty) {
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    Vec::new(),
                )),
                ..Default::default()
            },
        )
        .await?;
    }
    let mut configured = test.codex.environment_selections().await;
    if matches!(inheritance, EnvironmentInheritanceCase::Pending) {
        configured[0].config = EnvironmentConfigState::Pending;
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    configured
                        .clone()
                        .into_iter()
                        .map(TurnEnvironmentSelection::into_request)
                        .collect(),
                )),
                ..Default::default()
            },
        )
        .await?;
    }
    let mut temporary = test.executor_environment().selection().clone();
    temporary.cwd = test.workspace_path_uri("turn-only-workspace")?;
    temporary.workspace_roots = vec![temporary.cwd.clone()];
    temporary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &temporary));
    test.fs()
        .create_directory(
            &temporary.cwd,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    if matches!(inheritance, EnvironmentInheritanceCase::Legacy) {
        configured = vec![temporary.clone()];
    }
    let request = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: Some(ReasoningSummary::Detailed),
            environments: Some(vec![temporary.into_request()]),
            ..Default::default()
        },
    )
    .await?;
    if matches!(inheritance, EnvironmentInheritanceCase::Pending) {
        let config = environment_config_for_selection(&test.config, &configured[0]);
        test.codex
            .environment_ready(&configured[0], config.clone())
            .await?;
        configured[0].config = EnvironmentConfigState::Ready(config);
    }
    answer_paused_turn(&test.codex, &request.turn_id).await?;

    let child_thread_id = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 10),
        created_threads.recv(),
    )
    .await??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    for thread in [child_thread.as_ref(), test.codex.as_ref()] {
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    }
    assert_eq!(child_thread.environment_selections().await, configured);

    let expected_effort = requested_effort.unwrap_or(ReasoningEffort::High);
    let requests = response_mock.requests();
    let child_request = requests
        .iter()
        .find(|request| request.header("thread-id") == Some(child_thread_id.to_string()))
        .expect("child should make a model request");
    assert_eq!(
        request_settings(child_request),
        json!({
            "model": MODEL_B,
            "reasoning": { "effort": expected_effort, "summary": "detailed" },
            "service_tier": null,
        })
    );

    Ok(())
}
