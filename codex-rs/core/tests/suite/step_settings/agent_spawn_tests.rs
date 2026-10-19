//! Spawned agents inherit the invoking step's model settings after active-turn updates.

use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(false, None; "v1 inherits captured settings")]
#[test_case(true, None; "v2 inherits captured settings")]
#[test_case(false, Some(ReasoningEffort::Medium); "v1 validates effort against captured model")]
#[test_case(true, Some(ReasoningEffort::Medium); "v2 validates effort against captured model")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_inherits_captured_settings_after_a_turn_update(
    multi_agent_v2: bool,
    requested_effort: Option<ReasoningEffort>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut spawn_arguments = json!({ "message": "Complete the delegated task." });
    let namespace = if multi_agent_v2 {
        spawn_arguments["task_name"] = json!("worker");
        spawn_arguments["fork_turns"] = json!("none");
        "collaboration"
    } else {
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
    let request = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: Some(ReasoningSummary::Detailed),
            ..Default::default()
        },
    )
    .await?;
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
