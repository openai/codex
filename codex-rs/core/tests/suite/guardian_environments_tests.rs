//! Reviews bind each action to its captured target policy while reusing the reviewer context.

use super::*;
use codex_history::RolloutItem;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use test_case::test_case;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test_case("exec_command")]
#[test_case("apply_patch")]
#[test_case("request_permissions")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reviews_target_environment_and_reuses_prefix(tool: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let executor_url = format!("ws://{}", listener.local_addr()?);
    let (attach, connection) = tokio::sync::oneshot::channel();
    let (shutdown, stop) = tokio::sync::oneshot::channel();
    let executor = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(
        serve_environment_with_agents_md(listener, "", connection, stop),
    ));
    attach.send(()).expect("attach secondary executor");

    let server = start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
            )
            .expect("configure asynchronous review");
        })
        .with_model_info_override("guardian-environments-parent", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-5.5".to_owned());
        })
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
            config
                .features
                .disable(Feature::DeferredExecutor)
                .expect("disable deferred executor");
        })
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.environment_manager();
    let secondary_id = "guardian-secondary";
    manager.upsert_environment(
        secondary_id.to_string(),
        executor_url,
        /*connect_timeout*/ None,
    )?;
    manager
        .get_environment(secondary_id)
        .context("secondary executor")?
        .wait_until_ready()
        .await?;

    let mut primary = test.executor_environment().selection().clone();
    primary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &primary));
    let mut secondary = primary.clone();
    secondary.environment_id = secondary_id.to_string();
    let denied = secondary.cwd.join("secondary-private")?;
    let mut secondary_config = environment_config_for_selection(&test.config, &secondary);
    let mut file_system = secondary_config
        .permission_profile
        .permission_profile()
        .file_system_sandbox_policy();
    file_system.entries.push(FileSystemSandboxEntry::new(
        denied.clone().into(),
        FileSystemAccessMode::Deny,
    ));
    secondary_config.permission_profile = PermissionProfileSnapshot::legacy(
        PermissionProfile::from_runtime_permissions(&file_system, NetworkSandboxPolicy::Restricted),
    );
    secondary.config = EnvironmentConfigState::Ready(secondary_config);

    let targets = [secondary_id, primary.environment_id.as_str()];
    let mut events = Vec::new();
    for (index, environment_id) in targets.iter().enumerate() {
        let call_id = format!("action-{index}");
        let action = match tool {
            "exec_command" => ev_function_call(
                &call_id,
                tool,
                &json!({
                    "environment_id": environment_id,
                    "cmd": "echo review-only",
                    "sandbox_permissions": "require_escalated",
                    "justification": "Review the target environment.",
                })
                .to_string(),
            ),
            "apply_patch" => ev_apply_patch_custom_tool_call(
                &call_id,
                &format!(
                    "*** Begin Patch\n*** Environment ID: {environment_id}\n*** Add File: guardian-marker.txt\n+review-only\n*** End Patch\n"
                ),
            ),
            "request_permissions" => ev_function_call(
                &call_id,
                tool,
                &json!({
                    "environment_id": environment_id,
                    "permissions": {"network": {"enabled": true}},
                    "reason": "Review the target environment.",
                })
                .to_string(),
            ),
            _ => unreachable!(),
        };
        events.push(sse(vec![action, ev_completed(&call_id)]));
        let review_id = format!("review-{index}");
        events.push(sse(vec![
            ev_assistant_message(&review_id, r#"{"outcome":"deny"}"#),
            ev_completed(&review_id),
        ]));
    }
    events.push(sse(vec![
        ev_assistant_message("done", "done"),
        ev_completed("done"),
    ]));
    // Hold the second action until the first score is published. Its own classifier
    // response stays pending, so only the score for the other computer is available.
    let (advance, next_action) = tokio::sync::oneshot::channel();
    let mut next_action = Some(next_action);
    let (parent, _) = start_streaming_sse_server(
        events
            .iter()
            .step_by(2)
            .enumerate()
            .map(|(index, body)| {
                vec![StreamingSseChunk {
                    gate: if index == 1 { next_action.take() } else { None },
                    body: body.clone(),
                }]
            })
            .collect(),
    )
    .await;
    let (publish, score_ready) = tokio::sync::oneshot::channel();
    let (_hold_score, pending_score) = tokio::sync::oneshot::channel();
    let (classifier, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(score_ready),
            body: sse(vec![
                ev_assistant_message("score-0", "low"),
                ev_completed("score-0"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(pending_score),
            body: sse(vec![
                ev_assistant_message("score-1", "low"),
                ev_completed("score-1"),
            ]),
        }],
    ])
    .await;
    for (model, destination) in [
        ("guardian-environments-parent", parent.uri()),
        ("gpt-5.6-luna", classifier.uri()),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_partial_json(json!({"model": model})))
            .respond_with(
                ResponseTemplate::new(/*s*/ 307)
                    .insert_header("location", format!("{destination}/v1/responses")),
            )
            .with_priority(/*p*/ 1)
            .mount(&server)
            .await;
    }
    let responses =
        mount_sse_sequence(&server, events.into_iter().skip(1).step_by(2).collect()).await;
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Review each action on its requested environment.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![primary.clone(), secondary],
                )),
                ..Default::default()
            }),
        )
        .await?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        parent.wait_for_request_count(/*count*/ 2).await;
        publish.send(()).expect("publish the first computer score");
        loop {
            let history = test.codex.load_history(/*include_archived*/ false).await?;
            if history.items.into_iter().any(|item| {
                matches!(item, RolloutItem::SecurityRiskScore(score) if score.call_id.as_deref() == Some("action-0"))
            }) {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    advance
        .send(())
        .expect("start action on the primary computer");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    let reviews = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), targets.len());
    for (review, environment_id) in reviews.iter().zip(targets) {
        let groups = review.message_input_text_groups("user");
        let latest = groups.last().context("current review input")?;
        assert_eq!(
            latest
                .concat()
                .matches("user: Review each action on its requested environment.")
                .count(),
            usize::from(environment_id == secondary_id),
            "the transcript delivers the instruction once, only in the first review"
        );
        let start = latest
            .iter()
            .position(|text| text == "\n>>> PARENT TURN PERMISSION CONTEXT START\n")
            .context("permission context")?;
        let permissions = &latest[start + 1];
        assert!(permissions.contains(&format!("environment {environment_id:?}")));
        if environment_id == secondary_id {
            assert!(permissions.contains(&denied.inferred_native_path_string()));
        } else {
            assert!(permissions.contains("no explicit denied-read"));
            assert!(!permissions.contains(&denied.inferred_native_path_string()));
        }
        let action = latest
            .iter()
            .find_map(|text| serde_json::from_str::<Value>(text).ok())
            .context("planned action JSON")?;
        assert_eq!(
            (&action["tool"], &action["environment_id"]),
            (&json!(tool), &json!(environment_id))
        );
    }
    assert_eq!(
        reviews[0].body_json()["client_metadata"]["thread_id"],
        reviews[1].body_json()["client_metadata"]["thread_id"]
    );
    assert!(
        reviews[1].input().starts_with(&reviews[0].input()),
        "earlier Guardian input must remain unchanged"
    );
    let classifier_requests = classifier.requests().await;
    let request: Value = serde_json::from_slice(&classifier_requests[0])?;
    let input = request["input"].to_string();
    assert!(input.contains(secondary_id));
    assert!(input.contains("secondary-private"));
    test.codex.shutdown_and_wait().await?;
    parent.shutdown().await;
    classifier.shutdown().await;
    shutdown.send(()).expect("stop secondary executor");
    executor.await?;
    Ok(())
}
