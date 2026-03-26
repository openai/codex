use codex_protocol::protocol::SandboxPolicy;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

const SYSTEM_BWRAP_PROGRAM: &str = "bwrap";
const MISSING_BWRAP_WARNING: &str = "Codex could not find system bubblewrap on PATH. Please install bubblewrap with your package manager. Codex will use the vendored bubblewrap in the meantime.";
const USER_NAMESPACE_WARNING: &str = "Codex's linux sandbox uses bubblewrap and needs access to create user namespaces. Install bubblewrap for your Linux distribution.";
const USER_NAMESPACE_FAILURES: [&str; 4] = [
    "loopback: Failed RTM_NEWADDR",
    "loopback: Failed RTM_NEWLINK",
    "setting up uid map: Permission denied",
    "No permissions to create a new namespace",
];

pub fn system_bwrap_warning(sandbox_policy: &SandboxPolicy) -> Option<String> {
    if !should_warn_about_system_bwrap(sandbox_policy) {
        return None;
    }

    let system_bwrap_path = find_system_bwrap_in_path();
    system_bwrap_warning_for_path(system_bwrap_path.as_deref())
}

fn should_warn_about_system_bwrap(sandbox_policy: &SandboxPolicy) -> bool {
    !matches!(
        sandbox_policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    )
}

fn system_bwrap_warning_for_path(system_bwrap_path: Option<&Path>) -> Option<String> {
    let Some(system_bwrap_path) = system_bwrap_path else {
        return Some(MISSING_BWRAP_WARNING.to_string());
    };

    if !system_bwrap_has_user_namespace_access(system_bwrap_path) {
        return Some(USER_NAMESPACE_WARNING.to_string());
    }

    None
}

fn system_bwrap_has_user_namespace_access(system_bwrap_path: &Path) -> bool {
    let output = match Command::new(system_bwrap_path)
        .args([
            "--unshare-user",
            "--unshare-net",
            "--ro-bind",
            "/",
            "/",
            "/bin/true",
        ])
        .output()
    {
        Ok(output) => output,
        Err(_) => return true,
    };

    output.status.success() || !is_user_namespace_failure(&output)
}

fn is_user_namespace_failure(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    USER_NAMESPACE_FAILURES
        .iter()
        .any(|failure| stderr.contains(failure))
}

pub fn find_system_bwrap_in_path() -> Option<PathBuf> {
    let search_path = std::env::var_os("PATH")?;
    let cwd = std::env::current_dir().ok()?;
    find_system_bwrap_in_search_paths(std::env::split_paths(&search_path), &cwd)
}

fn find_system_bwrap_in_search_paths(
    search_paths: impl IntoIterator<Item = PathBuf>,
    cwd: &Path,
) -> Option<PathBuf> {
    let search_path = std::env::join_paths(search_paths).ok()?;
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    which::which_in_all(SYSTEM_BWRAP_PROGRAM, Some(search_path), &cwd)
        .ok()?
        .find_map(|path| {
            let path = std::fs::canonicalize(path).ok()?;
            if path.starts_with(&cwd) {
                None
            } else {
                Some(path)
            }
        })
}

#[cfg(test)]
#[path = "bwrap_tests.rs"]
mod tests;
