use std::io;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
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
            refresh_ssh_auth_sock_from_path(control_socket_path, &agent_socket_path)?
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

/// Points the app server's stable SSH agent symlink at this proxy process's
/// current forwarded agent socket.
pub fn refresh_ssh_auth_sock_for_proxy(control_socket_path: &Path) -> io::Result<Option<PathBuf>> {
    #[cfg(unix)]
    {
        let Some(agent_socket_path) = std::env::var_os(SSH_AUTH_SOCK_ENV_VAR) else {
            return Ok(None);
        };
        refresh_ssh_auth_sock_from_path(control_socket_path, &agent_socket_path)
    }

    #[cfg(not(unix))]
    {
        let _ = control_socket_path;
        Ok(None)
    }
}

#[cfg(unix)]
fn refresh_ssh_auth_sock_from_path(
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
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
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
    control_socket_path.with_extension("agent")
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

#[cfg(all(test, unix))]
#[path = "ssh_agent_tests.rs"]
mod tests;
