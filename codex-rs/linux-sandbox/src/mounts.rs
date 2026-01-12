use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) fn apply_read_only_mounts(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Result<()> {
    let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
    let mount_targets = collect_read_only_mount_targets(&writable_roots)?;
    if mount_targets.is_empty() {
        return Ok(());
    }

    if is_running_as_root() {
        unshare_mount_namespace()?;
    } else {
        unshare_user_and_mount_namespaces()?;
        write_user_namespace_maps()?;
    }
    make_mounts_private()?;

    for target in mount_targets {
        bind_mount_read_only(target.as_path())?;
    }

    Ok(())
}

fn collect_read_only_mount_targets(
    writable_roots: &[WritableRoot],
) -> Result<Vec<AbsolutePathBuf>> {
    let mut targets = Vec::new();
    for writable_root in writable_roots {
        ensure_gitdir_is_directory(&writable_root.root)?;
        for ro_subpath in &writable_root.read_only_subpaths {
            if !ro_subpath.as_path().exists() {
                return Err(CodexErr::UnsupportedOperation(format!(
                    "Sandbox expected to protect {path}, but it does not exist. Ensure the repository contains this path or create it before running Codex.",
                    path = ro_subpath.as_path().display()
                )));
            }
            targets.push(ro_subpath.clone());
        }
    }
    Ok(targets)
}

fn ensure_gitdir_is_directory(root: &AbsolutePathBuf) -> Result<()> {
    #[allow(clippy::expect_used)]
    let dot_git = root.join(".git").expect(".git is a valid relative path");
    if dot_git.as_path().is_file() {
        return Err(CodexErr::UnsupportedOperation(format!(
            "Sandbox protection requires .git to be a directory, but {path} is a file. If this is a worktree, protect the real gitdir (from `git rev-parse --git-dir`) and consider making the .git pointer file read-only.",
            path = dot_git.as_path().display()
        )));
    }
    Ok(())
}

fn unshare_mount_namespace() -> Result<()> {
    let result = unsafe { libc::unshare(libc::CLONE_NEWNS) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

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

fn write_user_namespace_maps() -> Result<()> {
    write_proc_file("/proc/self/setgroups", "deny\n")?;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    write_proc_file("/proc/self/uid_map", format!("0 {uid} 1\n"))?;
    write_proc_file("/proc/self/gid_map", format!("0 {gid} 1\n"))?;
    Ok(())
}

fn write_proc_file(path: &str, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}

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
