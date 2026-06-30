use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::GitReadError;
use crate::git_config::path_is_within;
use crate::safe_git::isolate_git_command_environment;
use crate::safe_git::isolate_tokio_git_command_environment;

/// A Git executable resolved outside every repository-controlled root for one
/// internal operation.
#[derive(Clone, Debug)]
pub(crate) struct GitRunner {
    executable: PathBuf,
}

impl GitRunner {
    pub(crate) fn for_cwd(cwd: &Path) -> Result<Self, GitReadError> {
        let roots = untrusted_roots_for_cwd(cwd)?;
        let search_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        Self::from_search_path(&roots, &search_path)
    }

    pub(crate) fn for_cwd_io(cwd: &Path) -> io::Result<Self> {
        Self::for_cwd(cwd).map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))
    }

    pub(crate) fn command(&self) -> Command {
        Command::new(&self.executable)
    }

    pub(crate) fn tokio_command(&self) -> tokio::process::Command {
        tokio::process::Command::new(&self.executable)
    }

    /// Spawn a configured synchronous command after applying the final
    /// repository-selector environment lock.
    ///
    /// PR #29470 composes its one local-only transport policy at this final
    /// boundary so caller configuration cannot restore transport authority.
    pub(crate) fn output(&self, mut command: Command) -> io::Result<std::process::Output> {
        isolate_git_command_environment(&mut command);
        command.envs(crate::local_only_git_env());
        command.output()
    }

    /// Like [`Self::output`], but restores one internally generated scratch
    /// index after inherited/caller-provided selectors have been removed.
    pub(crate) fn output_with_index_file(
        &self,
        mut command: Command,
        index_file: &Path,
    ) -> io::Result<std::process::Output> {
        isolate_git_command_environment(&mut command);
        let index_file = index_file_for_git_env(index_file)?;
        command.env("GIT_INDEX_FILE", index_file);
        command.envs(crate::local_only_git_env());
        command.output()
    }

    /// Spawn a configured Tokio command after applying the final
    /// repository-selector environment lock.
    pub(crate) async fn output_tokio(
        &self,
        mut command: tokio::process::Command,
    ) -> io::Result<std::process::Output> {
        isolate_tokio_git_command_environment(&mut command);
        command.envs(crate::local_only_git_env());
        command.output().await
    }

    fn from_search_path(
        untrusted_roots: &[PathBuf],
        search_path: &OsStr,
    ) -> Result<Self, GitReadError> {
        for directory in std::env::split_paths(search_path) {
            if !directory.is_absolute() {
                continue;
            }
            let candidate = directory.join(git_executable_name());
            if untrusted_roots
                .iter()
                .any(|root| path_is_within(&candidate, root))
            {
                continue;
            }
            let Ok(canonical_parent) = std::fs::canonicalize(&directory) else {
                continue;
            };
            if untrusted_roots
                .iter()
                .any(|root| path_is_within(&canonical_parent, root))
            {
                continue;
            }
            let Ok(canonical_candidate) = std::fs::canonicalize(&candidate) else {
                continue;
            };
            if untrusted_roots
                .iter()
                .any(|root| path_is_within(&canonical_candidate, root))
                || !is_native_executable_file(&canonical_candidate)
            {
                continue;
            }
            return Ok(Self {
                // Preserve the absolute PATH entry rather than replacing it
                // with the canonical target: Git may be installed through a
                // trusted multicall symlink whose argv[0] selects Git mode.
                executable: candidate,
            });
        }
        Err(GitReadError::NoTrustedGit)
    }

    #[cfg(test)]
    fn executable(&self) -> &Path {
        &self.executable
    }

    #[cfg(all(test, unix))]
    pub(crate) fn from_executable_for_test(executable: PathBuf) -> Self {
        Self { executable }
    }
}

fn index_file_for_git_env(index_file: &Path) -> io::Result<PathBuf> {
    AbsolutePathBuf::from_absolute_path_checked(index_file).map(AbsolutePathBuf::into_path_buf)
}

fn untrusted_roots_for_cwd(cwd: &Path) -> Result<Vec<PathBuf>, GitReadError> {
    let canonical_cwd = std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
        path: cwd.to_path_buf(),
    })?;
    let worktree_root = crate::get_git_repo_root(&canonical_cwd)
        .and_then(|root| std::fs::canonicalize(root).ok())
        .unwrap_or_else(|| canonical_cwd.clone());
    let mut roots = vec![worktree_root.clone()];
    if let Some(main_root) = linked_main_worktree_root(&worktree_root)
        && !roots.contains(&main_root)
    {
        roots.push(main_root);
    }
    Ok(roots)
}

fn linked_main_worktree_root(worktree_root: &Path) -> Option<PathBuf> {
    let dot_git = worktree_root.join(".git");
    if dot_git.is_dir() {
        return None;
    }
    let contents = std::fs::read_to_string(dot_git).ok()?;
    let git_dir = contents.trim().strip_prefix("gitdir:")?.trim();
    if git_dir.is_empty() {
        return None;
    }
    let git_dir = PathBuf::from(git_dir);
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        worktree_root.join(git_dir)
    };
    let git_dir = std::fs::canonicalize(git_dir).ok()?;
    let worktrees = git_dir.parent()?;
    if worktrees.file_name()? != OsStr::new("worktrees") {
        return None;
    }
    std::fs::canonicalize(worktrees.parent()?.parent()?).ok()
}

#[cfg(windows)]
fn git_executable_name() -> &'static str {
    "git.exe"
}

#[cfg(not(windows))]
fn git_executable_name() -> &'static str {
    "git"
}

#[cfg(unix)]
fn is_native_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn is_native_executable_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(not(any(unix, windows)))]
fn is_native_executable_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
#[path = "git_command_tests.rs"]
mod tests;
