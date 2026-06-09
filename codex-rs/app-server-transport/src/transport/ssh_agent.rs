use std::io;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use uuid::Uuid;

#[cfg(unix)]
const SSH_AUTH_SOCK_ENV_VAR: &str = "SSH_AUTH_SOCK";
#[cfg(unix)]
const SOCKET_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const SOCKET_DIR_PERMISSION_BITS: u32 = 0o777;
#[cfg(unix)]
const PROXY_LOCK_FILE_MODE: u32 = 0o600;

/// Holds exclusive ownership of the app server's stable SSH agent path.
///
/// The guard must remain alive for the full proxy connection so another proxy
/// cannot replace the agent path while this connection is active.
#[must_use = "the SSH agent proxy guard must be held for the full proxy connection"]
pub struct SshAgentProxyGuard {
    #[cfg(unix)]
    stable_path: PathBuf,
    #[cfg(unix)]
    _lock_file: fs::File,
}

/// Replaces the app server's inherited SSH agent path with a stable symlink.
///
/// This must run before the process starts any threads because it changes the
/// process environment.
pub fn normalize_ssh_auth_sock_before_runtime(
    control_socket_path: &Path,
) -> io::Result<Option<PathBuf>> {
    #[cfg(unix)]
    {
        let Some(agent_socket_path) = std::env::var_os(SSH_AUTH_SOCK_ENV_VAR) else {
            return Ok(None);
        };
        let Some(stable_path) =
            normalize_ssh_auth_sock_from_path(control_socket_path, &agent_socket_path)?
        else {
            return Ok(None);
        };

        // Safety: callers run this before creating any threads or Tokio
        // runtime, so no other thread can concurrently access the environment.
        unsafe {
            std::env::set_var(SSH_AUTH_SOCK_ENV_VAR, &stable_path);
        }
        Ok(Some(stable_path))
    }

    #[cfg(not(unix))]
    {
        let _ = control_socket_path;
        Ok(None)
    }
}

/// Acquires exclusive ownership of the app server's stable SSH agent path.
///
/// When this proxy has a live forwarded agent, the stable path is pointed at
/// that socket. The path is cleared when the guard is dropped. If another
/// proxy already owns the path, this returns [`io::ErrorKind::WouldBlock`].
pub fn acquire_ssh_agent_proxy_guard(
    control_socket_path: &Path,
) -> io::Result<Option<SshAgentProxyGuard>> {
    #[cfg(unix)]
    {
        let agent_socket_path = std::env::var_os(SSH_AUTH_SOCK_ENV_VAR);
        acquire_ssh_agent_proxy_guard_from_path(control_socket_path, agent_socket_path.as_deref())
    }

    #[cfg(not(unix))]
    {
        let _ = control_socket_path;
        Ok(None)
    }
}

#[cfg(unix)]
fn normalize_ssh_auth_sock_from_path(
    control_socket_path: &Path,
    agent_socket_path: &OsStr,
) -> io::Result<Option<PathBuf>> {
    let stable_path = stable_ssh_auth_sock_path(control_socket_path);
    let agent_socket_path = Path::new(agent_socket_path);

    if agent_socket_path == stable_path {
        if let Some(parent) = stable_path.parent() {
            prepare_private_socket_directory(parent)?;
        }
        ensure_symlink_or_missing(&stable_path)?;
        return Ok(Some(stable_path));
    }

    let agent_socket_path = if agent_socket_path.is_absolute() {
        agent_socket_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(agent_socket_path)
    };
    let agent_socket_metadata = match fs::metadata(&agent_socket_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = stable_path.parent() {
                prepare_private_socket_directory(parent)?;
            }
            replace_symlink(&stable_path, &agent_socket_path)?;
            return Ok(Some(stable_path));
        }
        Err(err) => return Err(err),
    };
    if !agent_socket_metadata.file_type().is_socket() {
        return Ok(None);
    }

    if let Some(parent) = stable_path.parent() {
        prepare_private_socket_directory(parent)?;
    }
    replace_symlink(&stable_path, &agent_socket_path)?;
    Ok(Some(stable_path))
}

#[cfg(unix)]
fn stable_ssh_auth_sock_path(control_socket_path: &Path) -> PathBuf {
    append_file_name_suffix(control_socket_path, ".agent")
}

#[cfg(unix)]
fn ssh_agent_proxy_lock_path(control_socket_path: &Path) -> PathBuf {
    append_file_name_suffix(&stable_ssh_auth_sock_path(control_socket_path), ".lock")
}

