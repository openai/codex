//! Exercises recovery through the parent tool call, reviewer, and executor boundary.

use anyhow::Result;
use codex_core::config::Constrained;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::responses::*;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_retry_executes_only_after_a_completed_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config
            .set_legacy_sandbox_policy(SandboxPolicy::new_workspace_write_policy())
            .expect("set sandbox policy");
    });
    let test = builder.build_with_auto_env(&server).await?;
    let responses = vec![
        sse(vec![
            ev_function_call(
                "write-marker",
                "exec_command",
                &json!({
                    "cmd": "echo executed >> guardian-retry.txt",
                    "sandbox_permissions": "require_escalated",
                    "justification": "Write the requested marker",
                })
                .to_string(),
            ),
            ev_completed("parent-call"),
        ]),
        sse_failed(
            "first-review",
            "rate_limit_exceeded",
            "temporary review error",
        ),
        sse_failed(
            "stream-retry",
            "rate_limit_exceeded",
            "temporary review error",
        ),
        sse(vec![
            ev_assistant_message(
                "approval",
                r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"requested write"}"#,
            ),
            ev_completed("review-approved"),
        ]),
        sse(vec![ev_completed("parent-done")]),
    ];
    let requests = mount_sse_sequence(&server, responses).await;
    test.codex
        .start_or_steer_turn(codex_core::TurnInputRequest::user_input(vec![
            codex_protocol::user_input::UserInput::Text {
                text: "Write the marker once".into(),
                text_elements: vec![],
            },
        ]))
        .await?;
    let mut reviews = Vec::new();
    let mut warnings = Vec::new();
    loop {
        match test.codex.next_event().await?.msg {
            EventMsg::GuardianAssessment(review) => reviews.push(review.status),
            EventMsg::GuardianWarning(warning) => warnings.push(warning.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(requests.requests().len(), 5);
    assert_eq!(
        reviews,
        vec![
            GuardianAssessmentStatus::InProgress,
            GuardianAssessmentStatus::Approved,
        ]
    );
    assert_eq!(
        warnings.len(),
        1,
        "internal retries should not emit terminal warnings"
    );
    let contents = test
        .fs()
        .read_file_text(
            &test.workspace_path_uri("guardian-retry.txt")?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(contents.lines().collect::<Vec<_>>(), vec!["executed"]);
    Ok(())
}
