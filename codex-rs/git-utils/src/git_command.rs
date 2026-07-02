use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

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
        let cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()?.join(cwd)
        };
        let cwd = self.authority.canonical_command_cwd(&cwd)?;
        let mut command = self.command();
        command.arg("-C").arg(cwd);
        Ok(command)
    }

    pub(crate) fn output(&self, mut command: GitCommand) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolate_git_command_environment(&mut command.inner);
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        command.inner.output()
    }

    fn revalidate_active_repository_metadata(&self) -> io::Result<()> {
        self.authority.revalidate_active_repository_metadata()
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
}

pub(crate) fn repository_authority_for_cwd(
    cwd: &Path,
) -> Result<RepositoryAuthority, GitReadError> {
    RepositoryAuthority::discover(cwd)
}

#[cfg(test)]
#[path = "git_command_tests.rs"]
mod tests;
