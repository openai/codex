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
    common_dirs: Vec<PathBuf>,
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
    let lexical_cwd = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| GitReadError::NotRepository {
                path: cwd.to_path_buf(),
            })?
            .join(cwd)
    };
    let canonical_cwd = std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
        path: cwd.to_path_buf(),
    })?;
    let worktree_root = crate::get_git_repo_root(&canonical_cwd)
        .and_then(|root| std::fs::canonicalize(root).ok())
        .unwrap_or_else(|| canonical_cwd.clone());
    let mut locations = UntrustedGitLocations {
        roots: Vec::new(),
        common_dirs: Vec::new(),
    };
    record_repository_ancestry(&worktree_root, &mut locations)?;

    // Canonicalization can erase a repository-controlled symlink prefix. Walk
    // the requested spelling too, deliberately retaining symlink and `..`
    // components so every lexical enclosing checkout remains untrusted.
    let lexical_base = if lexical_cwd.is_dir() {
        lexical_cwd
    } else {
        lexical_cwd
            .parent()
            .ok_or_else(|| GitReadError::NotRepository {
                path: cwd.to_path_buf(),
            })?
            .to_path_buf()
    };
    record_repository_ancestry(&lexical_base, &mut locations)?;

    // Callers commonly obtain their default cwd from `current_dir()`, which
    // can already have erased a symlink spelling. Recover the standard logical
    // process cwd only when it is absolute and resolves to both the requested
    // cwd and the process cwd. Treating extra roots as untrusted cannot widen
    // executable selection.
    if let Some(logical_cwd) = validated_logical_process_cwd(&canonical_cwd) {
        record_repository_ancestry(&logical_cwd, &mut locations)?;
    }
    Ok(locations)
}

fn validated_logical_process_cwd(canonical_cwd: &Path) -> Option<PathBuf> {
    let process_cwd = std::fs::canonicalize(std::env::current_dir().ok()?).ok()?;
    if !paths_equal(&process_cwd, canonical_cwd) {
        return None;
    }
    let logical_cwd = PathBuf::from(std::env::var_os("PWD")?);
    if !logical_cwd.is_absolute() {
        return None;
    }
    let canonical_logical_cwd = std::fs::canonicalize(&logical_cwd).ok()?;
    paths_equal(&canonical_logical_cwd, canonical_cwd).then_some(logical_cwd)
}

fn record_repository_ancestry(
    start: &Path,
    locations: &mut UntrustedGitLocations,
) -> Result<(), GitReadError> {
    push_unique(&mut locations.roots, start.to_path_buf());
    record_repository_marker(start, locations)?;
    for ancestor in start.parent().into_iter().flat_map(Path::ancestors) {
        let dot_git = ancestor.join(".git");
        match std::fs::symlink_metadata(&dot_git) {
            Ok(_) => {
                push_unique(&mut locations.roots, ancestor.to_path_buf());
                let canonical_root =
                    std::fs::canonicalize(ancestor).map_err(|_| GitReadError::NoTrustedGit)?;
                push_unique(&mut locations.roots, canonical_root.clone());
                record_repository_marker(ancestor, locations)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(GitReadError::NoTrustedGit),
        }
    }
    Ok(())
}

fn record_repository_marker(
    worktree_root: &Path,
    locations: &mut UntrustedGitLocations,
) -> Result<(), GitReadError> {
    let dot_git = worktree_root.join(".git");
    let common_dir = match std::fs::symlink_metadata(&dot_git) {
        Ok(_) => resolve_common_git_dir(&dot_git).map_err(|()| GitReadError::NoTrustedGit)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(GitReadError::NoTrustedGit),
    };
    if !path_is_within(&common_dir, worktree_root) {
        push_unique(&mut locations.roots, common_dir.clone());
    }
    push_unique(&mut locations.common_dirs, common_dir);
    Ok(())
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| paths_equal(existing, &path)) {
        paths.push(path);
    }
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
        .common_dirs
        .iter()
        .any(|common_dir| path_is_in_worktree_for_common_dir(path, common_dir))
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
