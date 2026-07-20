//! Central approval policy-stage execution and reviewer routing.

use crate::guardian::guardian_timeout_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian_with_reviewer;
use crate::hook_runtime::run_permission_request_hooks;
use crate::sandboxing::SandboxPermissions;
use crate::session::turn_context::TurnContext;
use crate::tools::flat_tool_name;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_config::types::ApprovalsReviewer;
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ApprovalAction {
    Shell {
        id: String,
        environment_id: String,
        command: Vec<String>,
        cwd: PathUri,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
        justification: Option<String>,
    },
    ExecCommand {
        id: String,
        environment_id: String,
        command: Vec<String>,
        cwd: PathUri,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
        justification: Option<String>,
        tty: bool,
    },
    ApplyPatch {
        id: String,
        environment_id: String,
        cwd: PathUri,
        files: Vec<PathUri>,
        patch: String,
    },
}

impl ApprovalAction {
    fn into_guardian_request(self) -> std::io::Result<crate::guardian::GuardianApprovalRequest> {
        Ok(match self {
            Self::Shell {
                id,
                environment_id,
                command,
                cwd,
                sandbox_permissions,
                additional_permissions,
                justification,
            } => crate::guardian::GuardianApprovalRequest::Shell {
                id,
                command,
                cwd: guardian_cwd(&environment_id, cwd)?,
                sandbox_permissions,
                additional_permissions,
                justification,
            },
            Self::ExecCommand {
                id,
                environment_id,
                command,
                cwd,
                sandbox_permissions,
                additional_permissions,
                justification,
                tty,
            } => crate::guardian::GuardianApprovalRequest::ExecCommand {
                id,
                command,
                cwd: guardian_cwd(&environment_id, cwd)?,
                sandbox_permissions,
                additional_permissions,
                justification,
                tty,
            },
            Self::ApplyPatch {
                id,
                environment_id,
                cwd,
                files,
                patch,
            } => crate::guardian::GuardianApprovalRequest::ApplyPatch {
                id,
                cwd: guardian_cwd(&environment_id, cwd)?,
                files: files
                    .into_iter()
                    .map(|path| path.to_abs_path())
                    .collect::<std::io::Result<Vec<_>>>()?,
                patch,
            },
        })
    }
}

fn guardian_cwd(environment_id: &str, cwd: PathUri) -> std::io::Result<AbsolutePathBuf> {
    match cwd.to_abs_path() {
        Ok(cwd) => Ok(cwd),
        Err(err) if environment_id != codex_exec_server::LOCAL_ENVIRONMENT_ID => Err(err),
        Err(_) => {
            let cwd_display = cwd.to_string();
            let path = cwd.to_url().to_file_path().map_err(|()| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("local cwd URI `{cwd_display}` is not a host-native path"),
                )
            })?;
            AbsolutePathBuf::from_absolute_path_checked(path).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("local cwd URI `{cwd_display}` is not absolute: {err}"),
                )
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApprovalReviewer {
    Guardian,
    User,
}

impl ApprovalReviewer {
    pub(super) fn for_turn(turn: &TurnContext) -> Self {
        Self::for_reviewer(turn, turn.config.approvals_reviewer)
    }

