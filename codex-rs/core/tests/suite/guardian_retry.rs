//! Exercises recovery through the parent tool call, reviewer, and executor boundary.

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::context::UserGoalUpdate;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::responses::*;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_stale_approval_replaces_review_and_preserves_earlier_context() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let verdict = |id, rationale| {
        sse(vec![
            ev_assistant_message(
                id,
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": rationale,
                })
                .to_string(),
            ),
            ev_completed(id),
        ])
    };
    let command = |id, cmd| {
        sse(vec![
            ev_function_call(
                id,
                "exec_command",
                &json!({
                    "cmd": cmd,
                    "sandbox_permissions": "require_escalated",
                    "justification": "Write the requested marker once",
                })
                .to_string(),
            ),
            ev_completed(id),
        ])
    };
    let (release_review, review_gate) = tokio::sync::oneshot::channel();
    let (release_retry, retry_gate) = tokio::sync::oneshot::channel();
    let (model, _) = start_streaming_sse_server(
        vec![
            (
                None,
                command("earlier-command", "echo earlier >> earlier.txt"),
            ),
            (None, verdict("earlier-review", "Earlier valid approval")),
            (
                None,
                command("pending-command", "echo pending >> pending.txt"),
            ),
            (
                Some(review_gate),
                verdict("stale-review", "First invalidated approval"),
            ),
            (
                Some(retry_gate),
                verdict("stale-retry", "Second invalidated approval"),
            ),
            (
                None,
                verdict("replacement-review", "Decision based on updated guidance"),
            ),
            (None, sse(vec![ev_completed("parent-done")])),
        ]
        .into_iter()
        .map(|(gate, body)| vec![StreamingSseChunk { gate, body }])
        .collect(),
    )
    .await;
    let server = start_mock_server().await;
    let base_url = format!("{}/v1", model.uri());
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .set_legacy_sandbox_policy(SandboxPolicy::new_workspace_write_policy())
                .expect("sandbox policy");
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(codex_core::TurnInputRequest::user_input(vec![
            codex_protocol::user_input::UserInput::Text {
                text: "Write the earlier marker, then the pending marker, once each.".to_owned(),
                text_elements: vec![],
            },
        ]))
        .await?;

    // Update authorization through the public goal API while each allow is held in flight.
    // The second invalidation must still preserve the checkpoint from the earlier action.
    let guidance = [
        "The pending marker should contain one line.",
        "Continue with the pending marker exactly once.",
    ];
    for (count, release, objective) in [
        (4, release_review, guidance[0]),
        (5, release_retry, guidance[1]),
    ] {
        tokio::time::timeout(Duration::from_secs(20), model.wait_for_request_count(count))
            .await
            .context("wait for pending review")?;
        test.codex
            .record_user_goal_update(UserGoalUpdate::Set {
                objective: Some(objective.to_owned()),
                status: None,
            })
            .await?;
        release.send(()).expect("release stale approval");
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = model
        .requests()
        .await
        .into_iter()
        .map(|body| serde_json::from_slice::<Value>(&body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let reviews = requests
        .iter()
        .filter(|request| request["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), 4);
    let replacement = reviews[3]["input"].to_string();
    for expected in ["Earlier valid approval", guidance[0], guidance[1]] {
        assert!(
            replacement.contains(expected),
            "missing context: {expected}"
        );
    }
    for stale in ["First invalidated approval", "Second invalidated approval"] {
        assert!(
            !replacement.contains(stale),
            "stale review survived: {stale}"
        );
    }
    assert_eq!(
        replacement.matches(">>> APPROVAL REQUEST START").count(),
        2,
        "retain the earlier action and replace the pending action's stale review"
    );
    let contents = test
        .fs()
        .read_file_text(
            &test.workspace_path_uri("pending.txt")?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(contents.lines().collect::<Vec<_>>(), vec!["pending"]);
    test.codex.shutdown_and_wait().await?;
    model.shutdown().await;
    Ok(())
}
