use crate::policy_transforms::should_require_platform_sandbox;
use codex_protocol::models::PermissionProfile;
use std::io::ErrorKind;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::process::ChildStderr;
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
const PROC_SYS_DISCONNECTED_WARNING: &str = concat!(
    "Codex's Linux sandbox cannot access /proc/sys because the mount is disconnected. ",
    "Restart the environment or container before running sandboxed commands.",
);
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
const SYSTEM_BWRAP_PROBE_SPAWN_RETRY_INTERVAL: Duration = Duration::from_millis(5);
const SYSTEM_BWRAP_PROBE_STDERR_LIMIT_BYTES: usize = 64 * 1024;
const SYSTEM_BWRAP_PROBE_DRAIN_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemBwrapProbeResult {
    Available,
    UserNamespaceUnavailable,
    ProcSysDisconnected,
    Failed,
}

pub fn system_bwrap_warning(
    permission_profile: &PermissionProfile,
    managed_network_active: bool,
) -> Option<String> {
    let system_bwrap_path = find_system_bwrap_in_path();
    system_bwrap_warning_with_path(
        permission_profile,
        managed_network_active,
        system_bwrap_path.as_deref(),
    )
}

fn system_bwrap_warning_with_path(
    permission_profile: &PermissionProfile,
    managed_network_active: bool,
    system_bwrap_path: Option<&Path>,
) -> Option<String> {
    let (file_system_policy, network_policy) = permission_profile.to_runtime_permissions();
    if !should_require_platform_sandbox(&file_system_policy, network_policy, managed_network_active)
    {
        return None;
    }

    system_bwrap_warning_for_path(
        system_bwrap_path,
        /*unshare_network*/ !network_policy.is_enabled() || managed_network_active,
    )
}

fn system_bwrap_warning_for_path(
    system_bwrap_path: Option<&Path>,
    unshare_network: bool,
) -> Option<String> {
    if is_wsl1() {
        return Some(WSL1_BWRAP_WARNING.to_string());
    }

    let Some(system_bwrap_path) = system_bwrap_path else {
        return Some(MISSING_BWRAP_WARNING.to_string());
    };

    match probe_system_bwrap(
        system_bwrap_path,
        SYSTEM_BWRAP_PROBE_TIMEOUT,
        unshare_network,
    ) {
        SystemBwrapProbeResult::Available => None,
        SystemBwrapProbeResult::UserNamespaceUnavailable => {
            Some(USER_NAMESPACE_WARNING.to_string())
        }
        SystemBwrapProbeResult::ProcSysDisconnected => {
            Some(PROC_SYS_DISCONNECTED_WARNING.to_string())
        }
        // The probe deliberately exercises only a minimal bubblewrap command,
        // so an unclassified failure does not prove that the real launch will
        // fail. Warn only for failures that identify an actionable host issue.
        SystemBwrapProbeResult::Failed => None,
    }
}

fn probe_system_bwrap(
    system_bwrap_path: &Path,
    timeout: Duration,
    unshare_network: bool,
) -> SystemBwrapProbeResult {
    let mut command = Command::new(system_bwrap_path);
    command.arg("--unshare-user");
    if unshare_network {
        command.arg("--unshare-net");
    }
    command
        .args(["--ro-bind", "/", "/"])
        .arg(resolve_true_command())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let deadline = Instant::now() + timeout;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(err) if err.raw_os_error() == Some(libc::ETXTBSY) && Instant::now() < deadline => {
                thread::sleep(SYSTEM_BWRAP_PROBE_SPAWN_RETRY_INTERVAL);
            }
            Err(_) => return SystemBwrapProbeResult::Failed,
        }
    };
    let Some(mut stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return SystemBwrapProbeResult::Failed;
    };
    let fd = stderr.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        let _ = child.kill();
        let _ = child.wait();
        return SystemBwrapProbeResult::Failed;
    }

    let mut stderr_bytes = Vec::new();
    loop {
        read_available_probe_stderr(&mut stderr, &mut stderr_bytes);
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return SystemBwrapProbeResult::Available;
                }
                read_available_probe_stderr(&mut stderr, &mut stderr_bytes);
                let output = Output {
                    status,
                    stdout: Vec::new(),
                    stderr: stderr_bytes,
                };
                if is_user_namespace_failure(&output) {
                    return SystemBwrapProbeResult::UserNamespaceUnavailable;
                }
                if is_proc_sys_disconnected(&output) {
                    return SystemBwrapProbeResult::ProcSysDisconnected;
                }
                return SystemBwrapProbeResult::Failed;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return SystemBwrapProbeResult::Failed;
                }
                thread::sleep(SYSTEM_BWRAP_PROBE_POLL_INTERVAL);
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return SystemBwrapProbeResult::Failed;
            }
        }
    }
}

fn read_available_probe_stderr(stderr: &mut ChildStderr, bytes: &mut Vec<u8>) {
    let mut buffer = [0; 4096];
    let mut drained = 0;
    while drained < SYSTEM_BWRAP_PROBE_DRAIN_LIMIT_BYTES {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                drained += read;
                let retained = (SYSTEM_BWRAP_PROBE_STDERR_LIMIT_BYTES - bytes.len()).min(read);
                bytes.extend_from_slice(&buffer[..retained]);
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn resolve_true_command() -> &'static str {
    ["/usr/bin/true", "/bin/true"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .unwrap_or("true")
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

fn is_proc_sys_disconnected(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("/proc/sys")
        && (stderr.contains("Transport endpoint is not connected")
            || stderr.contains("Socket not connected"))
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
    let cwd_is_root = cwd.parent().is_none();
    which::which_in_all(SYSTEM_BWRAP_PROGRAM, Some(search_path), &cwd)
        .ok()?
        .find_map(|path| {
            let path = std::fs::canonicalize(path).ok()?;
            if !cwd_is_root && path.starts_with(&cwd) {
                None
            } else {
                Some(path)
            }
        })
}

#[cfg(test)]
#[path = "bwrap_tests.rs"]
mod tests;