    fn for_reviewer(turn: &TurnContext, reviewer: ApprovalsReviewer) -> Self {
        if routes_approval_to_guardian_with_reviewer(turn, reviewer) {
            Self::Guardian
        } else {
            Self::User
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApprovalResolutionSource {
    Hook,
    Guardian,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovalResolution {
    decision: ReviewDecision,
    source: ApprovalResolutionSource,
}

impl ApprovalResolution {
    fn into_tool_result(self) -> Result<ReviewDecision, ToolError> {
        let source = self.source;
        match self.decision {
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } if network_policy_amendment.action == NetworkPolicyRuleAction::Deny => {
                let rejection = match source {
                    ApprovalResolutionSource::Hook => "rejected by configuration",
                    ApprovalResolutionSource::Guardian => {
                        "automatic approval review denied the action"
                    }
                    ApprovalResolutionSource::User => "rejected by user",
                };
                Err(ToolError::Rejected(rejection.to_string()))
            }
            ReviewDecision::Denied { rejection } => Err(ToolError::Rejected(rejection)),
            ReviewDecision::TimedOut => Err(ToolError::Rejected(guardian_timeout_message())),
            ReviewDecision::Abort => {
                Err(ToolError::Rejected("approval request aborted".to_string()))
            }
            decision => Ok(decision),
        }
    }
}

pub(super) async fn resolve_tool_apporval<Rq, Out, T>(
    tool: &mut T,
    req: &Rq,
    permission_request_run_id: &str,
    ctx: ApprovalCtx<'_>,
    tool_ctx: &ToolCtx,
    reviewer: ApprovalReviewer,
    otel: &codex_otel::SessionTelemetry,
) -> Result<ReviewDecision, ToolError>
where
    T: ToolRuntime<Rq, Out>,
{
    if let Some(permission_request) = tool.permission_request_payload(req) {
        match run_permission_request_hooks(
            ctx.session,
            ctx.turn,
            permission_request_run_id,
            permission_request,
        )
        .await
        {
            Some(PermissionRequestDecision::Allow) => {
                let resolution = ApprovalResolution {
                    decision: ReviewDecision::Approved,
                    source: ApprovalResolutionSource::Hook,
                };
                record_resolution(otel, tool_ctx, &resolution);
                return resolution.into_tool_result();
            }
            Some(PermissionRequestDecision::Deny { message }) => {
                let resolution = ApprovalResolution {
                    decision: ReviewDecision::denied(message),
                    source: ApprovalResolutionSource::Hook,
                };
                record_resolution(otel, tool_ctx, &resolution);
                return resolution.into_tool_result();
            }
            None => {}
        }
    }

    let resolution = match reviewer {
        ApprovalReviewer::Guardian => {
            let review_id = new_guardian_review_id();
            let action = match tool
                .approval_action(req, &ctx)
                .and_then(ApprovalAction::into_guardian_request)
            {
                Ok(action) => action,
                Err(err) => {
                    tracing::error!(%err, "failed to build automatic approval action");
                    let resolution = ApprovalResolution {
                        decision: ReviewDecision::denied(
                            "automatic approval review could not prepare the action",
                        ),
                        source: ApprovalResolutionSource::Guardian,
                    };
                    record_resolution(otel, tool_ctx, &resolution);
                    return resolution.into_tool_result();
                }
            };
            review_approval_request(
                ctx.session,
                ctx.turn,
                review_id,
                action,
                ctx.retry_reason.clone(),
            )
            .await
        }
        ApprovalReviewer::User => tool.start_approval_async(req, ctx.clone()).await,
    };
    let source = match reviewer {
        ApprovalReviewer::Guardian => ApprovalResolutionSource::Guardian,
        ApprovalReviewer::User => ApprovalResolutionSource::User,
    };
    let resolution = ApprovalResolution {
        decision: resolution,
        source,
    };
    record_resolution(otel, tool_ctx, &resolution);
    resolution.into_tool_result()
}

fn record_resolution(
    otel: &codex_otel::SessionTelemetry,
    tool_ctx: &ToolCtx,
    resolution: &ApprovalResolution,
) {
    let source = match resolution.source {
        ApprovalResolutionSource::Hook => ToolDecisionSource::Config,
        ApprovalResolutionSource::Guardian => ToolDecisionSource::AutomatedReviewer,
        ApprovalResolutionSource::User => ToolDecisionSource::User,
    };
    let tool_name = flat_tool_name(&tool_ctx.tool_name);
    otel.tool_decision(
        tool_name.as_ref(),
        &tool_ctx.call_id,
        &resolution.decision,
        source,
    );
}

#[cfg(all(test, unix))]
#[path = "approvals_tests.rs"]
mod tests;
