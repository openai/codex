//! Discover bubblewrap dependencies separately from resolving configured policy rules.

use super::dependencies::Dependency;
use super::dependencies::codex_and_launcher_dependencies;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxType;
use codex_shell_command::shell_detect::get_user_home_path;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

pub(super) const SANDBOX_TYPE: SandboxType = SandboxType::LinuxSeccomp;
pub(super) const NAME: &str = "bubblewrap";

pub(super) fn critical_dependencies(
    request: &SandboxExecRequest,
    policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> io::Result<Vec<Dependency>> {
    let command_cwd = request.cwd.to_abs_path()?;
    let path = request
        .env
        .get("PATH")
        .map(|path| {
            std::env::join_paths(
                std::env::split_paths(path).map(|entry| command_cwd.as_path().join(entry)),
            )
        })
        .transpose()
        .map_err(io::Error::other)?;
    let mut dependencies = codex_and_launcher_dependencies(request);
    if let Some(exe) = request.command.first()
        && let Some(bwrap) = codex_linux_sandbox::find_bundled_bwrap_for_exe(
            &command_cwd.as_path().join(exe),
            command_cwd.as_path(),
            |key| request.env.get(key).map(OsString::from),
        )
    {
        dependencies.push(("bundled_bwrap_candidate", Ok(bwrap.into_path_buf())));
    }
    if let Some(path) = &path
        && let Some(bwrap) = codex_sandboxing::find_executable_in_search_paths(
            "bwrap",
            std::env::split_paths(path),
            command_cwd.as_path(),
            policy,
            cwd.as_path(),
        )
    {
        // The launcher probes this executable even if it later uses its bundled copy.
        dependencies.push(("system_bwrap_candidate", Ok(bwrap)));
    }
    if !policy
        .get_unreadable_globs_with_cwd(cwd.as_path())
        .is_empty()
        && let Some(path) = &path
        && let Some(scanner) = codex_sandboxing::find_executable_in_search_paths(
            codex_linux_sandbox::GLOB_SCAN_PROGRAM,
            std::env::split_paths(path),
            command_cwd.as_path(),
            policy,
            cwd.as_path(),
        )
    {
        dependencies.push(("glob_scanner", Ok(scanner)));
    }
    Ok(dependencies)
}

pub(super) fn prepare_policy(
    request: &SandboxExecRequest,
    mut policy: FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> io::Result<FileSystemSandboxPolicy> {
    let command_cwd = request.cwd.to_abs_path()?;
    let needs_home = policy.entries.iter().any(|entry| match &entry.path {
        FileSystemPath::GlobPattern { pattern } => pattern.starts_with('~'),
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir,
        } => request
            .env
            .get("TMPDIR")
            .is_some_and(|path| path.starts_with('~')),
        FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => false,
    });
    let prepare = || {
        // The helper resolves :tmpdir from its filtered environment, not the executor's.
        policy.entries.retain_mut(|entry| {
            if matches!(
                entry.path,
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir
                }
            ) {
                let Some(tmpdir) = request.env.get("TMPDIR").filter(|path| !path.is_empty()) else {
                    return false;
                };
                entry.path =
                    AbsolutePathBuf::resolve_path_against_base(tmpdir, &command_cwd).into();
            }
            true
        });
        let patterns = policy.get_unreadable_globs_with_cwd(cwd.as_path());
        if !patterns.is_empty() {
            let paths = codex_linux_sandbox::expand_unreadable_globs_in_environment(
                &patterns,
                cwd.as_path(),
                &policy,
                policy.glob_scan_max_depth,
                &request.env,
                command_cwd.as_path(),
            )
            .map_err(io::Error::other)?;
            policy = policy.with_expanded_deny_globs(paths);
        }
        Ok(policy)
    };
    if !needs_home {
        return prepare();
    }
    let home = request
        .env
        .get("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(get_user_home_path)
        .ok_or_else(|| io::Error::other("could not resolve the command's home directory"))?;
    AbsolutePathBufGuard::with_home_directory(&home, prepare)
}
