use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::errors::GitReadError;
use crate::git_config::path_is_within;
use crate::safe_git::isolate_git_command_environment;

/// A Git executable outside the repository-controlled roots for one operation.
#[derive(Clone, Debug)]
pub(crate) struct GitRunner {
    executable: PathBuf,
}

struct UntrustedGitLocations {
    roots: Vec<PathBuf>,
    common_dir: Option<PathBuf>,
}

impl GitRunner {
    pub(crate) fn for_cwd(cwd: &Path) -> Result<Self, GitReadError> {
        let locations = untrusted_git_locations_for_cwd(cwd)?;
        let search_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        Self::from_search_path(&locations, &search_path)
    }

    pub(crate) fn for_cwd_io(cwd: &Path) -> io::Result<Self> {
        Self::for_cwd(cwd).map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))
    }

    pub(crate) fn command(&self) -> Command {
        Command::new(&self.executable)
    }

    pub(crate) fn output(&self, mut command: Command) -> io::Result<std::process::Output> {
        isolate_git_command_environment(&mut command);
        command.envs(crate::local_only_git_env());
        command.output()
    }

    fn from_search_path(
        untrusted: &UntrustedGitLocations,
        search_path: &OsStr,
    ) -> Result<Self, GitReadError> {
        for directory in std::env::split_paths(search_path) {
            if !directory.is_absolute() {
                continue;
            }
            let candidate = directory.join(git_executable_name());
            if path_is_untrusted(&candidate, untrusted) {
                continue;
            }
            let Ok(canonical_parent) = std::fs::canonicalize(&directory) else {
                continue;
            };
            if path_is_untrusted(&canonical_parent, untrusted) {
                continue;
            }
            let Ok(canonical_candidate) = std::fs::canonicalize(&candidate) else {
                continue;
            };
            if path_is_untrusted(&canonical_candidate, untrusted)
                || !is_native_executable_file(&canonical_candidate)
            {
                continue;
            }
            return Ok(Self {
                // Preserve multicall spelling because argv[0] may select mode.
                executable: candidate,
            });
        }
        Err(GitReadError::NoTrustedGit)
    }
}

fn untrusted_git_locations_for_cwd(cwd: &Path) -> Result<UntrustedGitLocations, GitReadError> {
    let canonical_cwd = std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
        path: cwd.to_path_buf(),
    })?;
    let worktree_root = crate::get_git_repo_root(&canonical_cwd)
        .and_then(|root| std::fs::canonicalize(root).ok())
        .unwrap_or_else(|| canonical_cwd.clone());
    let mut roots = vec![worktree_root.clone()];
    let dot_git = worktree_root.join(".git");
    let common_dir = match std::fs::symlink_metadata(&dot_git) {
        Ok(_) => Some(resolve_common_git_dir(&dot_git).map_err(|()| GitReadError::NoTrustedGit)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => return Err(GitReadError::NoTrustedGit),
    };
    if let Some(common_dir) = &common_dir
        && !path_is_within(common_dir, &worktree_root)
    {
        roots.push(common_dir.clone());
    }
    Ok(UntrustedGitLocations { roots, common_dir })
}

fn path_is_untrusted(path: &Path, locations: &UntrustedGitLocations) -> bool {
    if locations
        .roots
        .iter()
        .any(|root| path_is_within(path, root))
    {
        return true;
    }
    locations
        .common_dir
        .as_deref()
        .is_some_and(|common_dir| path_is_in_worktree_for_common_dir(path, common_dir))
}

fn path_is_in_worktree_for_common_dir(path: &Path, expected_common_dir: &Path) -> bool {
    let path = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for ancestor in path.ancestors() {
        let dot_git = ancestor.join(".git");
        match std::fs::symlink_metadata(&dot_git) {
            Ok(_) => match resolve_common_git_dir(&dot_git) {
                Ok(common_dir) if paths_equal(&common_dir, expected_common_dir) => return true,
                Ok(_) => {}
                Err(()) => return true,
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
    }
    false
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    path_is_within(left, right) && path_is_within(right, left)
}

fn resolve_common_git_dir(dot_git: &Path) -> Result<PathBuf, ()> {
    if dot_git.is_dir() {
        return std::fs::canonicalize(dot_git).map_err(|_| ());
    }
    let contents = std::fs::read_to_string(dot_git).map_err(|_| ())?;
    let git_dir = contents
        .trim()
        .strip_prefix("gitdir:")
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or(())?;
    let git_dir = canonicalize_from(dot_git.parent().ok_or(())?, git_dir)?;
    let commondir = git_dir.join("commondir");
    if commondir.is_file() {
        let common_dir = std::fs::read_to_string(commondir).map_err(|_| ())?;
        let common_dir = common_dir.trim();
        if common_dir.is_empty() {
            return Err(());
        }
        return canonicalize_from(&git_dir, common_dir);
    }
    if git_dir
        .parent()
        .is_some_and(|parent| parent.file_name() == Some(OsStr::new("worktrees")))
    {
        return std::fs::canonicalize(git_dir.parent().and_then(Path::parent).ok_or(())?)
            .map_err(|_| ());
    }
    Ok(git_dir)
}

fn canonicalize_from(base: &Path, path: &str) -> Result<PathBuf, ()> {
    std::fs::canonicalize(base.join(path)).map_err(|_| ())
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