#[cfg(unix)]
fn append_file_name_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("app-server"));
    file_name.push(suffix);
    path.with_file_name(file_name)
}

#[cfg(unix)]
fn acquire_ssh_agent_proxy_guard_from_path(
    control_socket_path: &Path,
    agent_socket_path: Option<&OsStr>,
) -> io::Result<Option<SshAgentProxyGuard>> {
    let stable_path = stable_ssh_auth_sock_path(control_socket_path);
    if agent_socket_path.is_none() && !path_exists_or_is_symlink(&stable_path)? {
        return Ok(None);
    }

    if let Some(parent) = stable_path.parent() {
        prepare_private_socket_directory(parent)?;
    }
    ensure_symlink_or_missing(&stable_path)?;

    let lock_path = ssh_agent_proxy_lock_path(control_socket_path);
    let mut lock_options = OpenOptions::new();
    lock_options
        .create(true)
        .read(true)
        .write(true)
        .mode(PROXY_LOCK_FILE_MODE);
    let lock_file = lock_options.open(&lock_path)?;
    match lock_file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "another app-server proxy already owns SSH agent forwarding for {}",
                    control_socket_path.display()
                ),
            ));
        }
        Err(err) => return Err(err.into()),
    }

    refresh_proxy_ssh_auth_sock(&stable_path, agent_socket_path)?;
    Ok(Some(SshAgentProxyGuard {
        stable_path,
        _lock_file: lock_file,
    }))
}

#[cfg(unix)]
fn refresh_proxy_ssh_auth_sock(
    stable_path: &Path,
    agent_socket_path: Option<&OsStr>,
) -> io::Result<()> {
    let Some(agent_socket_path) = agent_socket_path else {
        return remove_symlink_if_present(stable_path);
    };
    let agent_socket_path = Path::new(agent_socket_path);
    if agent_socket_path == stable_path {
        return ensure_symlink_or_missing(stable_path);
    }

    let agent_socket_path = if agent_socket_path.is_absolute() {
        agent_socket_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(agent_socket_path)
    };
    let agent_socket_metadata = match fs::metadata(&agent_socket_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return remove_symlink_if_present(stable_path);
        }
        Err(err) => return Err(err),
    };
    if !agent_socket_metadata.file_type().is_socket() {
        return remove_symlink_if_present(stable_path);
    }

    replace_symlink(stable_path, &agent_socket_path)
}

#[cfg(unix)]
fn path_exists_or_is_symlink(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
fn prepare_private_socket_directory(socket_dir: &Path) -> io::Result<()> {
    let mut dir_builder = fs::DirBuilder::new();
    dir_builder.mode(SOCKET_DIR_MODE);
    match dir_builder.create(socket_dir) {
        Ok(()) => return Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err),
    }

    let metadata = fs::symlink_metadata(socket_dir)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "socket directory path exists and is not a directory: {}",
                socket_dir.display()
            ),
        ));
    }

    if metadata.permissions().mode() & SOCKET_DIR_PERMISSION_BITS != SOCKET_DIR_MODE {
        fs::set_permissions(socket_dir, fs::Permissions::from_mode(SOCKET_DIR_MODE))?;
    }
    Ok(())
}

#[cfg(unix)]
fn replace_symlink(stable_path: &Path, target_path: &Path) -> io::Result<()> {
    ensure_symlink_or_missing(stable_path)?;

    let stable_file_name = stable_path
        .file_name()
        .unwrap_or_else(|| OsStr::new("ssh-agent"));
    let temporary_path = stable_path.with_file_name(format!(
        ".{}.{}.tmp",
        stable_file_name.to_string_lossy(),
        Uuid::now_v7()
    ));
    symlink(target_path, &temporary_path)?;
    if let Err(err) = fs::rename(&temporary_path, stable_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(err);
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_symlink_or_missing(stable_path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(stable_path) {
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to replace non-symlink SSH agent path: {}",
                    stable_path.display()
                ),
            ));
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    Ok(())
}

#[cfg(unix)]
fn remove_symlink_if_present(stable_path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(stable_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::remove_file(stable_path),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to remove non-symlink SSH agent path: {}",
                stable_path.display()
            ),
        )),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
impl Drop for SshAgentProxyGuard {
    fn drop(&mut self) {
        let _ = remove_symlink_if_present(&self.stable_path);
    }
}

#[cfg(all(test, unix))]
#[path = "ssh_agent_tests.rs"]
mod tests;
