//! Model coverage for subagents inheriting the first pending environment result.

use super::*;
use core_test_support::responses::sse_response;
use pretty_assertions::assert_eq;
use test_case::test_case;

const SPAWN: &str = "spawn-environment-worker";
const WAIT_CHILD: &str = "wait-for-environment-worker";
const WAIT_ENV: &str = "wait-for-inherited-environment";
const NEXT_TURN: &str = "child-next-turn-after-owner-change";
const FAILURE: &str = "owner could not prepare the subagent environment";

#[derive(Clone, Copy)]
pub(crate) enum PendingSpawnCase {
    TurnSettings,
    V1Fork,
    Failure,
    FirstResultOnly,
    ChildConfiguredFirst,
}

fn call_output<'a>(body: &'a Value, call_id: &str) -> Option<&'a str> {
    let output = body["input"]
        .as_array()?
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == call_id)?;
    let output = &output["output"];
    output
        .as_str()
        .or_else(|| output["content"].as_str())
        .or_else(|| output[0]["text"].as_str())
}

// The parent and child run concurrently; these stages give the snapshot a stable causal order.
fn request_stage(body: &Value, root_id: &str) -> usize {
    if body["client_metadata"]["thread_id"] == root_id {
        if call_output(body, WAIT_CHILD).is_some() {
            4
        } else if call_output(body, SPAWN).is_some() {
            1
        } else {
            0
        }
    } else if body["input"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["role"] == "user" && item["content"].to_string().contains(NEXT_TURN))
    }) {
        5
    } else if call_output(body, WAIT_ENV).is_some() {
        3
    } else {
        2
    }
}

