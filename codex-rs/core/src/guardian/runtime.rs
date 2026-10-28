//! Prepares and validates host approval actions for the extension-owned reviewer.

use codex_protocol::protocol::ReviewDecision;
use codex_utils_path_uri::PathConvention;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::ApprovalRequestReasons;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::GuardianReviewOptions;
use crate::session::session::Session;
use crate::tools::sandboxing::ApprovalAction;
pub(crate) use codex_guardian_reviewer::ReviewAction;

pub(crate) fn from_approval_action(
    action: ApprovalAction,
    context: &GuardianReviewContext,
) -> ReviewAction {
    let exec_command_cwd_convention = match &action {
        ApprovalAction::ExecCommand {
            environment_id,
            cwd,
            ..
        } => Some(
            match context
                .environments()
                .turn_environments()
                .find(|environment| environment.selection.environment_id == *environment_id)
                .and_then(|environment| environment.executor_platform_os.as_deref())
            {
                Some("windows") => PathConvention::Windows,
                Some(_) => PathConvention::Posix,
                // Legacy executors did not report their OS. Preserve the
                // previous spelling-based behavior for those executors.
                None => cwd
                    .infer_path_convention()
                    .unwrap_or_else(PathConvention::native),
            },
        ),
        _ => None,
    };
    let category = action.guardian_scope();
    let original = serde_json::to_value(&action).map_err(|error| error.to_string());
    match action.into_guardian_request(exec_command_cwd_convention) {
        Ok(request) => ReviewAction {
            category,
            ..ReviewAction::from(request)
        },
        Err(error) => ReviewAction {
            action: original,
            category,
            tool_call_id: None,
            request: Err(error.to_string()),
        },
    }
}

/// Preserve the checks previously made before entering Guardian from tool approvals.
pub(super) fn validate<'a>(
    action: &'a ReviewAction,
    context: &GuardianReviewContext,
) -> Result<&'a GuardianApprovalRequest, ReviewDecision> {
    let request = action.request.as_ref().map_err(|error| {
        tracing::error!(%error, "failed to build automatic approval action");
        ReviewDecision::denied("automatic approval review could not prepare the action")
    })?;
    if let Some(environment_id) = request.target_environment_id()
        && !context
            .environments()
            .turn_environments()
            .any(|environment| environment.selection.environment_id == environment_id)
    {
        let message = if matches!(request, GuardianApprovalRequest::WriteStdin { .. }) {
            "automatic approval review cannot access the terminal's environment; select it before retrying"
        } else {
            "automatic approval review cannot access the request's environment"
        };
        return Err(ReviewDecision::denied(message));
    }
    Ok(request)
}

#[derive(Clone)]
pub(super) struct ReviewRuntime {
    pub(super) session: Arc<Session>,
    pub(super) history_reset: CancellationToken,
    pub(super) context: GuardianReviewContext,
    pub(super) request: ReviewAction,
    pub(super) reasons: ApprovalRequestReasons,
    pub(super) options: GuardianReviewOptions,
}
