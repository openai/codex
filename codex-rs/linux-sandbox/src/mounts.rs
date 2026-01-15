#![allow(dead_code)]

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Apply read-only bind mounts for protected subpaths before Landlock.
///
/// This unshares mount namespaces (and user namespaces for non-root) so the
/// read-only remounts do not affect the host, then bind-mounts each protected
/// target onto itself and remounts it read-only.
pub(crate) fn apply_read_only_mounts(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Result<()> {
    let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
    let mount_targets = collect_read_only_mount_targets(&writable_roots)?;
    if mount_targets.is_empty() {
        return Ok(());
    }

    // Root can unshare the mount namespace directly; non-root needs a user
    // namespace to gain capabilities for remounting.
    if is_running_as_root() {
        unshare_mount_namespace()?;
    } else {
        let original_euid = unsafe { libc::geteuid() };
        let original_egid = unsafe { libc::getegid() };
        unshare_user_and_mount_namespaces()?;
        write_user_namespace_maps(original_euid, original_egid)?;
    }
    make_mounts_private()?;

    for target in mount_targets {
        // Bind and remount read-only works for both files and directories.
        bind_mount_read_only(target.as_path())?;
    }

    // Drop ambient capabilities acquired from the user namespace so the
    // sandboxed command cannot remount or create new bind mounts.
    if !is_running_as_root() {
        drop_caps()?;
    }

    Ok(())
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
    let result = unsafe { libc::unshare(libc::CLONE_NEWNS) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Unshare user + mount namespaces so the process can remount read-only without privileges.
fn unshare_user_and_mount_namespaces() -> Result<()> {
    let result = unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn is_running_as_root() -> bool {
    unsafe { libc::geteuid() == 0 }
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
    write_proc_file("/proc/self/setgroups", "deny\n")?;

    write_proc_file("/proc/self/uid_map", format!("0 {uid} 1\n"))?;
    write_proc_file("/proc/self/gid_map", format!("0 {gid} 1\n"))?;
    Ok(())
}

/// Drop all capabilities in the current user namespace.
fn drop_caps() -> Result<()> {
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
}
