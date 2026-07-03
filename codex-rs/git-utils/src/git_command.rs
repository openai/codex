use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use crate::errors::GitReadError;
use crate::git_config_environment::GitConfigEnvironmentSnapshot;
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
    config_environment: GitConfigEnvironmentSnapshot,
}

/// A Git command that can only be spawned through [`GitRunner::output`],
/// keeping metadata revalidation and launch hardening at one choke point.
pub(crate) struct GitCommand {
    inner: Command,
}

/// App-owned common repository metadata for one final three-way apply.
///
/// The real per-worktree Git directory remains selected for HEAD and index
/// state, while this directory replaces every common config and attribute
/// source that could define a repository-selected executable helper.
pub(crate) struct IsolatedGitCommonDir {
    root: tempfile::TempDir,
}

impl IsolatedGitCommonDir {
    pub(crate) fn config_path(&self) -> PathBuf {
        self.root.path().join("config")
    }

    fn system_config_path(&self) -> PathBuf {
        self.root.path().join("system.gitconfig")
    }

    fn global_config_path(&self) -> PathBuf {
        self.root.path().join("global.gitconfig")
    }

    fn home_path(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn xdg_config_home(&self) -> PathBuf {
        self.root.path().join("xdg")
    }

    fn validate(&self) -> io::Result<()> {
        for path in [
            self.config_path(),
            self.system_config_path(),
            self.global_config_path(),
            self.root.path().join("info/attributes"),
        ] {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("isolated Git metadata file changed at {}", path.display()),
                ));
            }
        }
        for path in [
            self.root.path().join("objects"),
            self.root.path().join("refs"),
            self.root.path().join("info"),
            self.home_path(),
            self.xdg_config_home(),
        ] {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "isolated Git metadata directory changed at {}",
                        path.display()
                    ),
                ));
            }
        }
        Ok(())
    }
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

impl GitRunner {
    pub(crate) fn for_cwd(cwd: &Path) -> Result<Self, GitReadError> {
        #[cfg(test)]
        GIT_RUNNER_CONSTRUCTION_COUNT.with(|count| count.set(count.get() + 1));
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
        self.config_environment.apply_to(&mut command);
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

    pub(crate) fn ensure_config_source_is_not_worktree_controlled(
        &self,
        path: &Path,
        description: &str,
    ) -> io::Result<()> {
        self.authority
            .ensure_config_source_is_not_worktree_controlled(path, description)
    }

    pub(crate) fn ensure_active_worktree_root(&self, root: &Path) -> io::Result<()> {
        self.authority.ensure_active_worktree_root(root)
    }

    pub(crate) fn ensure_repository_root_route(&self, root: &Path) -> io::Result<()> {
        self.authority.ensure_repository_root_route(root)
    }

    pub(crate) fn config_environment_value(&self, name: &str) -> Option<&OsStr> {
        self.config_environment.value(name)
    }

    pub(crate) fn create_isolated_common_dir(&self) -> io::Result<IsolatedGitCommonDir> {
        let root = tempfile::tempdir()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                root.path(),
                "owned isolated Git common directory",
            )?;
        for path in ["objects", "refs", "info", "home", "xdg"] {
            std::fs::create_dir_all(root.path().join(path))?;
        }
        for path in [
            "config",
            "system.gitconfig",
            "global.gitconfig",
            "info/attributes",
        ] {
            std::fs::write(root.path().join(path), [])?;
        }
        let isolated = IsolatedGitCommonDir { root };
        isolated.validate()?;
        Ok(isolated)
    }

