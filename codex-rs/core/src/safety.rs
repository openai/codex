use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;

use crate::exec::SandboxType;

use crate::command_safety::is_dangerous_command::command_might_be_dangerous;
use crate::command_safety::is_safe_command::is_known_safe_command;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;

#[derive(Debug, PartialEq)]
pub enum SafetyCheck {
    AutoApprove {
        sandbox_type: SandboxType,
        user_explicitly_approved: bool,
    },
    AskUser,
    Reject {
        reason: String,
    },
}

pub fn assess_patch_safety(
    action: &ApplyPatchAction,
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> SafetyCheck {
    if action.is_empty() {
        return SafetyCheck::Reject {
            reason: "empty patch".to_string(),
        };
    }

    match policy {
        AskForApproval::OnFailure | AskForApproval::Never | AskForApproval::OnRequest => {
            // Continue to see if this can be auto-approved.
        }
        // TODO(ragona): I'm not sure this is actually correct? I believe in this case
        // we want to continue to the writable paths check before asking the user.
        AskForApproval::UnlessTrusted => {
            return SafetyCheck::AskUser;
        }
    }

    // Even though the patch *appears* to be constrained to writable paths, it
    // is possible that paths in the patch are hard links to files outside the
    // writable roots, so we should still run `apply_patch` in a sandbox in that
    // case.
    if is_write_patch_constrained_to_writable_paths(action, sandbox_policy, cwd)
        || policy == AskForApproval::OnFailure
    {
        // Only auto‑approve when we can actually enforce a sandbox. Otherwise
        // fall back to asking the user because the patch may touch arbitrary
        // paths outside the project.
        match get_platform_sandbox() {
            Some(sandbox_type) => SafetyCheck::AutoApprove {
                sandbox_type,
                user_explicitly_approved: false,
            },
            None if sandbox_policy == &SandboxPolicy::DangerFullAccess => {
                // If the user has explicitly requested DangerFullAccess, then
                // we can auto-approve even without a sandbox.
                SafetyCheck::AutoApprove {
                    sandbox_type: SandboxType::None,
                    user_explicitly_approved: false,
                }
            }
            None => SafetyCheck::AskUser,
        }
    } else if policy == AskForApproval::Never {
        SafetyCheck::Reject {
            reason: "writing outside of the project; rejected by user approval settings"
                .to_string(),
        }
    } else {
        SafetyCheck::AskUser
    }
}

/// For a command to be run _without_ a sandbox, one of the following must be
/// true:
///
/// - the user has explicitly approved the command
/// - the command is on the "known safe" list
/// - `DangerFullAccess` was specified and `UnlessTrusted` was not
pub fn assess_command_safety(
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    approved: &HashSet<Vec<String>>,
    with_escalated_permissions: bool,
) -> SafetyCheck {
    // Some commands look dangerous. Even if they are run inside a sandbox,
    // unless the user has explicitly approved them, we should ask,
    // or reject if the approval_policy tells us not to ask.
    if command_might_be_dangerous(command) && !approved.contains(command) {
        if approval_policy == AskForApproval::Never {
            return SafetyCheck::Reject {
                reason: "dangerous command detected; rejected by user approval settings"
                    .to_string(),
            };
        }

        return SafetyCheck::AskUser;
    }

    // A command is "trusted" because either:
    // - it belongs to a set of commands we consider "safe" by default, or
    // - the user has explicitly approved the command for this session
    //
    // Currently, whether a command is "trusted" is a simple boolean, but we
    // should include more metadata on this command test to indicate whether it
    // should be run inside a sandbox or not. (This could be something the user
    // defines as part of `execpolicy`.)
    //
    // For example, when `is_known_safe_command(command)` returns `true`, it
    // would probably be fine to run the command in a sandbox, but when
    // `approved.contains(command)` is `true`, the user may have approved it for
    // the session _because_ they know it needs to run outside a sandbox.

    if is_known_safe_command(command) || approved.contains(command) {
        let user_explicitly_approved = approved.contains(command);
        return SafetyCheck::AutoApprove {
            sandbox_type: SandboxType::None,
            user_explicitly_approved,
        };
    }

    assess_safety_for_untrusted_command(approval_policy, sandbox_policy, with_escalated_permissions)
}

pub(crate) fn assess_safety_for_untrusted_command(
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    with_escalated_permissions: bool,
) -> SafetyCheck {
    use AskForApproval::*;
    use SandboxPolicy::*;

    match (approval_policy, sandbox_policy) {
        (UnlessTrusted, _) => {
            // Even though the user may have opted into DangerFullAccess,
            // they also requested that we ask for approval for untrusted
            // commands.
            SafetyCheck::AskUser
        }
        (OnFailure, DangerFullAccess)
        | (Never, DangerFullAccess)
        | (OnRequest, DangerFullAccess) => SafetyCheck::AutoApprove {
            sandbox_type: SandboxType::None,
            user_explicitly_approved: false,
        },
        (OnRequest, ReadOnly) | (OnRequest, WorkspaceWrite { .. }) => {
            if with_escalated_permissions {
                SafetyCheck::AskUser
            } else {
                match get_platform_sandbox() {
                    Some(sandbox_type) => SafetyCheck::AutoApprove {
                        sandbox_type,
                        user_explicitly_approved: false,
                    },
                    // Fall back to asking since the command is untrusted and
                    // we do not have a sandbox available
                    None => SafetyCheck::AskUser,
                }
            }
        }
        (Never, ReadOnly)
        | (Never, WorkspaceWrite { .. })
        | (OnFailure, ReadOnly)
        | (OnFailure, WorkspaceWrite { .. }) => {
            match get_platform_sandbox() {
                Some(sandbox_type) => SafetyCheck::AutoApprove {
                    sandbox_type,
                    user_explicitly_approved: false,
                },
                None => {
                    if matches!(approval_policy, OnFailure) {
                        // Since the command is not trusted, even though the
                        // user has requested to only ask for approval on
                        // failure, we will ask the user because no sandbox is
                        // available.
                        SafetyCheck::AskUser
                    } else {
                        // We are in non-interactive mode and lack approval, so
                        // all we can do is reject the command.
                        SafetyCheck::Reject {
                            reason: "auto-rejected because command is not on trusted list"
                                .to_string(),
                        }
                    }
                }
            }
        }
    }
}

pub fn get_platform_sandbox() -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else {
        None
    }
}

