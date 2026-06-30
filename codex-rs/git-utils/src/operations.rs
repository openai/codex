use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::GitToolingError;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::isolate_git_command_environment;

pub(crate) fn ensure_git_repository(path: &Path) -> Result<(), GitToolingError> {
    match run_git_for_stdout(
        path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--is-inside-work-tree"),
        ],
        /*env*/ None,
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
        /*env*/ None,
    ) {
        Ok(sha) => Ok(Some(sha)),
        Err(GitToolingError::GitCommand { status, .. }) if status.code() == Some(128) => Ok(None),
        Err(other) => Err(other),
    }
}

pub(crate) fn resolve_repository_root(path: &Path) -> Result<PathBuf, GitToolingError> {
    let root = run_git_for_stdout(
        path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--show-toplevel"),
        ],
        /*env*/ None,
    )?;
    Ok(PathBuf::from(root))
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
    let mut args_vec = Vec::with_capacity(upper.unwrap_or(lower) + 2);
    // Keep internal Git helper commands independent of configured hook directories.
    args_vec.push(OsString::from("-c"));
    args_vec.push(OsString::from(format!(
        "core.hooksPath={DISABLED_HOOKS_PATH}"
    )));
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
    isolate_git_command_environment(&mut command);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        let mut command = Command::new("git");
        isolate_git_command_environment(&mut command);
        let status = command
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .expect("initialize repository");
        assert!(status.success());
        repo
    }

    #[test]
    fn caller_env_cannot_restore_repository_or_pathspec_selectors() {
        let target = init_repo();
        let alternate = init_repo();
        std::fs::write(target.path().join("target.txt"), "target\n").expect("target file");
        std::fs::write(alternate.path().join("alternate.txt"), "alternate\n")
            .expect("alternate file");
        for (repo, path) in [(&target, "target.txt"), (&alternate, "alternate.txt")] {
            let mut command = Command::new("git");
            isolate_git_command_environment(&mut command);
            let status = command
                .args(["add", path])
                .current_dir(repo.path())
                .status()
                .expect("add file");
            assert!(status.success());
        }

        let alternate_git_dir = alternate.path().join(".git");
        let env = [
            (
                OsString::from("GIT_DIR"),
                alternate_git_dir.as_os_str().into(),
            ),
            (
                OsString::from("GIT_WORK_TREE"),
                alternate.path().as_os_str().into(),
            ),
            (
                OsString::from("GIT_COMMON_DIR"),
                alternate_git_dir.as_os_str().into(),
            ),
            (
                OsString::from("GIT_INDEX_FILE"),
                alternate_git_dir.join("index").into_os_string(),
            ),
            (OsString::from("GIT_PREFIX"), OsString::from("elsewhere/")),
            (OsString::from("GIT_LITERAL_PATHSPECS"), OsString::from("1")),
            (OsString::from("GIT_GLOB_PATHSPECS"), OsString::from("1")),
            (OsString::from("GIT_NOGLOB_PATHSPECS"), OsString::from("1")),
            (OsString::from("GIT_ICASE_PATHSPECS"), OsString::from("1")),
        ];
        let output = run_git_for_stdout(target.path(), ["ls-files"], Some(&env))
            .expect("query cwd-selected index");
        assert_eq!(output, "target.txt");
    }
}
