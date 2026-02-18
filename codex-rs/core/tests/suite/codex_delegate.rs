use codex_core::config::Constrained;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::ReviewTarget;
use codex_core::protocol::SandboxPolicy;
use codex_core::sandboxing::SandboxPermissions;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

/// Delegate should surface ExecApprovalRequest from sub-agent and proceed
/// after parent submits an approval decision.
#[ignore = "TODO once we have a delegate that can ask for approvals"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_forwards_exec_approval_and_proceeds_on_approval() {
    skip_if_no_network!();

    // Sub-agent turn 1: emit a shell_command function_call requiring approval, then complete.
    let call_id = "call-exec-1";
    let args = serde_json::json!({
        "command": "rm -rf delegated",
        "timeout_ms": 1000,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
    })
    .to_string();
    let sse1 = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-1"),
    ]);

    // Sub-agent turn 2: return structured review output and complete.
    let review_json = serde_json::json!({
        "findings": [],
        "overall_correctness": "ok",
        "overall_explanation": "delegate approved exec",
        "overall_confidence_score": 0.5
    })
    .to_string();
    let sse2 = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", &review_json),
        ev_completed("resp-2"),
    ]);

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![sse1, sse2]).await;

    // Build a conversation configured to require approvals so the delegate
    // routes ExecApprovalRequest via the parent.
    let mut builder = test_codex().with_model("gpt-5.1").with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.permissions.sandbox_policy =
            Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    });
    let test = builder.build(&server).await.expect("build test codex");

    // Kick off review (sub-agent starts internally).
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    // Lifecycle: Entered -> ExecApprovalRequest -> Exited(Some) -> TurnComplete.
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::EnteredReviewMode(_))
    })
    .await;

    // Expect parent-side approval request (forwarded by delegate).
    let approval_event = wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExecApprovalRequest(_))
    })
    .await;
    let EventMsg::ExecApprovalRequest(approval) = approval_event else {
        panic!("expected ExecApprovalRequest event");
    };

    // Approve via parent using the emitted approval call ID.
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await
        .expect("submit exec approval");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExitedReviewMode(_))
    })
    .await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

/// Delegate should surface ApplyPatchApprovalRequest and honor parent decision
/// so the sub-agent can proceed to completion.
#[ignore = "TODO once we have a delegate that can ask for approvals"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_forwards_patch_approval_and_proceeds_on_decision() {
    skip_if_no_network!();

    let call_id = "call-patch-1";
    let patch = "*** Begin Patch\n*** Add File: delegated.txt\n+hello\n*** End Patch\n";
    let sse1 = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_function_call(call_id, patch),
        ev_completed("resp-1"),
    ]);
    let review_json = serde_json::json!({
        "findings": [],
        "overall_correctness": "ok",
        "overall_explanation": "delegate patch handled",
        "overall_confidence_score": 0.5
    })
    .to_string();
    let sse2 = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", &review_json),
        ev_completed("resp-2"),
    ]);

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![sse1, sse2]).await;

    let mut builder = test_codex().with_model("gpt-5.1").with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        // Use a restricted sandbox so patch approval is required
        config.permissions.sandbox_policy =
            Constrained::allow_any(SandboxPolicy::new_read_only_policy());
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::EnteredReviewMode(_))
    })
    .await;
    let approval_event = wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ApplyPatchApprovalRequest(_))
    })
    .await;
    let EventMsg::ApplyPatchApprovalRequest(approval) = approval_event else {
        panic!("expected ApplyPatchApprovalRequest event");
    };

    // Deny via parent so delegate can continue, using the emitted approval call ID.
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::Denied,
        })
        .await
        .expect("submit patch approval");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExitedReviewMode(_))
    })
    .await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_ignores_legacy_deltas() {
    skip_if_no_network!();

    // Single response with reasoning summary deltas.
    let sse_stream = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item_added("reason-1", &["initial"]),
        ev_reasoning_summary_text_delta("think-1"),
        ev_completed("resp-1"),
    ]);

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![sse_stream]).await;

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("build test codex");

    // Kick off review (delegated).
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let mut reasoning_delta_count = 0;
    let mut legacy_reasoning_delta_count = 0;

    loop {
        let ev = wait_for_event(&test.codex, |_| true).await;
        match ev {
            EventMsg::ReasoningContentDelta(_) => reasoning_delta_count += 1,
            EventMsg::AgentReasoningDelta(_) => legacy_reasoning_delta_count += 1,
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(reasoning_delta_count, 1, "expected one new reasoning delta");
    assert_eq!(
        legacy_reasoning_delta_count, 1,
        "expected one legacy reasoning delta"
    );
}
