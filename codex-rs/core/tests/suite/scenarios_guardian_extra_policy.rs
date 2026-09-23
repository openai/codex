//! Managed Guardian policies override config defaults and reach the reviewer separately.

use anyhow::Context;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::Constrained;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extra_policy_reaches_guardian_review() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(
                    "escalated-command",
                    "exec_command",
                    r#"{"cmd":"exit 0","justification":"Run a check outside the sandbox.","sandbox_permissions":"require_escalated"}"#,
                ),
                responses::ev_completed("parent-action"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message(
                    "guardian-denial",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": "The unsandboxed command is not authorized.",
                    })
                    .to_string(),
                ),
                responses::ev_completed("guardian-review"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("parent-result", "The check was not authorized."),
                responses::ev_completed("parent-done"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model("gpt-5.5")
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(
                r#"
[auto_review]
policy = "Use the default tenant policy."
extra_policy = "Use the default additional policy."
"#,
            )
            .add_enterprise_requirement(
                r#"
guardian_policy_config = "Keep workspace data private."
guardian_extra_policy = "Draft reminders without sending them."
"#,
            )
            .into_loader(),
        )
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.guardian_policy_template = Some(
                ResolvedModelMessages::bundled()
                    .auto_review()
                    .policy_template
                    .to_owned(),
            );
            config.workspace_roots = vec![config.cwd.clone()];
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("read-only fixture permissions");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Check the workspace.").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let reviewer = requests
        .iter()
        .find(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .context("Guardian reviewer request")?;
    let reviewer_text = reviewer
        .message_input_texts("developer")
        .join("\n")
        .replace("\r\n", "\n");
    assert!(
        reviewer_text.contains(
            "# Security Policy\nKeep workspace data private.\n\nDraft reminders without sending them.\n\n# Investigation Guidelines"
        ),
        "{reviewer_text}"
    );
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Guardian reviews an escalated command with separate tenant and additional policies, denies it, and the parent continues.",
        &requests,
        &ContextSnapshotOptions::default().include_request_settings(),
    );
    let environment_id = &test.executor_environment().selection().environment_id;
    snapshot = snapshot
        .replace(
            &format!("\"environment_id\": {environment_id:?}"),
            "\"environment_id\": \"<ENVIRONMENT>\"",
        )
        .replace(
            &format!("For this action on environment {environment_id:?},"),
            "For this action on environment \"<ENVIRONMENT>\",",
        );
    for (pattern, replacement) in [
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"exit 0"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"exit 0\"]",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!("extra_policy_reaches_guardian_review", snapshot);
    Ok(())
}
