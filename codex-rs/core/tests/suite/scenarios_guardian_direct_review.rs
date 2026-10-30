//! Direct exec review preserves live action rendering without executing the command.

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::guardian_review::GuardianReviewResult;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentRequests;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_exec_review_matches_live_action_without_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let builder = test_codex().with_model("gpt-5.5").with_config(|config| {
        super::configure_scenario_catalog(config);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        let rules = config.codex_home.join("rules");
        std::fs::create_dir_all(&rules).expect("create rules directory");
        std::fs::write(
            rules.join("approved.rules"),
            r#"prefix_rule(pattern=["git", "init", "guardian-skipped"], decision="allow")"#,
        )
        .expect("write approved command");
    });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();

    let arguments = r#"{"cmd":"echo should-not-run > guardian-direct.txt","justification":"Run the requested command.","sandbox_permissions":"require_escalated"}"#;
    let mut call = ev_function_call("exec-review", "exec_command", arguments);
    call["item"]["encrypted_function_args"] = json!([]);
    call["item"]["encrypted_content"] = json!("cached-output-tokens");
    let assessment = json!({
        "risk_level": "low",
        "user_authorization": "high",
        "outcome": "allow",
        "rationale": "The command is authorized."
    });
    let mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![call, ev_completed("parent-action")]),
            sse(vec![
                ev_assistant_message("live-denial", r#"{"outcome":"deny"}"#),
                ev_completed("live-review"),
            ]),
            sse(vec![
                ev_assistant_message("parent-done", "Denied."),
                ev_completed("parent-done"),
            ]),
            sse(vec![
                ev_assistant_message("direct-allow", &assessment.to_string()),
                ev_completed("direct-review"),
            ]),
        ],
    )
    .await;
    // Capture a live review without executing the command.
    let prompt = "Check the command, but do not run it without approval.";
    test.submit_text_turn(prompt).await?;

    // Replay the recorded items, including the provenance attached to the user message.
    test.codex.flush_rollout().await?;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    let items = history
        .items
        .into_iter()
        .filter_map(|entry| match entry {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::Message { role, content, .. }
                    if role == "user" && content == &[input_text(prompt)] =>
                {
                    Some(item.into_item())
                }
                ResponseItem::FunctionCall { .. } => Some(item.into_item()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    // Start replay in a different directory, then select the live call's cwd for the next turn.
    harness.create_dir_all("initial-cwd").await?;
    let mut initial = test.executor_environment().request();
    initial.cwd = test.workspace_path_uri("initial-cwd")?;
    let mut options = test.start_thread_options().await;
    let live_environments = options
        .environments
        .replace(vec![initial])
        .expect("live environments");
    let direct = test.thread_manager.start_thread(options).await?;
    direct.thread.inject_response_items(items).await?;
    direct
        .thread
        .update_thread_settings(ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentRequests::new(
                test.config.cwd.clone(),
                live_environments,
            )),
            ..Default::default()
        })
        .await?;
    let result = direct
        .thread
        .guardian_review(&CancellationToken::new())
        .await?;
    assert_eq!(
        result,
        GuardianReviewResult {
            call_id: "exec-review".to_owned(),
            assessment: Some(serde_json::from_value(assessment)?),
        }
    );
    assert!(!harness.path_exists("guardian-direct.txt").await?);

    // A tool output at the end invalidates the request, even with an older exec call.
    direct
        .thread
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "function_call_output", "call_id": "exec-review", "output": "recorded output"
        }))?])
        .await?;
    let error = direct
        .thread
        .guardian_review(&CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::InvalidRequest(_)
    ));

    // The explicit allow rule skips review on every platform.
    direct
        .thread
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "function_call", "call_id": "skip-review", "name": "exec_command",
            "arguments": r#"{"cmd":"git init guardian-skipped"}"#,
        }))?])
        .await?;
    assert_eq!(
        direct
            .thread
            .guardian_review(&CancellationToken::new())
            .await?,
        GuardianReviewResult {
            call_id: "skip-review".to_owned(),
            assessment: None,
        }
    );
    assert!(!harness.path_exists("guardian-skipped").await?);

    // The same allowlisted command cannot be assessed after a recorded permission grant.
    direct
        .thread
        .inject_response_items(serde_json::from_value(json!([
            {"type": "function_call", "name": "request_permissions", "call_id": "grant",
             "arguments": r#"{"permissions":{"network":{"enabled":true}}}"#},
            {"type": "function_call_output", "call_id": "grant",
             "output": r#"{"permissions":{"network":{"enabled":true}},"scope":"turn","strict_auto_review":true}"#},
            {"type": "function_call", "name": "exec_command", "call_id": "after-grant",
             "arguments": r#"{"cmd":"git init guardian-skipped"}"#}
        ]))?)
        .await?;
    let error = direct
        .thread
        .guardian_review(&CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::InvalidRequest(message)
            if message.contains("does not support replaying request_permissions")
    ));

    let requests = mock.requests();
    let [_, live, _, recorded] = requests.as_slice() else {
        panic!("expected parent, live Guardian, parent, recorded Guardian");
    };
    assert_eq!(
        model_input(recorded, direct.thread_id)?,
        model_input(live, history.thread_id)?
    );

    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Direct Guardian review of an injected exec call; an allow assessment does not execute it.",
        std::slice::from_ref(recorded),
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let environment_id = &test.executor_environment().selection().environment_id;
    snapshot = snapshot
        .replace(
            &format!("\"environment_id\": {environment_id:?}"),
            "\"environment_id\": \"<ENVIRONMENT>\"",
        )
        .replace(
            &format!("environment {environment_id:?}"),
            "environment \"<ENVIRONMENT>\"",
        );
    for (pattern, replacement) in [
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"echo should-not-run > guardian-direct.txt"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"echo should-not-run > guardian-direct.txt\"]",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!("guardian_direct_exec", snapshot);
    direct.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

fn model_input(request: &ResponsesRequest, parent: ThreadId) -> Result<String> {
    let mut input = request.input();
    for item in &mut input {
        item.as_object_mut().expect("response item").remove("id");
        if let Some(metadata) = item
            .get_mut("internal_chat_message_metadata_passthrough")
            .and_then(serde_json::Value::as_object_mut)
        {
            metadata.remove("turn_id");
            metadata.remove("create_time");
        }
    }
    Ok(serde_json::to_string_pretty(&input)?.replace(
        &format!("Reviewed Codex session id: {parent}"),
        "Reviewed Codex session id: <PARENT>",
    ))
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn direct_exec_review_applies_hook_rewrite_before_policy_check() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let builder = test_codex()
        .with_model("gpt-5.5")
        .with_pre_build_hook(|home| {
            super::super::hooks::write_updating_pre_tool_use_hook(
                home,
                "^Bash$",
                &json!({"command": "git init guardian-rewritten"}),
            )
            .expect("write rewriting hook");
        })
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            core_test_support::hooks::trust_discovered_hooks(config);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            let rules = config.codex_home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(
                rules.join("direct-review.rules"),
                r#"prefix_rule(pattern=["git", "init", "guardian-original"], decision="allow")
prefix_rule(pattern=["git", "init", "guardian-rewritten"], decision="prompt")
"#,
            )
            .expect("write command policies");
        });
    // These hook fixtures use the local Python interpreter and filesystem.
    let harness = TestCodexHarness::with_builder(builder).await?;
    let test = harness.test();

    let assessment = json!({
        "risk_level": "low",
        "user_authorization": "high",
        "outcome": "allow",
        "rationale": "The rewritten command is authorized."
    });
    let mock = responses::mount_sse_once(
        harness.server(),
        sse(vec![
            ev_assistant_message("review", &assessment.to_string()),
            ev_completed("review"),
        ]),
    )
    .await;

    test.codex
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "function_call", "call_id": "hook-review", "name": "exec_command",
            "arguments": r#"{"cmd":"git init guardian-original"}"#,
        }))?])
        .await?;
    assert_eq!(
        test.codex
            .guardian_review(&CancellationToken::new())
            .await?,
        GuardianReviewResult {
            call_id: "hook-review".to_owned(),
            assessment: Some(serde_json::from_value(assessment)?),
        }
    );

    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Direct Guardian review applies the PreToolUse rewrite before checking approval policy.",
        &[mock.single_request()],
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let environment_id = &test.executor_environment().selection().environment_id;
    snapshot = snapshot
        .replace(
            &format!("\"environment_id\": {environment_id:?}"),
            "\"environment_id\": \"<ENVIRONMENT>\"",
        )
        .replace(
            &format!("environment {environment_id:?}"),
            "environment \"<ENVIRONMENT>\"",
        );
    for (pattern, replacement) in [
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"(git init guardian-(?:original|rewritten))"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"$1\"]",
        ),
        (
            r"(?m)^(\s*\[\d+\] )`.*` requires approval by policy$",
            "$1<COMMAND> requires approval by policy",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!("guardian_direct_pre_tool_use_rewrite", snapshot);

    assert!(!harness.path_exists("guardian-original").await?);
    assert!(!harness.path_exists("guardian-rewritten").await?);

    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn direct_exec_review_honors_hook_denial_before_policy_skip() -> Result<()> {
    use codex_protocol::protocol::EventMsg;
    use core_test_support::wait_for_event;

    skip_if_no_network!(Ok(()));

    let reason = "blocked by direct-review hook";
    let builder = test_codex()
        .with_model("gpt-5.5")
        .with_pre_build_hook(move |home| {
            super::super::hooks::write_pre_tool_use_hook(
                home,
                Some("^Bash$"),
                "json_deny_with_context",
                reason,
            )
            .expect("write blocking hook");
        })
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            core_test_support::hooks::trust_discovered_hooks(config);
            let rules = config.codex_home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(
                rules.join("direct-review.rules"),
                "prefix_rule(pattern=[\"git\", \"init\", \"guardian-original\"], decision=\"allow\")\n",
            )
            .expect("write allow rule");
        });
    // These hook fixtures use the local Python interpreter and filesystem.
    let harness = TestCodexHarness::with_builder(builder).await?;
    let test = harness.test();

    let mock =
        responses::mount_sse_once(harness.server(), sse(vec![ev_completed("unexpected")])).await;

    test.codex
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "function_call", "call_id": "hook-review", "name": "exec_command",
            "arguments": r#"{"cmd":"git init guardian-original"}"#,
        }))?])
        .await?;
    let error = test
        .codex
        .guardian_review(&CancellationToken::new())
        .await
        .expect_err("hook must reject the call before policy can skip review");
    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::InvalidRequest(_)
    ));
    assert!(mock.requests().is_empty());

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::HookStarted(_))
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::HookCompleted(_))
    })
    .await;

    test.codex.flush_rollout().await?;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    assert!(history.items.iter().any(|entry| {
        matches!(entry, RolloutItem::ResponseItem(item)
            if matches!(&item.item, ResponseItem::Message { role, content, .. }
                if role == "developer" && content == &[input_text(reason)]))
    }));

    assert!(!harness.path_exists("guardian-original").await?);

    test.codex.shutdown_and_wait().await?;
    Ok(())
}
