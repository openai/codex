use crate::policy_transforms::should_require_platform_sandbox;
use codex_protocol::models::PermissionProfile;
use std::io::ErrorKind;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const SYSTEM_BWRAP_PROGRAM: &str = "bwrap";
const MISSING_BWRAP_WARNING: &str = concat!(
    "Codex could not find bubblewrap on PATH. ",
    "Install bubblewrap with your OS package manager. ",
    "See the sandbox prerequisites: ",
    "https://developers.openai.com/codex/concepts/sandboxing#prerequisites. ",
    "Codex will use the bundled bubblewrap in the meantime.",
);
const USER_NAMESPACE_WARNING: &str =
    "Codex's Linux sandbox uses bubblewrap and needs access to create user namespaces.";
pub(crate) const WSL1_BWRAP_WARNING: &str = concat!(
    "Codex's Linux sandbox uses bubblewrap, which is not supported on WSL1 ",
    "because WSL1 cannot create the required user namespaces. ",
    "Use WSL2 for sandboxed shell commands."
);
const USER_NAMESPACE_FAILURES: [&str; 4] = [
    "loopback: Failed RTM_NEWADDR",
    "loopback: Failed RTM_NEWLINK",
    "setting up uid map: Permission denied",
    "No permissions to create a new namespace",
];
const SYSTEM_BWRAP_PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const SYSTEM_BWRAP_PROBE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SYSTEM_BWRAP_PROBE_STDERR_LIMIT_BYTES: u64 = 64 * 1024;

pub fn system_bwrap_warning(permission_profile: &PermissionProfile) -> Option<String> {
    if !should_warn_about_system_bwrap(permission_profile) {
        return None;
    }

    let system_bwrap_path = find_system_bwrap_in_path();
    system_bwrap_warning_for_path(system_bwrap_path.as_deref())
}

fn should_warn_about_system_bwrap(permission_profile: &PermissionProfile) -> bool {
    let (file_system_policy, network_policy) = permission_profile.to_runtime_permissions();
    should_require_platform_sandbox(
        &file_system_policy,
        network_policy,
        /*has_managed_network_requirements*/ false,
    )
}

fn system_bwrap_warning_for_path(system_bwrap_path: Option<&Path>) -> Option<String> {
    if is_wsl1() {
        return Some(WSL1_BWRAP_WARNING.to_string());
    }

    let Some(system_bwrap_path) = system_bwrap_path else {
        return Some(MISSING_BWRAP_WARNING.to_string());
    };

    if !system_bwrap_has_user_namespace_access(system_bwrap_path, SYSTEM_BWRAP_PROBE_TIMEOUT) {
        return Some(USER_NAMESPACE_WARNING.to_string());
    }

    None
}

fn system_bwrap_has_user_namespace_access(system_bwrap_path: &Path, timeout: Duration) -> bool {
    let mut child = match Command::new(system_bwrap_path)
        .args([
            "--unshare-user",
            "--unshare-net",
            "--ro-bind",
            "/",
            "/",
            "/bin/true",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return true,
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr = child.stderr.take().map_or_else(Vec::new, |stderr| {
                    let fd = stderr.as_raw_fd();
                    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                    if flags < 0
                        || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                    {
                        return Vec::new();
                    }

                    let mut bytes = Vec::new();
                    let mut stderr = stderr.take(SYSTEM_BWRAP_PROBE_STDERR_LIMIT_BYTES);
                    if let Err(err) = stderr.read_to_end(&mut bytes)
                        && err.kind() != ErrorKind::WouldBlock
                    {
                        return bytes;
                    }
                    bytes
                });
                let output = Output {
                    status,
                    stdout: Vec::new(),
                    stderr,
                };
                return output.status.success() || !is_user_namespace_failure(&output);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return true;
                }
                thread::sleep(SYSTEM_BWRAP_PROBE_POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return true;
            }
        }
    }
}

pub(crate) fn is_wsl1() -> bool {
    std::fs::read_to_string("/proc/version")
        .is_ok_and(|proc_version| proc_version_indicates_wsl1(&proc_version))
}

fn proc_version_indicates_wsl1(proc_version: &str) -> bool {
    let proc_version = proc_version.to_ascii_lowercase();
    let mut remaining = proc_version.as_str();
    while let Some(marker) = remaining.find("wsl") {
        let version_start = marker + "wsl".len();
        let version_digits: String = remaining[version_start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(version) = version_digits.parse::<u32>() {
            return version == 1;
        }
        remaining = &remaining[version_start..];
    }

    proc_version.contains("microsoft") && !proc_version.contains("microsoft-standard")
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
