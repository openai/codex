use super::*;

use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::Sandboxable;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_sandboxing::SandboxablePreference;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
struct ApprovalCall {
    used_guardian: bool,
    retry_reason: Option<String>,
}

#[derive(Default)]
struct TimeoutThenManualRuntime {
    calls: Vec<ApprovalCall>,
}

impl Approvable<()> for TimeoutThenManualRuntime {
    type ApprovalKey = ();

    fn approval_keys(&self, _req: &()) -> Vec<Self::ApprovalKey> {
        Vec::new()
    }

    fn start_approval_async<'a>(
        &'a mut self,
        _req: &'a (),
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        Box::pin(async move {
            let used_guardian = ctx.guardian_review_id.is_some();
            self.calls.push(ApprovalCall {
                used_guardian,
                retry_reason: ctx.retry_reason.clone(),
            });
            if used_guardian {
                ReviewDecision::TimedOut
            } else {
                ReviewDecision::Approved
            }
        })
    }
}

impl Sandboxable for TimeoutThenManualRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
}

impl ToolRuntime<(), ()> for TimeoutThenManualRuntime {
    async fn run(
        &mut self,
        _req: &(),
        _attempt: &SandboxAttempt<'_>,
        _ctx: &ToolCtx,
    ) -> Result<(), ToolError> {
        Ok(())
    }
}

#[tokio::test]
async fn guardian_timeout_falls_back_to_manual_approval() {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    turn.session_source = SessionSource::Cli;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tool_ctx = ToolCtx {
        session: Arc::clone(&session),
        turn: Arc::clone(&turn),
        call_id: "call-1".to_string(),
        tool_name: codex_tools::ToolName::plain("exec_command"),
    };
    let approval_ctx = ApprovalCtx {
        session: &session,
        turn: &turn,
        call_id: "call-1",
        guardian_review_id: Some("guardian-review-1".to_string()),
        retry_reason: None,
        network_approval_context: None,
    };
    let mut runtime = TimeoutThenManualRuntime::default();

    let approval_decision = ToolOrchestrator::request_approval_with_manual_fallback(
        &mut runtime,
        &(),
        "permission-request-1",
        approval_ctx,
        &tool_ctx,
        super::ApprovalRequestOptions {
            evaluate_permission_request_hooks: false,
            manual_fallback_for_guardian_timeout: true,
        },
        &turn.session_telemetry,
    )
    .await
    .expect("approval request should succeed");

    assert_eq!(
        approval_decision.decision,
        ReviewDecision::Approved,
        "manual fallback approval should become the terminal decision"
    );
    assert_eq!(approval_decision.guardian_review_id, None);
    assert_eq!(
        runtime.calls,
        vec![
            ApprovalCall {
                used_guardian: true,
                retry_reason: None,
            },
            ApprovalCall {
                used_guardian: false,
                retry_reason: Some(guardian_timeout_message()),
            },
        ]
    );
}

#[tokio::test]
async fn guardian_timeout_stays_terminal_when_manual_fallback_is_disabled() {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    turn.session_source = SessionSource::Cli;
    turn.manual_approval_fallback_enabled = false;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tool_ctx = ToolCtx {
        session: Arc::clone(&session),
        turn: Arc::clone(&turn),
        call_id: "call-1".to_string(),
        tool_name: codex_tools::ToolName::plain("exec_command"),
    };
    let approval_ctx = ApprovalCtx {
        session: &session,
        turn: &turn,
        call_id: "call-1",
        guardian_review_id: Some("guardian-review-1".to_string()),
        retry_reason: None,
        network_approval_context: None,
    };
    let mut runtime = TimeoutThenManualRuntime::default();

    let approval_decision = ToolOrchestrator::request_approval_with_manual_fallback(
        &mut runtime,
        &(),
        "permission-request-1",
        approval_ctx,
        &tool_ctx,
        super::ApprovalRequestOptions {
            evaluate_permission_request_hooks: false,
            manual_fallback_for_guardian_timeout: false,
        },
        &turn.session_telemetry,
    )
    .await
    .expect("approval request should succeed");

    assert_eq!(approval_decision.decision, ReviewDecision::TimedOut);
    assert_eq!(
        approval_decision.guardian_review_id,
        Some("guardian-review-1".to_string())
    );
    assert_eq!(
        runtime.calls,
        vec![ApprovalCall {
            used_guardian: true,
            retry_reason: None,
        }]
    );
}
