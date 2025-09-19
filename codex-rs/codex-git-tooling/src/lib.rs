use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use std::string::FromUtf8Error;
use tempfile::Builder;
use thiserror::Error;
use walkdir::WalkDir;

#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_path;

#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;

/// Default commit message used for ghost commits when none is provided.
const DEFAULT_COMMIT_MESSAGE: &str = "codex snapshot";

/// Details of a ghost commit created from a repository state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostCommit {
    id: String,
    parent: Option<String>,
}

impl GhostCommit {
    /// Create a new ghost commit wrapper from a raw commit ID and optional parent.
    pub fn new(id: String, parent: Option<String>) -> Self {
        Self { id, parent }
    }

    /// Commit ID for the snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parent commit ID, if the repository had a `HEAD` at creation time.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Options to control ghost commit creation.
pub struct CreateGhostCommitOptions<'a> {
    pub repo_path: &'a Path,
    pub message: Option<&'a str>,
    pub force_include: Vec<PathBuf>,
}

impl<'a> CreateGhostCommitOptions<'a> {
    pub fn new(repo_path: &'a Path) -> Self {
        Self {
            repo_path,
            message: None,
            force_include: Vec::new(),
        }
    }

    pub fn message(mut self, message: &'a str) -> Self {
        self.message = Some(message);
        self
    }

    pub fn force_include<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.force_include = paths.into_iter().collect();
        self
    }

    pub fn push_force_include<P>(mut self, path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.force_include.push(path.into());
        self
    }
}

