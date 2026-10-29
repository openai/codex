#![cfg(test)]

//! Managed checkpoints require a fresh human decision even with automatic review, hooks, and saved rules.

#[cfg(not(target_os = "windows"))]
use super::super::hooks::install_allow_permission_request_hook;
#[cfg(not(target_os = "windows"))]
use super::super::hooks::read_permission_request_hook_inputs;
use super::*;
use anyhow::Context;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
#[cfg(not(target_os = "windows"))]
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_once;
#[cfg(not(target_os = "windows"))]
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_checkpoint_requires_fresh_human_decisions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command approval requires host-native paths");
    let server = start_mock_server().await;
    let builder = test_codex();
    #[cfg(not(target_os = "windows"))]
    let builder = builder.with_pre_build_hook(|home| {
        install_allow_permission_request_hook(home).expect("allow permission request hook");
    });
    let test = builder
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"
allowed_approvals_reviewers = ["auto_review", "user"]
[[rules.prefix_rules]]
pattern = [{ token = "echo" }]
decision = "prompt-user"
justification = "A person must approve this command."
"#,
            ),
        )
        .with_config(|config| {
            configure_scenario_catalog(config);
            #[cfg(not(target_os = "windows"))]
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("unified exec");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("permissions");
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            fs::create_dir_all(config.codex_home.join("rules")).expect("rules directory");
            fs::write(
                config.codex_home.join("rules/allow.rules"),
                r#"prefix_rule(pattern=["echo"], decision="allow")"#,
            )
            .expect("saved allowance");
        })
        .build_with_auto_env(&server)
        .await?;
    let mut requests = Vec::new();
    let mut approval_ids = HashSet::<String>::new();
    for (index, decision) in [
        ReviewDecision::ApprovedForSession,
        ReviewDecision::Approved,
        ReviewDecision::Approved,
    ]
    .into_iter()
    .enumerate()
    {
        let call_id = format!("command-{index}");
        let command = mount_sse_once(
            &server,
            sse(vec![
                ev_response_created(&call_id),
                ev_function_call(
                    &call_id,
                    "exec_command",
                    &json!({"cmd": "echo checkpoint-ok", "login": false, "yield_time_ms": 10000})
                        .to_string(),
                ),
                ev_completed(&call_id),
            ]),
        )
        .await;
        let continuation = mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message("done", "Done."),
                ev_completed("done"),
            ]),
        )
        .await;
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![text("Run the command.")]))
            .await?;
        let approval = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::ExecApprovalRequest(approval) => Some(approval.clone()),
            _ => None,
        })
        .await;
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            read_permission_request_hook_inputs(test.codex_home_path())?.len(),
            index + 1,
        );
        let id = approval
            .approval_id
            .context("human approval has a unique id")?;
        assert!(approval_ids.insert(id.clone()));
        test.codex
            .submit(Op::ExecApproval {
                id,
                turn_id: None,
                decision,
            })
            .await?;
        requests.push(command.single_request());
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let result = continuation.single_request();
        let output = result
            .function_call_output_text(&call_id)
            .context("command result")?;
        if index == 0 {
            assert!(output.contains("one-time human approval"), "{output}");
        } else {
            assert!(output.contains("checkpoint-ok"), "{output}");
        }
        requests.push(result);
    }
    // Direct Guardian review cannot substitute for the managed human checkpoint either.
    test.codex
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "function_call", "name": "exec_command", "call_id": "direct-review",
            "arguments": r#"{"cmd":"echo checkpoint-ok","login":false}"#,
        }))?])
        .await?;
    let error = test
        .codex
        .guardian_review(&CancellationToken::new())
        .await
        .expect_err("managed checkpoint requires a human");
    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::InvalidRequest(message)
            if message == "Managed command policy requires human approval."
    ));
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Managed command checkpoints override saved allow rules and automatic review. Reusable decisions are rejected, and each successful execution requires a fresh human decision.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    for (pattern, replacement) in [
        (r"Chunk ID: [a-z0-9]+", "Chunk ID: <CHUNK_ID>"),
        (
            r"Wall time: [0-9]+(?:\.[0-9]+)? seconds",
            "Wall time: <DURATION> seconds",
        ),
        (
            r"Original token count: [0-9]+",
            "Original token count: <TOKEN_COUNT>",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!(snapshot);
    Ok(())
}
