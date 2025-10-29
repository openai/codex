use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::SandboxPolicy;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;

/// Delegate should surface ExecApprovalRequest from sub-agent and proceed
/// after parent submits an approval decision.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_forwards_exec_approval_and_proceeds_on_approval() {
    skip_if_no_network!();

    // Sub-agent turn 1: emit a shell function_call requiring approval, then complete.
    let call_id = "call-exec-1";
    let args = serde_json::json!({
        "command": ["bash", "-lc", "rm -rf delegated"],
        "timeout_ms": 1000,
        "with_escalated_permissions": true,
    })
    .to_string();
    let sse1 = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell", &args),
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
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::ReadOnly;
    });
    let test = builder.build(&server).await.expect("build test codex");

    // Kick off review (sub-agent starts internally).
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                prompt: "Please review".to_string(),
                user_facing_hint: "review".to_string(),
            },
        })
        .await
        .expect("submit review");

    // Lifecycle: Entered -> ExecApprovalRequest -> Exited(Some) -> TaskComplete.
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::EnteredReviewMode(_))
    })
    .await;

    // Expect parent-side approval request (forwarded by delegate).
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExecApprovalRequest(_))
    })
    .await;

    // Approve via parent; id "0" is the active sub_id in tests.
    test.codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::Approved,
        })
        .await
        .expect("submit exec approval");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExitedReviewMode(_))
    })
    .await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

/// Delegate should surface ApplyPatchApprovalRequest and honor parent decision
/// so the sub-agent can proceed to completion.
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

    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        // Use a restricted sandbox so patch approval is required
        config.sandbox_policy = SandboxPolicy::ReadOnly;
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                prompt: "Please review".to_string(),
                user_facing_hint: "review".to_string(),
            },
        })
        .await
        .expect("submit review");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::EnteredReviewMode(_))
    })
    .await;
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ApplyPatchApprovalRequest(_))
    })
    .await;

    // Deny via parent so delegate can continue; id "0" is the active sub_id in tests.
    test.codex
        .submit(Op::PatchApproval {
            id: "0".into(),
            decision: ReviewDecision::Denied,
        })
        .await
        .expect("submit patch approval");

    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ExitedReviewMode(_))
    })
    .await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}
