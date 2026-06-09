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
use std::os::unix::fs::DirBuilderExt;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use uuid::Uuid;

#[cfg(unix)]
const SSH_AUTH_SOCK_ENV_VAR: &str = "SSH_AUTH_SOCK";
#[cfg(unix)]
const SOCKET_DIR_MODE: u32 = 0o700;

/// A stable SSH agent path and its initial target, prepared before runtime
/// startup and published only after the app server owns its control socket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedSshAgentForwarding {
    #[cfg(unix)]
    stable_path: PathBuf,
    #[cfg(unix)]
    agent_socket_path: PathBuf,
}

impl PreparedSshAgentForwarding {
    /// Publishes the initial agent target at the stable path.
    pub fn publish(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            if let Some(parent) = self.stable_path.parent() {
                ensure_socket_directory_exists(parent)?;
            }
            replace_symlink(&self.stable_path, &self.agent_socket_path)
        }

        #[cfg(not(unix))]
        {
            Ok(())
        }
    }
}

/// Replaces the app server's inherited SSH agent environment value with a
/// stable path and returns the initial symlink update for later publication.
///
/// This must run before the process starts any threads because it changes the
/// process environment. The caller must publish the returned update only after
/// the app server has successfully bound its control socket.
pub fn normalize_ssh_auth_sock_before_runtime(
    control_socket_path: &Path,
) -> io::Result<Option<PreparedSshAgentForwarding>> {
    #[cfg(unix)]
    {
        let Some(agent_socket_path) = std::env::var_os(SSH_AUTH_SOCK_ENV_VAR) else {
            return Ok(None);
        };
        let Some(prepared) =
            prepare_ssh_auth_sock_from_path(control_socket_path, &agent_socket_path)?
        else {
            return Ok(None);
        };

        // Safety: callers run this before creating any threads or Tokio
        // runtime, so no other thread can concurrently access the environment.
        unsafe {
            std::env::set_var(SSH_AUTH_SOCK_ENV_VAR, &prepared.stable_path);
        }
        Ok(Some(prepared))
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
fn prepare_ssh_auth_sock_from_path(
    control_socket_path: &Path,
    agent_socket_path: &OsStr,
) -> io::Result<Option<PreparedSshAgentForwarding>> {
    let stable_path = stable_ssh_auth_sock_path(control_socket_path);
    let agent_socket_path = absolute_agent_socket_path(agent_socket_path)?;
    if agent_socket_path == stable_path {
        ensure_symlink_or_missing(&stable_path)?;
        return Ok(None);
    }
    if !agent_socket_matches_policy(
        &agent_socket_path,
        MissingAgentSocket::CreateDanglingSymlink,
    )? {
        return Ok(None);
    }
    ensure_symlink_or_missing(&stable_path)?;
    Ok(Some(PreparedSshAgentForwarding {
        stable_path,
        agent_socket_path,
    }))
}

#[cfg(unix)]
fn refresh_ssh_auth_sock_from_path(
    control_socket_path: &Path,
    agent_socket_path: &OsStr,
) -> io::Result<Option<PathBuf>> {
    let stable_path = stable_ssh_auth_sock_path(control_socket_path);
    let agent_socket_path = absolute_agent_socket_path(agent_socket_path)?;
    if agent_socket_path == stable_path {
        ensure_symlink_or_missing(&stable_path)?;
        return Ok(Some(stable_path));
    }
    if !agent_socket_matches_policy(&agent_socket_path, MissingAgentSocket::Ignore)? {
        return Ok(None);
    }
    PreparedSshAgentForwarding {
        stable_path: stable_path.clone(),
        agent_socket_path,
    }
    .publish()?;
    Ok(Some(stable_path))
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum MissingAgentSocket {
    CreateDanglingSymlink,
    Ignore,
}

#[cfg(unix)]
fn absolute_agent_socket_path(agent_socket_path: &OsStr) -> io::Result<PathBuf> {
    let agent_socket_path = Path::new(agent_socket_path);
    if agent_socket_path.is_absolute() {
        Ok(agent_socket_path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(agent_socket_path))
    }
}

#[cfg(unix)]
fn agent_socket_matches_policy(
    agent_socket_path: &Path,
    missing_agent_socket: MissingAgentSocket,
) -> io::Result<bool> {
    let agent_socket_metadata = match fs::metadata(agent_socket_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => match missing_agent_socket {
            MissingAgentSocket::CreateDanglingSymlink => return Ok(true),
            MissingAgentSocket::Ignore => return Ok(false),
        },
        Err(err) => return Err(err),
    };
    Ok(agent_socket_metadata.file_type().is_socket())
}

#[cfg(unix)]
fn stable_ssh_auth_sock_path(control_socket_path: &Path) -> PathBuf {
    append_file_name_suffix(control_socket_path, ".agent")
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
fn ensure_socket_directory_exists(socket_dir: &Path) -> io::Result<()> {
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
