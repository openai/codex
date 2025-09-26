use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::protocol::FileChange;
use crate::protocol::ReviewDecision;
use crate::safety::SafetyCheck;
use crate::safety::assess_patch_safety;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use std::collections::HashMap;
use std::path::PathBuf;

pub const CODEX_APPLY_PATCH_ARG1: &str = "--codex-run-as-apply-patch";

pub(crate) enum InternalApplyPatchInvocation {
    /// The `apply_patch` call was handled programmatically, without any sort
    /// of sandbox, because the user explicitly approved it. This is the
    /// result to use with the `shell` function call that contained `apply_patch`.
    Output(Result<String, FunctionCallError>),

    /// The `apply_patch` call was approved, either automatically because it
    /// appears that it should be allowed based on the user's sandbox policy
    /// *or* because the user explicitly approved it. In either case, we use
    /// exec with [`CODEX_APPLY_PATCH_ARG1`] to realize the `apply_patch` call,
    /// but [`ApplyPatchExec::auto_approved`] is used to determine the sandbox
    /// used with the `exec()`.
    DelegateToExec(ApplyPatchExec),
}

pub(crate) struct ApplyPatchExec {
    pub(crate) action: ApplyPatchAction,
    pub(crate) user_explicitly_approved_this_action: bool,
}

pub(crate) async fn apply_patch(
    sess: &Session,
    turn_context: &TurnContext,
    sub_id: &str,
    call_id: &str,
    action: ApplyPatchAction,
) -> InternalApplyPatchInvocation {
    match assess_patch_safety(
        &action,
        turn_context.approval_policy,
        &turn_context.sandbox_policy,
        &turn_context.cwd,
    ) {
        SafetyCheck::AutoApprove { .. } => {
            InternalApplyPatchInvocation::DelegateToExec(ApplyPatchExec {
                action,
                user_explicitly_approved_this_action: false,
            })
        }
        SafetyCheck::AskUser => {
            // Compute a readable summary of path changes to include in the
            // approval request so the user can make an informed decision.
            //
            // Note that it might be worth expanding this approval request to
            // give the user the option to expand the set of writable roots so
            // that similar patches can be auto-approved in the future during
            // this session.
            let rx_approve = sess
                .request_patch_approval(sub_id.to_owned(), call_id.to_owned(), &action, None, None)
                .await;
            match rx_approve.await.unwrap_or_default() {
                ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {
                    InternalApplyPatchInvocation::DelegateToExec(ApplyPatchExec {
                        action,
                        user_explicitly_approved_this_action: true,
                    })
                }
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    InternalApplyPatchInvocation::Output(Err(FunctionCallError::RespondToModel(
                        "patch rejected by user".to_string(),
                    )))
                }
            }
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
