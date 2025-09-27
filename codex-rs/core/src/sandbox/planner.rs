use std::collections::HashSet;
use std::path::Path;

use codex_apply_patch::ApplyPatchAction;

use super::apply_patch_adapter::build_exec_params_for_apply_patch;
use crate::apply_patch::ApplyPatchExec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecParams;
use crate::exec::SandboxType;
use crate::function_tool::FunctionCallError;
use crate::protocol::AskForApproval;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::safety::SafetyCheck;
use crate::safety::assess_command_safety;
use crate::safety::assess_patch_safety;

#[derive(Clone, Debug)]
pub struct ExecRequest<'a> {
    pub params: &'a ExecParams,
    pub approval: AskForApproval,
    pub policy: &'a SandboxPolicy,
    pub approved_session_commands: &'a HashSet<Vec<String>>,
}

#[derive(Clone, Debug)]
pub enum ExecPlan {
    Reject {
        reason: String,
    },
    AskUser {
        reason: Option<String>,
    },
    Approved {
        sandbox: SandboxType,
        on_failure_escalate: bool,
        approved_by_user: bool,
    },
}

impl ExecPlan {
    pub fn approved(
        sandbox: SandboxType,
        on_failure_escalate: bool,
        approved_by_user: bool,
    ) -> Self {
        ExecPlan::Approved {
            sandbox,
            on_failure_escalate,
            approved_by_user,
        }
    }
}

pub fn plan_exec(req: &ExecRequest<'_>) -> ExecPlan {
    let params = req.params;
    let with_escalated_permissions = params.with_escalated_permissions.unwrap_or(false);
    let safety = assess_command_safety(
        &params.command,
        req.approval,
        req.policy,
        req.approved_session_commands,
        with_escalated_permissions,
    );

    match safety {
        SafetyCheck::AutoApprove { sandbox_type } => ExecPlan::Approved {
            sandbox: sandbox_type,
            on_failure_escalate: should_escalate_on_failure(req.approval, sandbox_type),
            approved_by_user: false,
        },
        SafetyCheck::AskUser => ExecPlan::AskUser {
            reason: params.justification.clone(),
        },
        SafetyCheck::Reject { reason } => ExecPlan::Reject { reason },
    }
}

#[derive(Clone, Debug)]
pub struct PatchExecRequest<'a> {
    pub action: &'a ApplyPatchAction,
    pub approval: AskForApproval,
    pub policy: &'a SandboxPolicy,
    pub cwd: &'a Path,
    pub user_explicitly_approved: bool,
}

pub fn plan_apply_patch(req: &PatchExecRequest<'_>) -> ExecPlan {
    if req.user_explicitly_approved {
        ExecPlan::Approved {
            sandbox: SandboxType::None,
            on_failure_escalate: false,
            approved_by_user: true,
        }
    } else {
        match assess_patch_safety(req.action, req.approval, req.policy, req.cwd) {
            SafetyCheck::AutoApprove { sandbox_type } => ExecPlan::Approved {
                sandbox: sandbox_type,
                on_failure_escalate: should_escalate_on_failure(req.approval, sandbox_type),
                approved_by_user: false,
            },
            SafetyCheck::AskUser => ExecPlan::AskUser { reason: None },
            SafetyCheck::Reject { reason } => ExecPlan::Reject { reason },
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedExec {
    pub(crate) params: ExecParams,
    pub(crate) plan: ExecPlan,
    pub(crate) command_for_display: Vec<String>,
    pub(crate) apply_patch_exec: Option<ApplyPatchExec>,
}

pub(crate) async fn prepare_exec_invocation(
    sess: &Session,
    turn_context: &TurnContext,
    sub_id: &str,
    call_id: &str,
    params: ExecParams,
    apply_patch_exec: Option<ApplyPatchExec>,
    approved_session_commands: HashSet<Vec<String>>,
) -> Result<PreparedExec, FunctionCallError> {
    let mut params = params;

    let (plan, command_for_display) = if let Some(exec) = apply_patch_exec.as_ref() {
        params = build_exec_params_for_apply_patch(exec, &params)?;
        let command_for_display = vec!["apply_patch".to_string(), exec.action.patch.clone()];

        let plan_req = PatchExecRequest {
            action: &exec.action,
            approval: turn_context.approval_policy,
            policy: &turn_context.sandbox_policy,
            cwd: &turn_context.cwd,
            user_explicitly_approved: exec.user_explicitly_approved_this_action,
        };

        let plan = match plan_apply_patch(&plan_req) {
            plan @ ExecPlan::Approved { .. } => plan,
            ExecPlan::AskUser { .. } => {
                return Err(FunctionCallError::RespondToModel(
                    "patch requires approval but none was recorded".to_string(),
                ));
            }
            ExecPlan::Reject { reason } => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "patch rejected: {reason}"
                )));
            }
        };

        (plan, command_for_display)
    } else {
        let command_for_display = params.command.clone();

        let initial_plan = plan_exec(&ExecRequest {
            params: &params,
            approval: turn_context.approval_policy,
            policy: &turn_context.sandbox_policy,
            approved_session_commands: &approved_session_commands,
        });

        let plan = match initial_plan {
            plan @ ExecPlan::Approved { .. } => plan,
            ExecPlan::AskUser { reason } => {
                let decision = sess
                    .request_command_approval(
                        sub_id.to_string(),
                        call_id.to_string(),
                        params.command.clone(),
                        params.cwd.clone(),
                        reason,
                    )
                    .await;
                match decision {
                    ReviewDecision::Approved => ExecPlan::approved(SandboxType::None, false, true),
                    ReviewDecision::ApprovedForSession => {
                        sess.add_approved_command(params.command.clone()).await;
                        ExecPlan::approved(SandboxType::None, false, true)
                    }
                    ReviewDecision::Denied | ReviewDecision::Abort => {
                        return Err(FunctionCallError::RespondToModel(
                            "exec command rejected by user".to_string(),
                        ));
                    }
                }
            }
            ExecPlan::Reject { reason } => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "exec command rejected: {reason:?}"
                )));
            }
        };

        (plan, command_for_display)
    };

    Ok(PreparedExec {
        params,
        plan,
        command_for_display,
        apply_patch_exec,
    })
}

fn should_escalate_on_failure(approval: AskForApproval, sandbox: SandboxType) -> bool {
    matches!(
        (approval, sandbox),
        (
            AskForApproval::UnlessTrusted | AskForApproval::OnFailure,
            SandboxType::MacosSeatbelt | SandboxType::LinuxSeccomp
        )
    )
}