pub(crate) async fn pending_subagent_scenario(
    case: PendingSpawnCase,
    configure_scenario: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<Vec<Value>> {
    let v1 = matches!(case, PendingSpawnCase::V1Fork);
    let failed = matches!(case, PendingSpawnCase::Failure);
    let server = start_mock_server().await;
    let test = test_codex_with_wait_for_environment()
        .with_config(move |config| {
            config.project_doc_max_bytes = 0;
            for (feature, enabled) in [
                (Feature::DeferredExecutor, true),
                (Feature::Collab, true),
                (Feature::DeferMailboxPreemption, true),
                (Feature::EnableRequestCompression, false),
                (Feature::MultiAgentV2, !v1),
            ] {
                config
                    .features
                    .set_enabled(feature, enabled)
                    .expect("configure subagent feature");
            }
            configure_scenario(config);
        })
        .build_with_auto_env(&server)
        .await?;
    let selection = test.executor_environment().selection().clone();
    let pending = TurnEnvironmentSelection {
        config: EnvironmentConfigState::Pending,
        ..selection.clone()
    };
    let root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![pending.clone().into_request()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    let root_id = root.thread_id.to_string();
    let matcher_root_id = root_id.clone();
    let environment_id = selection.environment_id.clone();
    let parent_requested = Arc::new(tokio::sync::Notify::new());
    let child_requested = Arc::new(tokio::sync::Notify::new());
    let notify_parent_request = Arc::clone(&parent_requested);
    let notify_child_request = Arc::clone(&child_requested);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = request.body_json().expect("model request body");
            let namespace = if v1 {
                "multi_agent_v1"
            } else {
                "collaboration"
            };
            let tool = |id, namespace, name, args: Value| {
                ev_function_call_with_namespace(id, namespace, name, &args.to_string())
            };
            let stage = request_stage(&body, &matcher_root_id);
            let item = match stage {
                0 => {
                    let message = "Wait for the shared workspace to become ready.";
                    let args = if v1 {
                        json!({"message": message, "fork_context": true})
                    } else {
                        json!({"task_name": "worker", "message": message, "fork_turns": "none"})
                    };
                    tool(SPAWN, namespace, "spawn_agent", args)
                }
                1 => {
                    notify_parent_request.notify_one();
                    let mut args = json!({"timeout_ms": 10000});
                    if v1 {
                        let output = call_output(&body, SPAWN).expect("spawn result");
                        let output: Value =
                            serde_json::from_str(output).expect("spawn result JSON");
                        args["targets"] = json!([output["agent_id"]]);
                    }
                    tool(WAIT_CHILD, namespace, "wait_agent", args)
                }
                2 => {
                    notify_child_request.notify_one();
                    tool(
                        WAIT_ENV,
                        "functions",
                        "wait_for_environment",
                        json!({"environment_id": environment_id}),
                    )
                }
                3..=5 => ev_assistant_message(&format!("message-{stage}"), "Done."),
                _ => unreachable!("known request stage"),
            };
            let response_id = format!("response-{stage}");
            sse_response(sse(vec![
                ev_response_created(&response_id),
                item,
                ev_completed(&response_id),
            ]))
        })
        .mount(&server)
        .await;

    let mut created = test.thread_manager.subscribe_thread_created();
    let TurnInputSubmission::Started { turn_id, .. } = root
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Delegate waiting for the shared workspace, and report what the worker finds."
                .into(),
            text_elements: Vec::new(),
        }]))
        .await?
    else {
        anyhow::bail!("expected the parent's turn to start");
    };
    let child_id = timeout(Duration::from_secs(/*secs*/ 10), created.recv()).await??;
    let child = test.thread_manager.get_thread(child_id).await?;
    timeout(Duration::from_secs(/*secs*/ 10), async {
        tokio::join!(parent_requested.notified(), child_requested.notified());
    })
    .await?;
    assert_eq!(child.environment_selections().await, vec![pending.clone()]);

    let mut first = environment_config_for_selection(&test.config, &selection);
    if test.config.permissions.permission_profile() != &PermissionProfile::read_only() {
        first.permission_profile =
            PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    }
    if matches!(case, PendingSpawnCase::FirstResultOnly) {
        first.allow_login_shell = false;
    }
    let expected = TurnEnvironmentSelection {
        config: if failed {
            EnvironmentConfigState::Failed(FAILURE.to_string())
        } else {
            EnvironmentConfigState::Ready(first.clone())
        },
        ..selection
    };
    if matches!(case, PendingSpawnCase::ChildConfiguredFirst) {
        child.environment_ready(&pending, first.clone()).await?;
    }
    match case {
        PendingSpawnCase::TurnSettings => {
            let (reply, outcome) = tokio::sync::oneshot::channel();
            root.thread
                .submit(Op::TurnSettings {
                    turn_id,
                    update: TurnSettingsUpdate {
                        environments: Some(vec![expected.clone().into_request()]),
                        ..Default::default()
                    },
                    reply,
                })
                .await?;
            assert_eq!(outcome.await?, TurnSettingsUpdateOutcome::Applied);
        }
        PendingSpawnCase::Failure | PendingSpawnCase::ChildConfiguredFirst => {
            root.thread
                .environment_failed(&pending, FAILURE.to_string())
                .await?
        }
        PendingSpawnCase::V1Fork | PendingSpawnCase::FirstResultOnly => {
            root.thread
                .environment_ready(&pending, first.clone())
                .await?;
        }
    }
    tokio::join!(
        wait_for_event(&root.thread, |event| matches!(
            event,
            EventMsg::TurnComplete(_)
        )),
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))),
    );
    if matches!(case, PendingSpawnCase::ChildConfiguredFirst) {
        // The inherited callback runs separately; give it time to overwrite the child if unguarded.
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(child.environment_selections().await, vec![expected.clone()]);

    if matches!(case, PendingSpawnCase::FirstResultOnly) {
        let mut later = first;
        later.allow_login_shell = true;
        root.thread
            .environment_ready(&pending, later.clone())
            .await?;
        assert_eq!(
            root.thread.environment_selections().await,
            vec![TurnEnvironmentSelection {
                config: EnvironmentConfigState::Ready(later),
                ..pending.clone()
            }]
        );
        child
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: NEXT_TURN.into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        assert_eq!(child.environment_selections().await, vec![expected]);
    }

    let mut requests = server
        .received_requests()
        .await
        .context("recorded model requests")?
        .into_iter()
        .filter(|request| request.url.path() == "/v1/responses")
        .map(|request| request.body_json().context("model request body"))
        .collect::<Result<Vec<Value>>>()?;
    requests.sort_by_key(|body| request_stage(body, &root_id));
    let [_, _, starting, resumed, _, ..] = requests.as_slice() else {
        anyhow::bail!("missing model request")
    };
    let starting_tools = tool_names(starting);
    assert!(starting_tools.contains(&"wait_for_environment".to_string()));
    let output = call_output(resumed, WAIT_ENV).context("child wait result")?;
    if failed {
        assert!(output.contains(FAILURE));
    } else {
        assert_eq!(
            serde_json::from_str::<Value>(output)?,
            json!({"environment_id": pending.environment_id, "status": "ready"})
        );
        assert!(tool_names(resumed).contains(&"exec_command".to_string()));
    }
    Ok(requests)
}

#[test_case(PendingSpawnCase::V1Fork; "v1_fork")]
#[test_case(PendingSpawnCase::FirstResultOnly; "v2_fresh_ignores_later_owner_update_across_turns")]
#[test_case(PendingSpawnCase::Failure; "v2_owner_failure")]
#[test_case(PendingSpawnCase::ChildConfiguredFirst; "v2_child_configuration_wins_over_late_owner_failure")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_inherits_first_pending_owner_configuration(case: PendingSpawnCase) -> Result<()> {
    pending_subagent_scenario(case, |_| {}).await?;
    Ok(())
}
