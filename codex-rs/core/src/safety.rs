use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;

use crate::exec::SandboxType;
use crate::util::resolve_path;

use crate::protocol::AskForApproval;
use crate::protocol::FileSystemSandboxPolicy;
use crate::protocol::SandboxPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;

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
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    windows_sandbox_level: WindowsSandboxLevel,
) -> SafetyCheck {
    if action.is_empty() {
        return SafetyCheck::Reject {
            reason: "empty patch".to_string(),
        };
    }

    match policy {
        AskForApproval::OnFailure
        | AskForApproval::Never
        | AskForApproval::OnRequest
        | AskForApproval::Reject(_) => {
            // Continue to see if this can be auto-approved.
        }
        // TODO(ragona): I'm not sure this is actually correct? I believe in this case
        // we want to continue to the writable paths check before asking the user.
        AskForApproval::UnlessTrusted => {
            return SafetyCheck::AskUser;
        }
    }

    let rejects_sandbox_approval = matches!(policy, AskForApproval::Never)
        || matches!(
            policy,
            AskForApproval::Reject(reject_config) if reject_config.sandbox_approval
        );

    // Even though the patch appears to be constrained to writable paths, it is
    // possible that paths in the patch are hard links to files outside the
    // writable roots, so we should still run `apply_patch` in a sandbox in that case.
    if is_write_patch_constrained_to_writable_paths(action, file_system_sandbox_policy, cwd)
        || matches!(policy, AskForApproval::OnFailure)
    {
        if matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        ) {
            // DangerFullAccess is intended to bypass sandboxing entirely.
            SafetyCheck::AutoApprove {
                sandbox_type: SandboxType::None,
                user_explicitly_approved: false,
            }
        } else {
            // Only auto‑approve when we can actually enforce a sandbox. Otherwise
            // fall back to asking the user because the patch may touch arbitrary
            // paths outside the project.
            match get_platform_sandbox(windows_sandbox_level != WindowsSandboxLevel::Disabled) {
                Some(sandbox_type) => SafetyCheck::AutoApprove {
                    sandbox_type,
                    user_explicitly_approved: false,
                },
                None => {
                    if rejects_sandbox_approval {
                        SafetyCheck::Reject {
                            reason:
                                "writing outside of the project; rejected by user approval settings"
                                    .to_string(),
                        }
                    } else {
                        SafetyCheck::AskUser
                    }
                }
            }
        }
    } else if rejects_sandbox_approval {
        SafetyCheck::Reject {
            reason: "writing outside of the project; rejected by user approval settings"
                .to_string(),
        }
    } else {
        SafetyCheck::AskUser
    }
}

pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        if windows_sandbox_enabled {
            Some(SandboxType::WindowsRestrictedToken)
        } else {
            None
        }
    } else {
        None
    }
}