/// Errors returned while managing git worktree snapshots.
#[derive(Debug, Error)]
pub enum GitToolingError {
    #[error("git command `{command}` failed with status {status}: {stderr}")]
    GitCommand {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("git command `{command}` produced non-UTF-8 output")]
    GitOutputUtf8 {
        command: String,
        #[source]
        source: FromUtf8Error,
    },
    #[error("{path:?} is not a git repository")]
    NotAGitRepository { path: PathBuf },
    #[error("path {path:?} must be relative to the repository root")]
    NonRelativePath { path: PathBuf },
    #[error("path {path:?} escapes the repository root")]
    PathEscapesRepository { path: PathBuf },
    #[error("failed to process path inside worktree")]
    PathPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Walkdir(#[from] walkdir::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Create a ghost commit capturing the current state of the repository's working tree.
pub fn create_ghost_commit(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostCommit, GitToolingError> {
    ensure_git_repository(options.repo_path)?;

    let parent = resolve_head(options.repo_path)?;
    let normalized_force = normalize_force_include(&options.force_include)?;
    let _index_tempdir = Builder::new().prefix("codex-git-index-").tempdir()?;
    let index_path = _index_tempdir.path().join("index");
    let base_env = vec![(
        OsString::from("GIT_INDEX_FILE"),
        OsString::from(index_path.as_os_str()),
    )];
    run_git_for_status(
        options.repo_path,
        vec![OsString::from("add"), OsString::from("--all")],
        Some(base_env.as_slice()),
    )?;
    for path in &normalized_force {
        let args = vec![
            OsString::from("add"),
            OsString::from("--force"),
            OsString::from(path.as_os_str()),
        ];
        run_git_for_status(options.repo_path, args, Some(base_env.as_slice()))?;
    }

    let tree_id = run_git_for_stdout(
        options.repo_path,
        vec![OsString::from("write-tree")],
        Some(base_env.as_slice()),
    )?;

    let mut commit_env = base_env;
    commit_env.extend(default_commit_identity());
    let message = options.message.unwrap_or(DEFAULT_COMMIT_MESSAGE);
    let mut commit_args = vec![OsString::from("commit-tree"), OsString::from(&tree_id)];
    if let Some(parent) = parent.as_deref() {
        commit_args.push(OsString::from("-p"));
        commit_args.push(OsString::from(parent));
    }
    commit_args.push(OsString::from("-m"));
    commit_args.push(OsString::from(message));

    let commit_id =
        run_git_for_stdout(options.repo_path, commit_args, Some(commit_env.as_slice()))?;

    Ok(GhostCommit::new(commit_id, parent))
}

/// Restore the working tree to match the provided ghost commit.
pub fn restore_ghost_commit(repo_path: &Path, commit: &GhostCommit) -> Result<(), GitToolingError> {
    restore_to_commit(repo_path, commit.id())
}

/// Restore the working tree to match the given commit ID.
pub fn restore_to_commit(repo_path: &Path, commit_id: &str) -> Result<(), GitToolingError> {
    ensure_git_repository(repo_path)?;

    let mut worktree = TemporaryWorktree::create(repo_path, commit_id)?;
    sync_worktree_contents(worktree.path(), repo_path)?;
    worktree.remove()?;
    Ok(())
}

fn ensure_git_repository(path: &Path) -> Result<(), GitToolingError> {
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

fn resolve_head(path: &Path) -> Result<Option<String>, GitToolingError> {
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

fn normalize_force_include(paths: &[PathBuf]) -> Result<Vec<PathBuf>, GitToolingError> {
    paths
        .iter()
        .map(|path| normalize_relative_path(path))
        .collect()
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, GitToolingError> {
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

fn sync_worktree_contents(source: &Path, destination: &Path) -> Result<(), GitToolingError> {
    clean_directory(destination)?;
    let mut walker = WalkDir::new(source).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        let entry = entry?;
        if entry.depth() == 0 {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        if relative.components().next().map(|c| c.as_os_str()) == Some(OsStr::new(".git")) {
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }

        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_symlink() {
            create_symlink(entry.path(), &target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        } else {
            return Err(GitToolingError::Io(std::io::Error::other(format!(
                "unsupported file type at {}",
                entry.path().display()
            ))));
        }
    }
    Ok(())
}

fn clean_directory(path: &Path) -> Result<(), GitToolingError> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        let entry_path = entry.path();
        if file_type.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }
    Ok(())
}

fn create_symlink(source: &Path, destination: &Path) -> Result<(), GitToolingError> {
    let link_target = std::fs::read_link(source)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        symlink_path(&link_target, destination)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        let metadata = source.metadata();
        match metadata {
            Ok(data) if data.file_type().is_dir() => {
                symlink_dir(&link_target, destination)?;
            }
            _ => {
                symlink_file(&link_target, destination)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct TemporaryWorktree {
    repo_path: PathBuf,
    path: PathBuf,
    removed: bool,
}

impl TemporaryWorktree {
    fn create(repo_path: &Path, reference: &str) -> Result<Self, GitToolingError> {
        let tempdir = Builder::new().prefix("codex-git-tooling-").tempdir()?;
        let path = tempdir.keep();
        let args = vec![
            OsString::from("worktree"),
            OsString::from("add"),
            OsString::from("--detach"),
            OsString::from(path.as_os_str()),
            OsString::from(reference),
        ];
        run_git_for_status(repo_path, args, None)?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
            path,
            removed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(&mut self) -> Result<(), GitToolingError> {
        if self.removed {
            return Ok(());
        }
        let args = vec![
            OsString::from("worktree"),
            OsString::from("remove"),
            OsString::from("--force"),
            OsString::from(self.path.as_os_str()),
        ];
        run_git_for_status(self.repo_path.as_path(), args, None)?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for TemporaryWorktree {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        let args = vec![
            OsString::from("worktree"),
            OsString::from("remove"),
            OsString::from("--force"),
            OsString::from(self.path.as_os_str()),
        ];
        let _ = run_git(self.repo_path.as_path(), args, None);
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn default_commit_identity() -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from("GIT_AUTHOR_NAME"),
            OsString::from("Codex Snapshot"),
        ),
        (
            OsString::from("GIT_AUTHOR_EMAIL"),
            OsString::from("snapshot@codex.local"),
        ),
        (
            OsString::from("GIT_COMMITTER_NAME"),
            OsString::from("Codex Snapshot"),
        ),
        (
            OsString::from("GIT_COMMITTER_EMAIL"),
            OsString::from("snapshot@codex.local"),
        ),
    ]
}

struct GitRun {
    command: String,
    output: std::process::Output,
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
    let args_vec: Vec<OsString> = args
        .into_iter()
        .map(|arg| OsString::from(arg.as_ref()))
        .collect();
    let command_string = build_command_string(&args_vec);
    let mut command = Command::new("git");
    command.current_dir(dir);
    if let Some(envs) = env {
        for (key, value) in envs {
            command.env(key, value);
        }
    }
    command.args(args_vec.iter().map(|arg| arg.as_os_str()));
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

fn run_git_for_status<I, S>(
    dir: &Path,
    args: I,
    env: Option<&[(OsString, OsString)]>,
) -> Result<(), GitToolingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_git(dir, args, env).map(|_| ())
}

fn run_git_for_stdout<I, S>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn run_git_in(repo_path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn run_git_stdout(repo_path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("git command");
        assert!(output.status.success(), "git command failed: {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn create_and_restore_roundtrip() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        run_git_in(repo, &["init", "--initial-branch=main"]);
        std::fs::write(repo.join("tracked.txt"), "initial\n")?;
        std::fs::write(repo.join("delete-me.txt"), "to be removed\n")?;
        run_git_in(repo, &["add", "tracked.txt", "delete-me.txt"]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "init",
            ],
        );

        // Modify the repository state.
        let tracked_contents = "modified contents\n";
        std::fs::write(repo.join("tracked.txt"), tracked_contents)?;
        std::fs::remove_file(repo.join("delete-me.txt"))?;
        let new_file_contents = "hello ghost\n";
        std::fs::write(repo.join("new-file.txt"), new_file_contents)?;
        std::fs::write(repo.join(".gitignore"), "ignored.txt\n")?;
        let ignored_contents = "ignored but captured\n";
        std::fs::write(repo.join("ignored.txt"), ignored_contents)?;

        let options =
            CreateGhostCommitOptions::new(repo).force_include(vec![PathBuf::from("ignored.txt")]);
        let ghost = create_ghost_commit(&options)?;

        // Validate commit metadata.
        assert!(ghost.parent().is_some());
        let cat = run_git_for_stdout(
            repo,
            vec![
                OsString::from("show"),
                OsString::from(format!("{}:ignored.txt", ghost.id())),
            ],
            None,
        )?;
        assert_eq!(cat, ignored_contents.trim());

        // Diverge repo state.
        std::fs::write(repo.join("tracked.txt"), "other state\n")?;
        std::fs::write(repo.join("ignored.txt"), "changed\n")?;
        std::fs::remove_file(repo.join("new-file.txt"))?;
        std::fs::write(repo.join("ephemeral.txt"), "temp data\n")?;

        restore_ghost_commit(repo, &ghost)?;

        let tracked_after = std::fs::read_to_string(repo.join("tracked.txt"))?;
        assert_eq!(tracked_after, tracked_contents);
        let ignored_after = std::fs::read_to_string(repo.join("ignored.txt"))?;
        assert_eq!(ignored_after, ignored_contents);
        let new_file_after = std::fs::read_to_string(repo.join("new-file.txt"))?;
        assert_eq!(new_file_after, new_file_contents);
        assert_eq!(repo.join("delete-me.txt").exists(), false);
        assert_eq!(repo.join("ephemeral.txt").exists(), false);

        Ok(())
    }

    #[test]
    fn create_snapshot_without_existing_head() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        run_git_in(repo, &["init", "--initial-branch=main"]);

        let tracked_contents = "first contents\n";
        std::fs::write(repo.join("tracked.txt"), tracked_contents)?;
        let ignored_contents = "ignored but captured\n";
        std::fs::write(repo.join(".gitignore"), "ignored.txt\n")?;
        std::fs::write(repo.join("ignored.txt"), ignored_contents)?;

        let options =
            CreateGhostCommitOptions::new(repo).force_include(vec![PathBuf::from("ignored.txt")]);
        let ghost = create_ghost_commit(&options)?;

        assert!(ghost.parent().is_none());

        let message = run_git_stdout(repo, &["log", "-1", "--format=%s", ghost.id()]);
        assert_eq!(message, DEFAULT_COMMIT_MESSAGE);

        let ignored = run_git_stdout(repo, &["show", &format!("{}:ignored.txt", ghost.id())]);
        assert_eq!(ignored, ignored_contents.trim());

        Ok(())
    }

    #[test]
    fn create_ghost_commit_uses_custom_message() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        run_git_in(repo, &["init", "--initial-branch=main"]);
        std::fs::write(repo.join("file.txt"), "hello\n")?;
        run_git_in(repo, &["add", "file.txt"]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "base",
            ],
        );

        std::fs::write(repo.join("file.txt"), "updated\n")?;
        let message = "custom snapshot";
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo).message(message))?;

        assert!(ghost.parent().is_some());
        let commit_message = run_git_stdout(repo, &["log", "-1", "--format=%s", ghost.id()]);
        assert_eq!(commit_message, message);

        Ok(())
    }

    #[test]
    fn force_include_requires_relative_paths() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        run_git_in(repo, &["init", "--initial-branch=main"]);

        let options = CreateGhostCommitOptions::new(repo)
            .force_include(vec![PathBuf::from("/absolute/path")]);
        let err = create_ghost_commit(&options).unwrap_err();
        assert!(matches!(err, GitToolingError::NonRelativePath { .. }));
        Ok(())
    }

    #[test]
    fn restore_requires_git_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = restore_to_commit(temp.path(), "deadbeef").unwrap_err();
        assert!(matches!(err, GitToolingError::NotAGitRepository { .. }));
    }
}
