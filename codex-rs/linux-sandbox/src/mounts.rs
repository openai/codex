use std::error::Error;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

/// Apply read-only bind mounts for protected subpaths before Landlock.
///
/// Strategy overview:
/// - Root path: try to unshare the mount namespace first. If that is denied
///   (EPERM/PermissionDenied), fall back to unsharing a user namespace plus a
///   mount namespace to gain CAP_SYS_ADMIN inside the userns (bwrap-style).
/// - Non-root path: unshare user+mount namespaces up front to gain the
///   capabilities needed for remounting.
/// - If namespace or mount setup is denied in either path, skip mount-based
///   protections and continue with Landlock-only sandboxing, emitting a
///   warning log.
///
/// Once in the namespace(s), we make mounts private, bind each protected
/// target onto itself, remount read-only, and drop any userns-granted caps.
pub(crate) fn apply_read_only_mounts(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Result<()> {
    let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
    let mount_targets = collect_read_only_mount_targets(&writable_roots)?;
    if mount_targets.is_empty() {
        return Ok(());
    }

    // Root can unshare the mount namespace directly; non-root needs a user
    // namespace to gain capabilities for remounting.
    let running_as_root = is_running_as_root();
    let mut used_userns = false;
    if running_as_root {
        match unshare_mount_namespace() {
            Ok(()) => {}
            Err(err) if is_permission_denied(&err) => {
                // Root fallback: try userns+mountns to acquire mount powers.
                let original_euid = unsafe { libc::geteuid() };
                let original_egid = unsafe { libc::getegid() };
                match unshare_user_and_mount_namespaces() {
                    Ok(()) => {
                        if let Err(err) = write_user_namespace_maps(original_euid, original_egid) {
                            if is_permission_denied(&err) {
                                return Err(err);
                            }
                            return Err(err);
                        }
                        used_userns = true;
                    }
                    Err(err) if is_permission_denied(&err) => {
                        // No namespaces available; continue with Landlock-only.
                        log_namespace_fallback(&err);
                        return Ok(());
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(err) => return Err(err),
        }
    } else {
        let original_euid = unsafe { libc::geteuid() };
        let original_egid = unsafe { libc::getegid() };
        match unshare_user_and_mount_namespaces() {
            Ok(()) => {
                if let Err(err) = write_user_namespace_maps(original_euid, original_egid) {
                    if is_permission_denied(&err) {
                        return Err(err);
                    }
                    return Err(err);
                }
                used_userns = true;
            }
            Err(err) if is_permission_denied(&err) => {
                // No namespaces available; continue with Landlock-only.
                log_namespace_fallback(&err);
                return Ok(());
            }
            Err(err) => return Err(err),
        }
    }
    let should_drop_caps = used_userns || running_as_root;
    match apply_read_only_bind_mounts(&mount_targets, should_drop_caps)? {
        BindMountAttempt::Applied => Ok(()),
        BindMountAttempt::PermissionDenied(err) => {
            log_namespace_fallback(&err);
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BindMountProbeStatus {
    Supported,
    Unsupported { reason: String },
}

#[derive(Debug)]
enum BindMountAttempt {
    Applied,
    PermissionDenied(CodexErr),
}

/// Returns whether this host supports the namespace + mount operations needed
/// to apply bind-mount-based read-only protections.
pub(crate) fn probe_bind_mounts() -> BindMountProbeStatus {
    let probe_dir = match create_probe_dir() {
        Ok(dir) => dir,
        Err(err) => {
            return BindMountProbeStatus::Unsupported {
                reason: format!("create probe dir failed: {err}"),
            };
        }
    };
    let protected = probe_dir.join("protected");
    let status = loop {
        if let Err(err) = std::fs::create_dir_all(&protected) {
            break BindMountProbeStatus::Unsupported {
                reason: format!("create probe path failed: {err}"),
            };
        }
        let protected_abs = match AbsolutePathBuf::try_from(protected.as_path()) {
            Ok(path) => path,
            Err(err) => {
                break BindMountProbeStatus::Unsupported {
                    reason: format!("resolve probe path failed: {err}"),
                };
            }
        };

        let original_euid = unsafe { libc::geteuid() };
        let original_egid = unsafe { libc::getegid() };

        if let Err(err) = unshare_user_and_mount_namespaces() {
            break probe_unsupported("unshare user+mount namespaces", err);
        }
        if let Err(err) = write_user_namespace_maps(original_euid, original_egid) {
            break probe_unsupported("write user namespace maps", err);
        }
        let targets = [protected_abs];
        match apply_read_only_bind_mounts(&targets, true) {
            Ok(BindMountAttempt::Applied) => {}
            Ok(BindMountAttempt::PermissionDenied(err)) => {
                break probe_unsupported("apply bind mounts", err);
            }
            Err(err) => {
                break probe_unsupported("apply bind mounts", err);
            }
        }

        break BindMountProbeStatus::Supported;
    };

    let _ = std::fs::remove_dir_all(&probe_dir);
    status
}

fn probe_unsupported(step: &str, err: CodexErr) -> BindMountProbeStatus {
    let reason = if is_permission_denied(&err) {
        format!("{step} permission denied: {err}")
    } else {
        format!("{step} failed: {err}")
    };
    BindMountProbeStatus::Unsupported { reason }
}

fn create_probe_dir() -> Result<PathBuf> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut dir = std::env::temp_dir();
    dir.push(format!("codex-linux-sandbox-probe-{pid}-{nanos}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn apply_read_only_bind_mounts(
    mount_targets: &[AbsolutePathBuf],
    should_drop_caps: bool,
) -> Result<BindMountAttempt> {
    if let Err(err) = make_mounts_private() {
        if is_permission_denied(&err) {
            if should_drop_caps {
                drop_caps()?;
            }
            return Ok(BindMountAttempt::PermissionDenied(err));
        }
        return Err(err);
    }

    for target in mount_targets {
        // Bind and remount read-only works for both files and directories.
        if let Err(err) = bind_mount_read_only(target.as_path()) {
            if is_permission_denied(&err) {
                if should_drop_caps {
                    drop_caps()?;
                }
                return Ok(BindMountAttempt::PermissionDenied(err));
            }
            return Err(err);
        }
    }

    // Drop ambient capabilities acquired from the user namespace so the
    // sandboxed command cannot remount or create new bind mounts.
    if should_drop_caps {
        drop_caps()?;
    }

    Ok(BindMountAttempt::Applied)
}

/// Collect read-only mount targets, resolving worktree `.git` pointer files.
fn collect_read_only_mount_targets(
    writable_roots: &[WritableRoot],
) -> Result<Vec<AbsolutePathBuf>> {
    let mut targets = Vec::new();
    for writable_root in writable_roots {
        for ro_subpath in &writable_root.read_only_subpaths {
            // The policy expects these paths to exist; surface actionable errors
            // rather than silently skipping protections.
            if !ro_subpath.as_path().exists() {
                return Err(CodexErr::UnsupportedOperation(format!(
                    "Sandbox expected to protect {path}, but it does not exist. Ensure the repository contains this path or create it before running Codex.",
                    path = ro_subpath.as_path().display()
                )));
            }
            targets.push(ro_subpath.clone());
            // Worktrees and submodules store `.git` as a pointer file; add the
            // referenced gitdir as an extra read-only target.
            if is_git_pointer_file(ro_subpath) {
                let gitdir = resolve_gitdir_from_file(ro_subpath)?;
                if !targets
                    .iter()
                    .any(|target| target.as_path() == gitdir.as_path())
                {
                    targets.push(gitdir);
                }
            }
        }
    }
    Ok(targets)
}

/// Detect a `.git` pointer file used by worktrees and submodules.
fn is_git_pointer_file(path: &AbsolutePathBuf) -> bool {
    path.as_path().is_file() && path.as_path().file_name() == Some(std::ffi::OsStr::new(".git"))
}

/// Resolve a worktree `.git` pointer file to its gitdir path.
fn resolve_gitdir_from_file(dot_git: &AbsolutePathBuf) -> Result<AbsolutePathBuf> {
    let contents = std::fs::read_to_string(dot_git.as_path()).map_err(CodexErr::from)?;
    let trimmed = contents.trim();
    let (_, gitdir_raw) = trimmed.split_once(':').ok_or_else(|| {
        CodexErr::UnsupportedOperation(format!(
            "Expected {path} to contain a gitdir pointer, but it did not match `gitdir: <path>`.",
            path = dot_git.as_path().display()
        ))
    })?;
    // `gitdir: <path>` may be relative to the directory containing `.git`.
    let gitdir_raw = gitdir_raw.trim();
    if gitdir_raw.is_empty() {
        return Err(CodexErr::UnsupportedOperation(format!(
            "Expected {path} to contain a gitdir pointer, but it was empty.",
            path = dot_git.as_path().display()
        )));
    }
    let base = dot_git.as_path().parent().ok_or_else(|| {
        CodexErr::UnsupportedOperation(format!(
            "Unable to resolve parent directory for {path}.",
            path = dot_git.as_path().display()
        ))
    })?;
    let gitdir_path = AbsolutePathBuf::resolve_path_against_base(gitdir_raw, base)?;
    if !gitdir_path.as_path().exists() {
        return Err(CodexErr::UnsupportedOperation(format!(
            "Resolved gitdir path {path} does not exist.",
            path = gitdir_path.as_path().display()
        )));
    }
    Ok(gitdir_path)
}

/// Unshare the mount namespace so mount changes are isolated to the sandboxed process.
fn unshare_mount_namespace() -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_UNSHARE_PERMISSION_DENIED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM).into());
        }
    }
    let result = unsafe { libc::unshare(libc::CLONE_NEWNS) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Unshare user + mount namespaces so the process can remount read-only without privileges.
fn unshare_user_and_mount_namespaces() -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_UNSHARE_USERNS_SUCCESS.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        if FORCE_UNSHARE_PERMISSION_DENIED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM).into());
        }
    }
    let result = unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn is_running_as_root() -> bool {
    #[cfg(test)]
    {
        if FORCE_RUNNING_AS_ROOT.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }
    }
    unsafe { libc::geteuid() == 0 }
}

