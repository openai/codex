use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;

use crate::exec::SandboxType;
use crate::util::resolve_path;

use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;

#[cfg(target_os = "windows")]
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "windows")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "windows")]
static WINDOWS_SANDBOX_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static WINDOWS_ELEVATED_SANDBOX_ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub fn set_windows_sandbox_enabled(enabled: bool) {
    WINDOWS_SANDBOX_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn set_windows_sandbox_enabled(_enabled: bool) {}

#[cfg(target_os = "windows")]
pub fn set_windows_elevated_sandbox_enabled(enabled: bool) {
    WINDOWS_ELEVATED_SANDBOX_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn set_windows_elevated_sandbox_enabled(_enabled: bool) {}

#[cfg(target_os = "windows")]
pub fn is_windows_elevated_sandbox_enabled() -> bool {
    WINDOWS_ELEVATED_SANDBOX_ENABLED.load(Ordering::Relaxed)
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn is_windows_elevated_sandbox_enabled() -> bool {
    false
}

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

    // Even though the patch appears to be constrained to writable paths, it is
    // possible that paths in the patch are hard links to files outside the
    // writable roots, so we should still run `apply_patch` in a sandbox in that case.
    if is_write_patch_constrained_to_writable_paths(action, sandbox_policy, cwd)
        || policy == AskForApproval::OnFailure
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
            match get_platform_sandbox() {
                Some(sandbox_type) => SafetyCheck::AutoApprove {
                    sandbox_type,
                    user_explicitly_approved: false,
                },
                None => SafetyCheck::AskUser,
            }
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

pub fn get_platform_sandbox() -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        #[cfg(target_os = "windows")]
        {
            if WINDOWS_SANDBOX_ENABLED.load(Ordering::Relaxed) {
                return Some(SandboxType::WindowsRestrictedToken);
            }
        }
        None
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
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
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
        let abs = resolve_path(cwd, p);
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
    use codex_utils_absolute_path::AbsolutePathBuf;
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
            writable_roots: vec![AbsolutePathBuf::try_from(parent).unwrap()],
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
    fn external_sandbox_auto_approves_in_on_request() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_path_buf();
        let add_inside = ApplyPatchAction::new_add_for_test(&cwd.join("inner.txt"), "".to_string());

        let policy = SandboxPolicy::ExternalSandbox {
            network_access: codex_protocol::protocol::NetworkAccess::Enabled,
        };

        assert_eq!(
            assess_patch_safety(&add_inside, AskForApproval::OnRequest, &policy, &cwd),
            SafetyCheck::AutoApprove {
                sandbox_type: SandboxType::None,
                user_explicitly_approved: false,
            }
        );
    }
}
