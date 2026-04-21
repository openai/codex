use crate::function_tool::FunctionCallError;
use crate::safety::SafetyCheck;
use crate::safety::assess_patch_safety;
use crate::session::turn_context::TurnContext;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) enum InternalApplyPatchInvocation {
    /// The `apply_patch` call was handled programmatically, without any sort
    /// of sandbox, because the user explicitly approved it. This is the
    /// result to use with the `shell` function call that contained `apply_patch`.
    Output(Result<String, FunctionCallError>),

    /// The `apply_patch` call was approved, either automatically because it
    /// appears that it should be allowed based on the user's sandbox policy
    /// *or* because the user explicitly approved it. The runtime realizes the
    /// patch through the selected environment filesystem.
    DelegateToRuntime(ApplyPatchRuntimeInvocation),
}

#[derive(Debug)]
pub(crate) struct ApplyPatchRuntimeInvocation {
    pub(crate) action: ApplyPatchAction,
    pub(crate) auto_approved: bool,
    pub(crate) exec_approval_requirement: ExecApprovalRequirement,
}

pub(crate) async fn apply_patch(
    turn_context: &TurnContext,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    action: ApplyPatchAction,
) -> InternalApplyPatchInvocation {
    let cwd = &action.cwd;
    match assess_patch_safety(
        &action,
        turn_context.approval_policy.value(),
        turn_context.sandbox_policy.get(),
        file_system_sandbox_policy,
        cwd,
        turn_context.windows_sandbox_level,
    ) {
        SafetyCheck::AutoApprove {
            user_explicitly_approved,
            ..
        } => InternalApplyPatchInvocation::DelegateToRuntime(ApplyPatchRuntimeInvocation {
            action,
            auto_approved: !user_explicitly_approved,
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        }),
        SafetyCheck::AskUser => {
            // Delegate the approval prompt (including cached approvals) to the
            // tool runtime, consistent with how shell/unified_exec approvals
            // are orchestrator-driven.
            InternalApplyPatchInvocation::DelegateToRuntime(ApplyPatchRuntimeInvocation {
                action,
                auto_approved: false,
                exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: None,
                },
            })
        }
        SafetyCheck::Reject { reason } => InternalApplyPatchInvocation::Output(Err(
            FunctionCallError::RespondToModel(format!("patch rejected: {reason}")),
        )),
    }
}

pub(crate) fn convert_apply_patch_to_protocol(
    action: &ApplyPatchAction,
) -> HashMap<PathBuf, FileChange> {
    let changes = action.changes();
    let mut result = HashMap::with_capacity(changes.len());
    for (path, change) in changes {
        let protocol_change = match change {
            ApplyPatchFileChange::Add { content } => FileChange::Add {
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
                move_path: move_path.clone(),
            },
        };
        result.insert(path.clone(), protocol_change);
    }
    result
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