    pub(crate) fn output(&self, mut command: GitCommand) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolate_git_command_environment(&mut command.inner);
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        command.inner.output()
    }

    pub(crate) fn output_in_isolated_common_dir(
        &self,
        mut command: GitCommand,
        isolated: &IsolatedGitCommonDir,
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolated.validate()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                isolated.root.path(),
                "owned isolated Git common directory",
            )?;
        isolate_git_command_environment(&mut command.inner);
        scrub_repository_and_config_environment(&mut command.inner);

        let git_dir = self.authority.active_git_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git directory is unavailable for isolated three-way apply",
            )
        })?;
        let common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common directory is unavailable for isolated three-way apply",
            )
        })?;
        let worktree = self.authority.active_worktree_root();
        command
            .inner
            .env("GIT_DIR", git_dir)
            .env("GIT_COMMON_DIR", isolated.root.path())
            .env("GIT_WORK_TREE", worktree)
            .env("GIT_INDEX_FILE", git_dir.join("index"))
            .env("GIT_OBJECT_DIRECTORY", common_dir.join("objects"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", isolated.system_config_path())
            .env("GIT_CONFIG_GLOBAL", isolated.global_config_path())
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("HOME", isolated.home_path())
            .env("XDG_CONFIG_HOME", isolated.xdg_config_home());
        #[cfg(windows)]
        command
            .inner
            .env("APPDATA", isolated.home_path())
            .env("PROGRAMDATA", isolated.home_path())
            .env("USERPROFILE", isolated.home_path());
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
        let config_environment = GitConfigEnvironmentSnapshot::capture().map_err(|error| {
            GitReadError::InvalidConfigEnvironment {
                reason: error.to_string(),
            }
        })?;
        Ok(Self {
            executable: selected.executable,
            #[cfg(any(unix, test))]
            argv0: selected.argv0,
            safe_path: selected.safe_path,
            authority,
            config_environment,
        })
    }
}

fn scrub_repository_and_config_environment(command: &mut Command) {
    let mut names = std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| isolated_launch_variable(name))
        .collect::<Vec<_>>();
    names.extend(
        command
            .get_envs()
            .filter(|&(name, _)| isolated_launch_variable(name))
            .map(|(name, _)| name.to_os_string()),
    );
    names.sort();
    names.dedup();
    for name in names {
        command.env_remove(name);
    }
}

fn isolated_launch_variable(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    matches!(
        name.as_str(),
        "GIT_DIR"
            | "GIT_COMMON_DIR"
            | "GIT_WORK_TREE"
            | "GIT_INDEX_FILE"
            | "GIT_INDEX_VERSION"
            | "GIT_OBJECT_DIRECTORY"
            | "GIT_ALTERNATE_OBJECT_DIRECTORIES"
            | "GIT_NAMESPACE"
            | "GIT_QUARANTINE_PATH"
            | "GIT_GRAFT_FILE"
            | "GIT_SHALLOW_FILE"
            | "GIT_REPLACE_REF_BASE"
            | "GIT_NO_REPLACE_OBJECTS"
            | "GIT_ATTR_SOURCE"
            | "GIT_ATTR_NOSYSTEM"
            | "GIT_CONFIG"
            | "GIT_CONFIG_GLOBAL"
            | "GIT_CONFIG_SYSTEM"
            | "GIT_CONFIG_NOSYSTEM"
            | "GIT_CONFIG_COUNT"
            | "GIT_CONFIG_PARAMETERS"
            | "GIT_DEFAULT_HASH"
            | "GIT_DEFAULT_REF_FORMAT"
            | "GIT_REFERENCE_BACKEND"
            | "HOME"
            | "XDG_CONFIG_HOME"
            | "APPDATA"
            | "PROGRAMDATA"
            | "USERPROFILE"
            | "HOMEDRIVE"
            | "HOMEPATH"
    ) || name.starts_with("GIT_CONFIG_KEY_")
        || name.starts_with("GIT_CONFIG_VALUE_")
}

#[cfg(test)]
thread_local! {
    static GIT_RUNNER_CONSTRUCTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_git_runner_construction_count() {
    GIT_RUNNER_CONSTRUCTION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn git_runner_construction_count() -> usize {
    GIT_RUNNER_CONSTRUCTION_COUNT.with(std::cell::Cell::get)
}

pub(crate) fn repository_authority_for_cwd(
    cwd: &Path,
) -> Result<RepositoryAuthority, GitReadError> {
    RepositoryAuthority::discover(cwd)
}

#[cfg(test)]
#[path = "git_command_tests.rs"]
mod tests;
