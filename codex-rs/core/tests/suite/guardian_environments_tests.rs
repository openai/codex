//! Reviews bind each action to its captured target policy while reusing the reviewer context.

use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

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
            config
                .features
                .disable(Feature::GuardianV2)
                .expect("disable Guardian prewarm");
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
    let responses = mount_sse_sequence(&server, events).await;
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
    shutdown.send(()).expect("stop secondary executor");
    executor.await?;
    Ok(())
}