fn is_write_patch_constrained_to_writable_paths(
    action: &ApplyPatchAction,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> bool {
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

    let unreadable_roots = file_system_sandbox_policy.get_unreadable_roots_with_cwd(cwd);
    let writable_roots = file_system_sandbox_policy.get_writable_roots_with_cwd(cwd);

    // Determine whether `path` is inside **any** writable root. Both `path`
    // and roots are converted to absolute, normalized forms before the
    // prefix check.
    let is_path_writable = |p: &PathBuf| {
        let abs = resolve_path(cwd, p);
        let abs = match normalize(&abs) {
            Some(v) => v,
            None => return false,
        };

        if unreadable_roots
            .iter()
            .any(|root| abs.starts_with(root.as_path()))
        {
            return false;
        }

        if file_system_sandbox_policy.has_full_disk_write_access() {
            return true;
        }

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
    use codex_protocol::protocol::FileSystemAccessMode;
    use codex_protocol::protocol::FileSystemPath;
    use codex_protocol::protocol::FileSystemSandboxEntry;
    use codex_protocol::protocol::FileSystemSpecialPath;
    use codex_protocol::protocol::RejectConfig;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
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
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert!(is_write_patch_constrained_to_writable_paths(
            &add_inside,
            &FileSystemSandboxPolicy::from(&policy_workspace_only),
            &cwd,
        ));

        assert!(!is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &FileSystemSandboxPolicy::from(&policy_workspace_only),
            &cwd,
        ));

        // With the parent dir explicitly added as a writable root, the
        // outside write should be permitted.
        let policy_with_parent = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![AbsolutePathBuf::try_from(parent).unwrap()],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        assert!(is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &FileSystemSandboxPolicy::from(&policy_with_parent),
            &cwd,
        ));
    }

    #[test]
    fn external_sandbox_auto_approves_in_on_request() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let add_inside = ApplyPatchAction::new_add_for_test(&cwd.join("inner.txt"), "".to_string());

        let policy = SandboxPolicy::ExternalSandbox {
            network_access: codex_protocol::protocol::NetworkAccess::Enabled,
        };

        assert_eq!(
            assess_patch_safety(
                &add_inside,
                AskForApproval::OnRequest,
                &policy,
                &FileSystemSandboxPolicy::from(&policy),
                &cwd,
                WindowsSandboxLevel::Disabled
            ),
            SafetyCheck::AutoApprove {
                sandbox_type: SandboxType::None,
                user_explicitly_approved: false,
            }
        );
    }

    #[test]
    fn reject_with_all_flags_false_matches_on_request_for_out_of_root_patch() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let parent = cwd.parent().unwrap().to_path_buf();
        let add_outside =
            ApplyPatchAction::new_add_for_test(&parent.join("outside.txt"), "".to_string());
        let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert_eq!(
            assess_patch_safety(
                &add_outside,
                AskForApproval::OnRequest,
                &policy_workspace_only,
                &FileSystemSandboxPolicy::from(&policy_workspace_only),
                &cwd,
                WindowsSandboxLevel::Disabled,
            ),
            SafetyCheck::AskUser,
        );
        assert_eq!(
            assess_patch_safety(
                &add_outside,
                AskForApproval::Reject(RejectConfig {
                    sandbox_approval: false,
                    rules: false,
                    mcp_elicitations: false,
                }),
                &policy_workspace_only,
                &FileSystemSandboxPolicy::from(&policy_workspace_only),
                &cwd,
                WindowsSandboxLevel::Disabled,
            ),
            SafetyCheck::AskUser,
        );
    }

    #[test]
    fn reject_sandbox_approval_rejects_out_of_root_patch() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let parent = cwd.parent().unwrap().to_path_buf();
        let add_outside =
            ApplyPatchAction::new_add_for_test(&parent.join("outside.txt"), "".to_string());
        let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert_eq!(
            assess_patch_safety(
                &add_outside,
                AskForApproval::Reject(RejectConfig {
                    sandbox_approval: true,
                    rules: false,
                    mcp_elicitations: false,
                }),
                &policy_workspace_only,
                &FileSystemSandboxPolicy::from(&policy_workspace_only),
                &cwd,
                WindowsSandboxLevel::Disabled,
            ),
            SafetyCheck::Reject {
                reason: "writing outside of the project; rejected by user approval settings"
                    .to_string(),
            },
        );
    }
    #[test]
    fn explicit_unreadable_paths_prevent_auto_approval_for_external_sandbox() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let blocked_path = cwd.join("blocked.txt");
        let blocked_absolute = AbsolutePathBuf::from_absolute_path(blocked_path.clone()).unwrap();
        let action = ApplyPatchAction::new_add_for_test(&blocked_path, "".to_string());
        let sandbox_policy = SandboxPolicy::ExternalSandbox {
            network_access: codex_protocol::protocol::NetworkAccess::Restricted,
        };
        let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: blocked_absolute,
                },
                access: FileSystemAccessMode::None,
            },
        ]);

        assert!(!is_write_patch_constrained_to_writable_paths(
            &action,
            &file_system_sandbox_policy,
            &cwd,
        ));
        assert_eq!(
            assess_patch_safety(
                &action,
                AskForApproval::OnRequest,
                &sandbox_policy,
                &file_system_sandbox_policy,
                &cwd,
                WindowsSandboxLevel::Disabled,
            ),
            SafetyCheck::AskUser,
        );
    }

    #[test]
    fn explicit_read_only_subpaths_prevent_auto_approval_for_external_sandbox() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let blocked_path = cwd.join("docs").join("blocked.txt");
        let docs_absolute = AbsolutePathBuf::resolve_path_against_base("docs", &cwd).unwrap();
        let action = ApplyPatchAction::new_add_for_test(&blocked_path, "".to_string());
        let sandbox_policy = SandboxPolicy::ExternalSandbox {
            network_access: codex_protocol::protocol::NetworkAccess::Restricted,
        };
        let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::CurrentWorkingDirectory,
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: docs_absolute,
                },
                access: FileSystemAccessMode::Read,
            },
        ]);

        assert!(!is_write_patch_constrained_to_writable_paths(
            &action,
            &file_system_sandbox_policy,
            &cwd,
        ));
        assert_eq!(
            assess_patch_safety(
                &action,
                AskForApproval::OnRequest,
                &sandbox_policy,
                &file_system_sandbox_policy,
                &cwd,
                WindowsSandboxLevel::Disabled,
            ),
            SafetyCheck::AskUser,
        );
    }
}