fn is_write_patch_constrained_to_writable_paths(
    action: &ApplyPatchAction,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> bool {
    // Early‑exit if there are no declared writable roots.
    let writable_roots = match sandbox_policy {
        SandboxPolicy::ReadOnly => {
            return false;
        }
        SandboxPolicy::DangerFullAccess => {
            return true;
        }
        SandboxPolicy::WorkspaceWrite { .. } => sandbox_policy.get_writable_roots_with_cwd(cwd),
    };

    // Normalize a path by removing `.` and resolving `..` without touching the
    // filesystem (works even if the file does not exist).
    fn normalize(path: &Path) -> Option<PathBuf> {
        let mut out = PathBuf::new();
        for comp in path.components() {
            match comp {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => { /* skip */ }
                other => out.push(other.as_os_str()),
            }
        }
        Some(out)
    }

    // Determine whether `path` is inside **any** writable root. Both `path`
    // and roots are converted to absolute, normalized forms before the
    // prefix check.
    let is_path_writable = |p: &PathBuf| {
        let abs = if p.is_absolute() {
            p.clone()
        } else {
            cwd.join(p)
        };
        let abs = match normalize(&abs) {
            Some(v) => v,
            None => return false,
        };

        writable_roots
            .iter()
            .any(|writable_root| writable_root.is_path_writable(&abs))
    };

    for (path, change) in action.changes() {
        match change {
            ApplyPatchFileChange::Add { .. } | ApplyPatchFileChange::Delete { .. } => {
                if !is_path_writable(path) {
                    return false;
                }
            }
            ApplyPatchFileChange::Update { move_path, .. } => {
                if !is_path_writable(path) {
                    return false;
                }
                if let Some(dest) = move_path
                    && !is_path_writable(dest)
                {
                    return false;
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_writable_roots_constraint() {
        // Use a temporary directory as our workspace to avoid touching
        // the real current working directory.
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let parent = cwd.parent().unwrap().to_path_buf();

        // Helper to build a single‑entry patch that adds a file at `p`.
        let make_add_change = |p: PathBuf| ApplyPatchAction::new_add_for_test(&p, "".to_string());

        let add_inside = make_add_change(cwd.join("inner.txt"));
        let add_outside = make_add_change(parent.join("outside.txt"));

        // Policy limited to the workspace only; exclude system temp roots so
        // only `cwd` is writable by default.
        let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert!(is_write_patch_constrained_to_writable_paths(
            &add_inside,
            &policy_workspace_only,
            &cwd,
        ));

        assert!(!is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &policy_workspace_only,
            &cwd,
        ));

        // With the parent dir explicitly added as a writable root, the
        // outside write should be permitted.
        let policy_with_parent = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![parent],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        assert!(is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &policy_with_parent,
            &cwd,
        ));
    }

    #[test]
    fn test_request_escalated_privileges() {
        // Should not be a trusted command
        let command = vec!["git commit".to_string()];
        let approval_policy = AskForApproval::OnRequest;
        let sandbox_policy = SandboxPolicy::ReadOnly;
        let approved: HashSet<Vec<String>> = HashSet::new();
        let request_escalated_privileges = true;

        let safety_check = assess_command_safety(
            &command,
            approval_policy,
            &sandbox_policy,
            &approved,
            request_escalated_privileges,
        );

        assert_eq!(safety_check, SafetyCheck::AskUser);
    }

    #[test]
    fn dangerous_command_allowed_if_explicitly_approved() {
        let command = vec!["git".to_string(), "reset".to_string(), "--hard".to_string()];
        let approval_policy = AskForApproval::OnRequest;
        let sandbox_policy = SandboxPolicy::ReadOnly;
        let mut approved: HashSet<Vec<String>> = HashSet::new();
        approved.insert(command.clone());
        let request_escalated_privileges = false;

        let safety_check = assess_command_safety(
            &command,
            approval_policy,
            &sandbox_policy,
            &approved,
            request_escalated_privileges,
        );

        assert_eq!(
            safety_check,
            SafetyCheck::AutoApprove {
                sandbox_type: SandboxType::None,
                user_explicitly_approved: true,
            }
        );
    }

    #[test]
    fn dangerous_command_not_allowed_if_not_explicitly_approved() {
        let command = vec!["git".to_string(), "reset".to_string(), "--hard".to_string()];
        let approval_policy = AskForApproval::Never;
        let sandbox_policy = SandboxPolicy::ReadOnly;
        let approved: HashSet<Vec<String>> = HashSet::new();
        let request_escalated_privileges = false;

        let safety_check = assess_command_safety(
            &command,
            approval_policy,
            &sandbox_policy,
            &approved,
            request_escalated_privileges,
        );

        assert_eq!(
            safety_check,
            SafetyCheck::Reject {
                reason: "dangerous command detected; rejected by user approval settings"
                    .to_string(),
            }
        );
    }

    #[test]
    fn test_request_escalated_privileges_no_sandbox_fallback() {
        let command = vec!["git".to_string(), "commit".to_string()];
        let approval_policy = AskForApproval::OnRequest;
        let sandbox_policy = SandboxPolicy::ReadOnly;
        let approved: HashSet<Vec<String>> = HashSet::new();
        let request_escalated_privileges = false;

        let safety_check = assess_command_safety(
            &command,
            approval_policy,
            &sandbox_policy,
            &approved,
            request_escalated_privileges,
        );

        let expected = match get_platform_sandbox() {
            Some(sandbox_type) => SafetyCheck::AutoApprove {
                sandbox_type,
                user_explicitly_approved: false,
            },
            None => SafetyCheck::AskUser,
        };
        assert_eq!(safety_check, expected);
    }
}
