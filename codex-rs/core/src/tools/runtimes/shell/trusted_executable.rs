//! Trust checks for executable paths reported by the zsh execve interceptor.
//!
//! Policy matching, reporting, and execution keep the resolved absolute path.
//! These helpers only recover a bare executable name for unmatched-command
//! classification after proving that the original request used that bare name
//! and the resolved host path cannot be replaced by the agent.

use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TrustedExecutableDir {
    path: PathBuf,
    canonical_path: PathBuf,
}

/// One parent-approved command that may suppress one redundant intercepted
/// child prompt in the same tool invocation.
///
/// Matching re-runs the trusted-path proof and compares the complete normalized
/// argv. The value is consumed on success and is never persisted or reused.
#[derive(Debug)]
pub(super) struct ParentApprovedIntercept {
    command: Mutex<Option<Vec<String>>>,
}

impl ParentApprovedIntercept {
    fn new(command: Vec<String>) -> Self {
        Self {
            command: Mutex::new(Some(command)),
        }
    }

    pub(super) fn for_parent_git_approval(
        command: &[String],
        exec_approval_requirement: &ExecApprovalRequirement,
        approval_policy: AskForApproval,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<&AdditionalPermissionProfile>,
    ) -> Option<Self> {
        if approval_policy != AskForApproval::UnlessTrusted
            || sandbox_permissions != SandboxPermissions::UseDefault
            || additional_permissions.is_some()
            || !matches!(
                exec_approval_requirement,
                ExecApprovalRequirement::NeedsApproval { reason: None, .. }
            )
        {
            return None;
        }

        crate::exec_policy::single_plain_git_command(command).map(Self::new)
    }

    pub(super) fn consume_if_matches(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        trusted_executable_dirs: &[TrustedExecutableDir],
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        cwd: &AbsolutePathBuf,
    ) -> bool {
        let Some(intercepted_command) = trusted_intercepted_command(
            program,
            argv,
            trusted_executable_dirs,
            file_system_sandbox_policy,
            cwd,
        ) else {
            return false;
        };
        let Ok(mut approved_command) = self.command.lock() else {
            return false;
        };
        if approved_command.as_deref() != Some(intercepted_command.as_slice()) {
            return false;
        }
        approved_command.take();
        true
    }
}

pub(super) fn trusted_executable_dirs(
    env: &HashMap<String, String>,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Vec<TrustedExecutableDir> {
    env.get("PATH")
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|path| path.is_absolute())
        .filter_map(|path| {
            if agent_can_write_path(file_system_sandbox_policy, cwd, &path) {
                return None;
            }
            let canonical_path = std::fs::canonicalize(&path).ok()?;
            if agent_can_write_path(file_system_sandbox_policy, cwd, &canonical_path) {
                return None;
            }
            Some(TrustedExecutableDir {
                path,
                canonical_path,
            })
        })
        .collect()
}

pub(super) fn trusted_intercepted_executable_name(
    program: &AbsolutePathBuf,
    argv: &[String],
    trusted_executable_dirs: &[TrustedExecutableDir],
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Option<String> {
    let argv_zero = argv.first()?;
    let argv_zero_path = Path::new(argv_zero);
    let is_bare_name = argv_zero_path.components().count() == 1;
    let resolved_name_matches = program.as_path().file_name() == Some(argv_zero_path.as_os_str());
    let resolved_from_trusted_path = program.as_path().parent().is_some_and(|parent| {
        trusted_executable_dirs.iter().any(|directory| {
            directory.path == parent
                && std::fs::canonicalize(&directory.path)
                    .is_ok_and(|canonical_path| canonical_path == directory.canonical_path)
        })
    });
    let resolved_target_is_read_only =
        std::fs::canonicalize(program.as_path())
            .ok()
            .is_some_and(|canonical_program| {
                !agent_can_write_path(file_system_sandbox_policy, cwd, &canonical_program)
            });

    if is_bare_name
        && resolved_name_matches
        && resolved_from_trusted_path
        && resolved_target_is_read_only
    {
        Some(argv_zero.clone())
    } else {
        None
    }
}

fn trusted_intercepted_command(
    program: &AbsolutePathBuf,
    argv: &[String],
    trusted_executable_dirs: &[TrustedExecutableDir],
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Option<Vec<String>> {
    let executable_name = trusted_intercepted_executable_name(
        program,
        argv,
        trusted_executable_dirs,
        file_system_sandbox_policy,
        cwd,
    )?;
    Some(
        std::iter::once(executable_name)
            .chain(argv.iter().skip(1).cloned())
            .collect(),
    )
}

fn agent_can_write_path(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
    path: &Path,
) -> bool {
    if !file_system_sandbox_policy.has_full_disk_write_access() {
        return path.ancestors().any(|ancestor| {
            file_system_sandbox_policy.can_write_path_with_cwd(ancestor, cwd.as_path())
        });
    }

    path.ancestors().any(|ancestor| {
        let Ok(ancestor) = CString::new(ancestor.as_os_str().as_bytes()) else {
            return true;
        };
        // SAFETY: `ancestor` is a NUL-terminated C string that remains alive
        // for the duration of this read-only access check.
        unsafe { libc::access(ancestor.as_ptr(), libc::W_OK) == 0 }
    })
}

#[cfg(test)]
#[path = "trusted_executable_tests.rs"]
mod tests;
