use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

use crate::errors::GitReadError;
#[cfg(test)]
use crate::git_executable::git_executable_name;
use crate::git_executable::harden_git_launch_environment;
#[cfg(test)]
use crate::git_executable::path_is_untrusted;
#[cfg(test)]
use crate::git_executable::search_directory_is_untrusted;
use crate::git_executable::select_git_executable;
#[cfg(all(test, windows))]
use crate::git_executable::windows_path_requires_fail_closed;
use crate::repository_authority::RepositoryAuthority;
#[cfg(test)]
use crate::repository_authority::parse_marker_path as parse_git_marker_path;
use crate::safe_git::isolate_git_command_environment;

pub(crate) const MAX_INTERNAL_GIT_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// A Git executable outside the repository-controlled roots for one operation.
#[derive(Debug)]
pub(crate) struct GitRunner {
    /// Canonical executable target pinned at selection time. Never execute the
    /// mutable PATH spelling after validation.
    executable: PathBuf,
    #[cfg(any(unix, test))]
    argv0: PathBuf,
    safe_path: std::ffi::OsString,
    authority: RepositoryAuthority,
}

/// A Git command that can only be spawned through [`GitRunner::output`],
/// keeping metadata revalidation and launch hardening at one choke point.
pub(crate) struct GitCommand {
    inner: Command,
}

/// A Tokio Git command that retains the same authority and launch-hardening
/// choke point as [`GitCommand`]. Callers can configure arguments and stdin,
/// but cannot replace the authorized process cwd.
pub(crate) struct GitAsyncCommand {
    inner: tokio::process::Command,
    stdin_configured: bool,
}

impl GitCommand {
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(crate) fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    pub(crate) fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    pub(crate) fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self
    }
}

impl GitAsyncCommand {
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(crate) fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    pub(crate) fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    pub(crate) fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self.stdin_configured = true;
        self
    }
}

impl GitRunner {
    pub(crate) fn for_cwd(cwd: &Path) -> Result<Self, GitReadError> {
        let authority = repository_authority_for_cwd(cwd)?;
        let search_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        Self::from_search_path(authority, &search_path)
    }

    pub(crate) fn for_cwd_io(cwd: &Path) -> io::Result<Self> {
        Self::for_cwd(cwd).map_err(GitReadError::into_io_error)
    }

    pub(crate) fn command(&self) -> GitCommand {
        let mut command = Command::new(&self.executable);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            command.arg0(&self.argv0);
        }
        if let Some(parent) = self.executable.parent() {
            command.current_dir(parent);
        }
        harden_git_launch_environment(&mut command, &self.safe_path);
        GitCommand { inner: command }
    }

    pub(crate) fn command_for_cwd(&self, cwd: &Path) -> io::Result<GitCommand> {
        let cwd = self.canonical_command_cwd(cwd)?;
        let mut command = self.command();
        command.arg("-C").arg(cwd);
        Ok(command)
    }

    pub(crate) fn async_command_for_cwd(&self, cwd: &Path) -> io::Result<GitAsyncCommand> {
        let cwd = self.canonical_command_cwd(cwd)?;
        let mut command = tokio::process::Command::from(self.command().inner);
        command.arg("-C").arg(cwd);
        Ok(GitAsyncCommand {
            inner: command,
            stdin_configured: false,
        })
    }

    fn canonical_command_cwd(&self, cwd: &Path) -> io::Result<PathBuf> {
        let cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()?.join(cwd)
        };
        self.authority.canonical_command_cwd(&cwd)
    }

    pub(crate) fn ensure_config_source_is_not_worktree_controlled(
        &self,
        path: &Path,
        description: &str,
    ) -> io::Result<()> {
        self.authority
            .ensure_config_source_is_not_worktree_controlled(path, description)
    }

    pub(crate) fn active_worktree_root(&self) -> Option<&Path> {
        self.authority.active_worktree_root()
    }

    pub(crate) fn output(&self, mut command: GitCommand) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(&mut command.inner)?;
        command.inner.output()
    }

    fn revalidate_active_repository_metadata(&self) -> io::Result<()> {
        self.authority.revalidate_active_repository_metadata()
    }

    fn prepare_command_for_launch(&self, command: &mut Command) -> io::Result<()> {
        self.revalidate_active_repository_metadata()?;
        isolate_git_command_environment(command);
        command.envs(crate::local_only_git_env());
        harden_git_launch_environment(command, &self.safe_path);
        Ok(())
    }

    /// Spawn a configured Tokio command after applying the final
    /// repository-selector environment lock.
    #[cfg(all(test, unix))]
    pub(crate) async fn output_async(
        &self,
        mut command: GitAsyncCommand,
    ) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(command.inner.as_std_mut())?;
        command.inner.kill_on_drop(true);
        command.inner.output().await
    }

    pub(crate) async fn output_async_bounded(
        &self,
        mut command: GitAsyncCommand,
        max_bytes_per_stream: usize,
    ) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(command.inner.as_std_mut())?;
        command.inner.kill_on_drop(true);
        if !command.stdin_configured {
            command.inner.stdin(Stdio::null());
        }
        command.inner.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.inner.spawn()?;
        // No caller can obtain the pipe writer through this opaque command.
        // Close it immediately so a child configured with `Stdio::piped()`
        // observes EOF instead of waiting forever for unreachable input.
        drop(child.stdin.take());
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing bounded Git stdout pipe"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing bounded Git stderr pipe"))?;
        let output = tokio::try_join!(
            read_bounded_output(stdout, max_bytes_per_stream),
            read_bounded_output(stderr, max_bytes_per_stream)
        );
        let (stdout, stderr) = match output {
            Ok(output) => output,
            Err(error) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(error);
            }
        };
        let status = child.wait().await?;
        Ok(std::process::Output {
            status,
            stdout,
            stderr,
        })
    }

    fn from_search_path(
        authority: RepositoryAuthority,
        search_path: &OsStr,
    ) -> Result<Self, GitReadError> {
        authority.ensure_primary_authority()?;
        let selected = select_git_executable(&authority, search_path)?;
        Ok(Self {
            executable: selected.executable,
            #[cfg(any(unix, test))]
            argv0: selected.argv0,
            safe_path: selected.safe_path,
            authority,
        })
    }

    #[cfg(all(test, unix))]
    pub(crate) fn from_executable_for_test(
        cwd: &Path,
        executable: PathBuf,
    ) -> Result<Self, GitReadError> {
        let authority = repository_authority_for_cwd(cwd)?;
        let safe_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        Ok(Self {
            argv0: executable.clone(),
            executable,
            safe_path,
            authority,
        })
    }
}

async fn read_bounded_output(
    mut reader: impl AsyncRead + Unpin,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Git output exceeded the {max_bytes}-byte stream limit"),
            ));
        }
        output.extend_from_slice(&chunk[..read]);
    }
}

pub(crate) fn repository_authority_for_cwd(
    cwd: &Path,
) -> Result<RepositoryAuthority, GitReadError> {
    RepositoryAuthority::discover(cwd)
}

#[cfg(test)]
#[path = "git_command_tests.rs"]
mod tests;
