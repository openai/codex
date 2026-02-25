use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::GitToolingError;

pub(crate) fn ensure_git_repository(path: &Path) -> Result<(), GitToolingError> {
    match run_git_for_stdout(
        path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--is-inside-work-tree"),
        ],
        None,
    ) {
        Ok(output) if output.trim() == "true" => Ok(()),
        Ok(_) => Err(GitToolingError::NotAGitRepository {
            path: path.to_path_buf(),
        }),
        Err(GitToolingError::GitCommand { status, .. }) if status.code() == Some(128) => {
            Err(GitToolingError::NotAGitRepository {
                path: path.to_path_buf(),
            })
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn resolve_head(path: &Path) -> Result<Option<String>, GitToolingError> {
    match run_git_for_stdout(
        path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("HEAD"),
        ],
        None,
    ) {
        Ok(sha) => Ok(Some(sha)),
        Err(GitToolingError::GitCommand { status, .. }) if status.code() == Some(128) => Ok(None),
        Err(other) => Err(other),
    }
}

pub(crate) fn normalize_relative_path(path: &Path) -> Result<PathBuf, GitToolingError> {
    let mut result = PathBuf::new();
    let mut saw_component = false;
    for component in path.components() {
        saw_component = true;
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return Err(GitToolingError::PathEscapesRepository {
                        path: path.to_path_buf(),
                    });
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(GitToolingError::NonRelativePath {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    if !saw_component {
        return Err(GitToolingError::NonRelativePath {
            path: path.to_path_buf(),
        });
    }

    Ok(result)
}

pub(crate) fn resolve_repository_root(path: &Path) -> Result<PathBuf, GitToolingError> {
    let root = run_git_for_stdout(
        path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--show-toplevel"),
        ],
        None,
    )?;
    Ok(PathBuf::from(root))
}

pub(crate) fn apply_repo_prefix_to_force_include(
    prefix: Option<&Path>,
    paths: &[PathBuf],
) -> Vec<PathBuf> {
    if paths.is_empty() {
        return Vec::new();
    }

    match prefix {
        Some(prefix) => paths.iter().map(|path| prefix.join(path)).collect(),
        None => paths.to_vec(),
    }
}

pub(crate) fn repo_subdir(repo_root: &Path, repo_path: &Path) -> Option<PathBuf> {
    if repo_root == repo_path {
        return None;
    }

    repo_path
        .strip_prefix(repo_root)
        .ok()
        .and_then(non_empty_path)
        .or_else(|| {
            let repo_root_canon = repo_root.canonicalize().ok()?;
            let repo_path_canon = repo_path.canonicalize().ok()?;
            repo_path_canon
                .strip_prefix(&repo_root_canon)
                .ok()
                .and_then(non_empty_path)
        })
}

fn non_empty_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path.to_path_buf())
    }
}

pub(crate) fn run_git_for_status<I, S>(
    dir: &Path,
    args: I,
    env: Option<&[(OsString, OsString)]>,
) -> Result<(), GitToolingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_git(dir, args, env)?;
    Ok(())
}

pub(crate) fn run_git_for_stdout<I, S>(
    dir: &Path,
    args: I,
    env: Option<&[(OsString, OsString)]>,
) -> Result<String, GitToolingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let run = run_git(dir, args, env)?;
    String::from_utf8(run.output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|source| GitToolingError::GitOutputUtf8 {
            command: run.command,
            source,
        })
}

/// Executes `git` and returns the full stdout without trimming so callers
/// can parse delimiter-sensitive output, propagating UTF-8 errors with context.
pub(crate) fn run_git_for_stdout_all<I, S>(
    dir: &Path,
    args: I,
    env: Option<&[(OsString, OsString)]>,
) -> Result<String, GitToolingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    // Keep the raw stdout untouched so callers can parse delimiter-sensitive
    // output (e.g. NUL-separated paths) without trimming artefacts.
    let run = run_git(dir, args, env)?;
    // Propagate UTF-8 conversion failures with the command context for debugging.
    String::from_utf8(run.output.stdout).map_err(|source| GitToolingError::GitOutputUtf8 {
        command: run.command,
        source,
    })
}

fn run_git<I, S>(
    dir: &Path,
    args: I,
    env: Option<&[(OsString, OsString)]>,
) -> Result<GitRun, GitToolingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let iterator = args.into_iter();
    let (lower, upper) = iterator.size_hint();
    let mut args_vec = Vec::with_capacity(upper.unwrap_or(lower));
    for arg in iterator {
        args_vec.push(OsString::from(arg.as_ref()));
    }
    let command_string = build_command_string(&args_vec);
    let mut command = Command::new("git");
    command.current_dir(dir);
    if let Some(envs) = env {
        for (key, value) in envs {
            command.env(key, value);
        }
    }
    command.args(&args_vec);
    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitToolingError::GitCommand {
            command: command_string,
            status: output.status,
            stderr,
        });
    }
    Ok(GitRun {
        command: command_string,
        output,
    })
}

fn build_command_string(args: &[OsString]) -> String {
    if args.is_empty() {
        return "git".to_string();
    }
    let joined = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    format!("git {joined}")
}

struct GitRun {
    command: String,
    output: std::process::Output,
}
