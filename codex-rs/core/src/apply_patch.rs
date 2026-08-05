use crate::function_tool::FunctionCallError;
use crate::safety::SafetyCheck;
use crate::safety::assess_patch_safety;
use crate::session::turn_context::TurnContext;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct ApplyPatchRuntimeInvocation {
    pub(crate) action: ApplyPatchAction,
    pub(crate) auto_approved: bool,
    pub(crate) exec_approval_requirement: ExecApprovalRequirement,
}

pub(crate) fn prepare_apply_patch(
    turn_context: &TurnContext,
    permission_profile: &PermissionProfile,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    action: ApplyPatchAction,
) -> Result<ApplyPatchRuntimeInvocation, FunctionCallError> {
    match assess_patch_safety(
        &action,
        turn_context.approval_policy(),
        permission_profile,
        file_system_sandbox_policy,
        &action.cwd,
        turn_context.windows_sandbox_level,
    ) {
        SafetyCheck::AutoApprove => Ok(ApplyPatchRuntimeInvocation {
            action,
            auto_approved: true,
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        }),
        SafetyCheck::AskUser => {
            // Delegate the approval prompt (including cached approvals) to the
            // tool runtime, consistent with how shell/unified_exec approvals
            // are orchestrator-driven.
            Ok(ApplyPatchRuntimeInvocation {
                action,
                auto_approved: false,
                exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: None,
                },
            })
        }
        SafetyCheck::Reject { reason } => Err(FunctionCallError::RespondToModel(format!(
            "patch rejected: {reason}"
        ))),
    }
}

pub(crate) fn convert_apply_patch_to_protocol(
    action: &ApplyPatchAction,
) -> HashMap<PathBuf, FileChange> {
    let mut result = HashMap::with_capacity(action.changes().len());
    for (path, change) in action.changes() {
        let protocol_change = match change {
            ApplyPatchFileChange::Add { content, .. } => FileChange::Add {
                content: content.clone(),
            },
            ApplyPatchFileChange::Delete { content } => FileChange::Delete {
                content: content.clone(),
            },
            ApplyPatchFileChange::Update {
                unified_diff,
                move_path,
                new_content: _new_content,
            } => FileChange::Update {
                unified_diff: unified_diff.clone(),
                move_path: move_path.as_ref().map(PathUri::to_path_buf),
            },
        };
        // TODO(anp): Carry PathUri through patch protocol events once app-server and rollout
        // compatibility no longer require path-flavored strings.
        result.insert(path.to_path_buf(), protocol_change);
    }
    result
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