fn is_permission_denied(err: &CodexErr) -> bool {
    if let CodexErr::Io(io_error) = err
        && (io_error.kind() == std::io::ErrorKind::PermissionDenied
            || io_error.raw_os_error() == Some(libc::EPERM))
    {
        return true;
    }
    let mut current: Option<&(dyn Error + 'static)> = Some(err);
    while let Some(error) = current {
        if let Some(io_error) = error.downcast_ref::<std::io::Error>()
            && (io_error.kind() == std::io::ErrorKind::PermissionDenied
                || io_error.raw_os_error() == Some(libc::EPERM))
        {
            return true;
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
static FORCE_UNSHARE_PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_UNSHARE_USERNS_SUCCESS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_RUNNING_AS_ROOT: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_WRITE_USER_NAMESPACE_MAPS_PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_MAKE_MOUNTS_PRIVATE_SUCCESS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_MAKE_MOUNTS_PRIVATE_PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_BIND_MOUNT_SUCCESS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_BIND_MOUNT_PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_DROP_CAPS_SUCCESS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCE_FLAGS_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
// Tests mutate global FORCE_* flags; use force_flags_guard() to serialize and reset
// state to avoid cross-test contamination when tests run in parallel. An alternative
// would be integration tests that spawn separate codex-linux-sandbox processes and
// inject failures via per-process flags/env vars, avoiding global state entirely.
fn reset_force_flags() {
    FORCE_UNSHARE_PERMISSION_DENIED.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_UNSHARE_USERNS_SUCCESS.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_RUNNING_AS_ROOT.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_WRITE_USER_NAMESPACE_MAPS_PERMISSION_DENIED
        .store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_MAKE_MOUNTS_PRIVATE_SUCCESS.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_MAKE_MOUNTS_PRIVATE_PERMISSION_DENIED.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_BIND_MOUNT_SUCCESS.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_BIND_MOUNT_PERMISSION_DENIED.store(false, std::sync::atomic::Ordering::SeqCst);
    FORCE_DROP_CAPS_SUCCESS.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
struct ForceFlagsGuard(std::sync::MutexGuard<'static, ()>);

#[cfg(test)]
impl Drop for ForceFlagsGuard {
    fn drop(&mut self) {
        reset_force_flags();
    }
}

#[cfg(test)]
fn force_flags_guard() -> ForceFlagsGuard {
    let guard = FORCE_FLAGS_LOCK.lock().expect("force flags lock");
    reset_force_flags();
    ForceFlagsGuard(guard)
}

fn log_namespace_fallback(err: &CodexErr) {
    warn!(
        "codex-linux-sandbox: falling back to Landlock-only sandboxing because namespaces are unavailable (seccomp/caps likely): {err}"
    );
}

#[repr(C)]
struct CapUserHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
struct CapUserData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

/// Map the provided uid/gid to root inside the user namespace.
fn write_user_namespace_maps(uid: libc::uid_t, gid: libc::gid_t) -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        if FORCE_WRITE_USER_NAMESPACE_MAPS_PERMISSION_DENIED
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM).into());
        }
    }
    write_proc_file("/proc/self/setgroups", "deny\n")?;

    write_proc_file("/proc/self/uid_map", format!("0 {uid} 1\n"))?;
    write_proc_file("/proc/self/gid_map", format!("0 {gid} 1\n"))?;
    Ok(())
}

/// Drop all capabilities in the current user namespace.
fn drop_caps() -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_DROP_CAPS_SUCCESS.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
    }
    let mut header = CapUserHeader {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let data = [
        CapUserData {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        },
        CapUserData {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        },
    ];

    // Use syscall directly to avoid libc capability symbols that are missing on musl.
    let result = unsafe { libc::syscall(libc::SYS_capset, &mut header, data.as_ptr()) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Write a small procfs file, returning a sandbox error on failure.
fn write_proc_file(path: &str, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}

/// Ensure mounts are private so remounting does not propagate outside the namespace.
fn make_mounts_private() -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_MAKE_MOUNTS_PRIVATE_SUCCESS.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        if FORCE_MAKE_MOUNTS_PRIVATE_PERMISSION_DENIED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM).into());
        }
    }
    let root = CString::new("/").map_err(|_| {
        CodexErr::UnsupportedOperation("Sandbox mount path contains NUL byte: /".to_string())
    })?;
    let result = unsafe {
        libc::mount(
            std::ptr::null(),
            root.as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Bind-mount a path onto itself and remount read-only.
fn bind_mount_read_only(path: &Path) -> Result<()> {
    #[cfg(test)]
    {
        if FORCE_BIND_MOUNT_SUCCESS.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        if FORCE_BIND_MOUNT_PERMISSION_DENIED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM).into());
        }
    }
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        CodexErr::UnsupportedOperation(format!(
            "Sandbox mount path contains NUL byte: {path}",
            path = path.display()
        ))
    })?;

    let bind_result = unsafe {
        libc::mount(
            c_path.as_ptr(),
            c_path.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };
    if bind_result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let remount_result = unsafe {
        libc::mount(
            c_path.as_ptr(),
            c_path.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
            std::ptr::null(),
        )
    };
    if remount_result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn collect_read_only_mount_targets_errors_on_missing_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let missing = AbsolutePathBuf::try_from(tempdir.path().join("missing").as_path())
            .expect("missing path");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let writable_root = WritableRoot {
            root,
            read_only_subpaths: vec![missing],
        };

        let err = collect_read_only_mount_targets(&[writable_root])
            .expect_err("expected missing path error");
        let message = match err {
            CodexErr::UnsupportedOperation(message) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert_eq!(
            message,
            format!(
                "Sandbox expected to protect {path}, but it does not exist. Ensure the repository contains this path or create it before running Codex.",
                path = tempdir.path().join("missing").display()
            )
        );
    }

    #[test]
    fn collect_read_only_mount_targets_adds_gitdir_for_pointer_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let gitdir = tempdir.path().join("actual-gitdir");
        std::fs::create_dir_all(&gitdir).expect("create gitdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::write(&dot_git, format!("gitdir: {}\n", gitdir.display()))
            .expect("write gitdir pointer");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let writable_root = WritableRoot {
            root,
            read_only_subpaths: vec![
                AbsolutePathBuf::try_from(dot_git.as_path()).expect("dot git"),
            ],
        };

        let targets = collect_read_only_mount_targets(&[writable_root]).expect("collect targets");
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].as_path(), dot_git.as_path());
        assert_eq!(targets[1].as_path(), gitdir.as_path());
    }

    #[test]
    fn collect_read_only_mount_targets_errors_on_invalid_gitdir_pointer() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::write(&dot_git, "not-a-pointer\n").expect("write invalid pointer");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let writable_root = WritableRoot {
            root,
            read_only_subpaths: vec![
                AbsolutePathBuf::try_from(dot_git.as_path()).expect("dot git"),
            ],
        };

        let err = collect_read_only_mount_targets(&[writable_root])
            .expect_err("expected invalid pointer error");
        let message = match err {
            CodexErr::UnsupportedOperation(message) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert_eq!(
            message,
            format!(
                "Expected {path} to contain a gitdir pointer, but it did not match `gitdir: <path>`.",
                path = dot_git.display()
            )
        );
    }

    #[test]
    fn is_permission_denied_detects_permission_denied_kind() {
        let err: CodexErr =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope").into();
        assert!(is_permission_denied(&err));
    }

    #[test]
    fn is_permission_denied_detects_eperm_os_error() {
        let err: CodexErr = std::io::Error::from_raw_os_error(libc::EPERM).into();
        assert!(is_permission_denied(&err));
    }

    #[test]
    fn is_permission_denied_returns_false_for_other_errors() {
        let err = CodexErr::UnsupportedOperation("nope".to_string());
        assert!(!is_permission_denied(&err));
    }

    #[test]
    fn probe_bind_mounts_reports_supported_when_forced_success() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_USERNS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_MAKE_MOUNTS_PRIVATE_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_BIND_MOUNT_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_DROP_CAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);

        let status = probe_bind_mounts();

        assert_eq!(status, BindMountProbeStatus::Supported);
    }

    #[test]
    fn probe_bind_mounts_reports_unsupported_on_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_PERMISSION_DENIED.store(true, std::sync::atomic::Ordering::SeqCst);

        let status = probe_bind_mounts();

        let BindMountProbeStatus::Unsupported { reason } = status else {
            panic!("expected probe to report unsupported");
        };
        assert!(
            reason.contains("permission denied"),
            "expected permission denied reason, got: {reason}"
        );
    }

    #[test]
    fn apply_read_only_mounts_falls_back_on_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_PERMISSION_DENIED.store(true, std::sync::atomic::Ordering::SeqCst);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::create_dir_all(&dot_git).expect("create .git");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let result = apply_read_only_mounts(&sandbox_policy, tempdir.path());

        assert!(
            result.is_ok(),
            "expected fallback to Landlock-only when unshare is denied"
        );
    }

    #[test]
    fn apply_read_only_mounts_root_falls_back_on_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_RUNNING_AS_ROOT.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_UNSHARE_PERMISSION_DENIED.store(true, std::sync::atomic::Ordering::SeqCst);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::create_dir_all(&dot_git).expect("create .git");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let result = apply_read_only_mounts(&sandbox_policy, tempdir.path());

        assert!(
            result.is_ok(),
            "expected root path to fall back to Landlock-only when mountns unshare is denied"
        );
    }

    #[test]
    fn apply_read_only_mounts_falls_back_on_user_namespace_map_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_USERNS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_WRITE_USER_NAMESPACE_MAPS_PERMISSION_DENIED
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::create_dir_all(&dot_git).expect("create .git");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let err = apply_read_only_mounts(&sandbox_policy, tempdir.path())
            .expect_err("expected permission denied error");

        assert!(is_permission_denied(&err));
    }

    #[test]
    fn apply_read_only_mounts_falls_back_on_make_mounts_private_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_USERNS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_MAKE_MOUNTS_PRIVATE_PERMISSION_DENIED
            .store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_DROP_CAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::create_dir_all(&dot_git).expect("create .git");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let result = apply_read_only_mounts(&sandbox_policy, tempdir.path());

        assert!(
            result.is_ok(),
            "expected Landlock-only fallback when making mounts private is denied"
        );
    }

    #[test]
    fn apply_read_only_mounts_falls_back_on_bind_mount_permission_denied() {
        let _guard = force_flags_guard();
        FORCE_UNSHARE_USERNS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_WRITE_USER_NAMESPACE_MAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_MAKE_MOUNTS_PRIVATE_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_BIND_MOUNT_PERMISSION_DENIED.store(true, std::sync::atomic::Ordering::SeqCst);
        FORCE_DROP_CAPS_SUCCESS.store(true, std::sync::atomic::Ordering::SeqCst);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dot_git = tempdir.path().join(".git");
        std::fs::create_dir_all(&dot_git).expect("create .git");
        let root = AbsolutePathBuf::try_from(tempdir.path()).expect("root");
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let result = apply_read_only_mounts(&sandbox_policy, tempdir.path());

        assert!(
            result.is_ok(),
            "expected Landlock-only fallback when bind mount is denied"
        );
    }
}
