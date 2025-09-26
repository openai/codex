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

fn has_no_actual_changes(action: &ApplyPatchAction) -> bool {
    if action.is_empty() {
        return true;
    }

    for (_, change) in action.changes() {
        match change {
            ApplyPatchFileChange::Add { .. } => return false,
            ApplyPatchFileChange::Delete { .. } => return false,
            ApplyPatchFileChange::Update { unified_diff, move_path, .. } => {
                if move_path.is_some() {
                    return false;
                }
                let (added, removed) = calculate_changes_from_diff(unified_diff);
                if added > 0 || removed > 0 {
                    return false;
                }
            }
        }
    }
    true
}

fn calculate_changes_from_diff(diff: &str) -> (usize, usize) {
    if diff.trim().is_empty() {
        return (0, 0);
    }

    let mut added = 0;
    let mut removed = 0;

    for line in diff.lines() {
        if let Some(first_char) = line.chars().next() {
            match first_char {
                '+' => {
                    if !line.starts_with("+++ ") {
                        added += 1;
                    }
                }
                '-' => {
                    if !line.starts_with("--- ") {
                        removed += 1;
                    }
                }
                _ => {}
            }
        }
    }

    (added, removed)
}

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
    if has_no_actual_changes(&action) {
        return InternalApplyPatchInvocation::Output(Ok(
            "No changes to apply (0 additions, 0 deletions)".to_string(),
        ));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_has_no_actual_changes_add_operations_always_real() {
        let empty_add = codex_apply_patch::ApplyPatchAction::new_add_for_test(
            Path::new("/tmp/test.txt"),
            "".to_string(),
        );
        assert!(!has_no_actual_changes(&empty_add));

        let content_add = codex_apply_patch::ApplyPatchAction::new_add_for_test(
            Path::new("/tmp/test.txt"),
            "hello world".to_string(),
        );
        assert!(!has_no_actual_changes(&content_add));
    }

    #[test]
    fn test_has_no_actual_changes_delete_operations_always_real() {
        let delete_action = codex_apply_patch::ApplyPatchAction::new_delete_for_test(
            Path::new("/tmp/test.txt"),
            "".to_string(),
        );
        assert!(!has_no_actual_changes(&delete_action));
    }

    #[test]
    fn test_calculate_changes_from_diff_empty() {
        assert_eq!(calculate_changes_from_diff(""), (0, 0));
        assert_eq!(calculate_changes_from_diff("   \n  "), (0, 0));
    }

    #[test]
    fn test_calculate_changes_from_diff_with_changes() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old line
+new line
 line3"#;
        assert_eq!(calculate_changes_from_diff(diff), (1, 1));
    }

    #[test]
    fn test_calculate_changes_from_diff_only_additions() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,3 @@
 line1
+new line
 line2"#;
        assert_eq!(calculate_changes_from_diff(diff), (1, 0));
    }

    #[test]
    fn test_calculate_changes_from_diff_only_deletions() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,2 @@
 line1
-deleted line
 line3"#;
        assert_eq!(calculate_changes_from_diff(diff), (0, 1));
    }

    #[test]
    fn test_calculate_changes_from_diff_with_increment_operators() {
        let diff = r#"--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,3 @@
 fn main() {
-++old_counter;
+++new_counter;
 }"#;
        assert_eq!(calculate_changes_from_diff(diff), (1, 1));
    }

    #[test]
    fn test_calculate_changes_from_diff_with_decrement_operators() {
        let diff = r#"--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,3 @@
 fn main() {
---old_counter;
+--new_counter;
 }"#;
        assert_eq!(calculate_changes_from_diff(diff), (1, 1));
    }

    #[test]
    fn test_has_no_actual_changes_with_rename_only() {
        use std::path::Path;
        use codex_apply_patch::ApplyPatchFileChange;

        let mut action = codex_apply_patch::ApplyPatchAction::new();
        action.add_change(
            Path::new("/tmp/old_file.txt").to_path_buf(),
            ApplyPatchFileChange::Update {
                unified_diff: "".to_string(),
                move_path: Some(Path::new("/tmp/new_file.txt").to_path_buf()),
                new_content: "same content".to_string(),
            }
        );

        assert!(!has_no_actual_changes(&action));
    }

    #[test]
    fn test_has_no_actual_changes_update_with_no_diff_no_rename() {
        use std::path::Path;
        use codex_apply_patch::ApplyPatchFileChange;

        let mut action = codex_apply_patch::ApplyPatchAction::new();
        action.add_change(
            Path::new("/tmp/file.txt").to_path_buf(),
            ApplyPatchFileChange::Update {
                unified_diff: "".to_string(),
                move_path: None,
                new_content: "same content".to_string(),
            }
        );

        assert!(has_no_actual_changes(&action));
    }
}
