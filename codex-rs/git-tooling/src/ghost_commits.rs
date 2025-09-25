use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use tempfile::Builder;

use crate::GhostCommit;
use crate::GitToolingError;
use crate::operations::apply_repo_prefix_to_force_include;
use crate::operations::ensure_git_repository;
use crate::operations::normalize_relative_path;
use crate::operations::repo_subdir;
use crate::operations::resolve_head;
use crate::operations::resolve_repository_root;
use crate::operations::run_git_for_status;
use crate::operations::run_git_for_stdout;

/// Default commit message used for ghost commits when none is provided.
const DEFAULT_COMMIT_MESSAGE: &str = "codex snapshot";

/// Options to control ghost commit creation.
pub struct CreateGhostCommitOptions<'a> {
    pub repo_path: &'a Path,
    pub message: Option<&'a str>,
    pub force_include: Vec<PathBuf>,
}

impl<'a> CreateGhostCommitOptions<'a> {
    /// Creates options scoped to the provided repository path.
    pub fn new(repo_path: &'a Path) -> Self {
        Self {
            repo_path,
            message: None,
            force_include: Vec::new(),
        }
    }

    /// Sets a custom commit message for the ghost commit.
    pub fn message(mut self, message: &'a str) -> Self {
        self.message = Some(message);
        self
    }

    /// Supplies the entire force-include path list at once.
    pub fn force_include<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.force_include = paths.into_iter().collect();
        self
    }

    /// Adds a single path to the force-include list.
    pub fn push_force_include<P>(mut self, path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.force_include.push(path.into());
        self
    }
}

/// Create a ghost commit capturing the current state of the repository's working tree.
pub fn create_ghost_commit(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostCommit, GitToolingError> {
    ensure_git_repository(options.repo_path)?;

    let repo_root = resolve_repository_root(options.repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), options.repo_path);
    let parent = resolve_head(repo_root.as_path())?;

    let normalized_force = options
        .force_include
        .iter()
        .map(|path| normalize_relative_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    let force_include =
        apply_repo_prefix_to_force_include(repo_prefix.as_deref(), &normalized_force);
    let index_tempdir = Builder::new().prefix("codex-git-index-").tempdir()?;
    let index_path = index_tempdir.path().join("index");
    let base_env = vec![(
        OsString::from("GIT_INDEX_FILE"),
        OsString::from(index_path.as_os_str()),
    )];

    let mut add_args = vec![OsString::from("add"), OsString::from("--all")];
    if let Some(prefix) = repo_prefix.as_deref() {
        add_args.extend([OsString::from("--"), prefix.as_os_str().to_os_string()]);
    }

    run_git_for_status(repo_root.as_path(), add_args, Some(base_env.as_slice()))?;
    if !force_include.is_empty() {
        let mut args = Vec::with_capacity(force_include.len() + 2);
        args.push(OsString::from("add"));
        args.push(OsString::from("--force"));
        args.extend(
            force_include
                .iter()
                .map(|path| OsString::from(path.as_os_str())),
        );
        run_git_for_status(repo_root.as_path(), args, Some(base_env.as_slice()))?;
    }

    let tree_id = run_git_for_stdout(
        repo_root.as_path(),
        vec![OsString::from("write-tree")],
        Some(base_env.as_slice()),
    )?;

    let mut commit_env = base_env;
    commit_env.extend(default_commit_identity());
    let message = options.message.unwrap_or(DEFAULT_COMMIT_MESSAGE);
    let commit_args = {
        let mut result = vec![OsString::from("commit-tree"), OsString::from(&tree_id)];
        if let Some(parent) = parent.as_deref() {
            result.extend([OsString::from("-p"), OsString::from(parent)]);
        }
        result.extend([OsString::from("-m"), OsString::from(message)]);
        result
    };

    // Retrieve commit ID.
    let commit_id = run_git_for_stdout(
        repo_root.as_path(),
        commit_args,
        Some(commit_env.as_slice()),
    )?;

    Ok(GhostCommit::new(commit_id, parent))
}

/// Restore the working tree to match the provided ghost commit.
pub fn restore_ghost_commit(repo_path: &Path, commit: &GhostCommit) -> Result<(), GitToolingError> {
    restore_to_commit(repo_path, commit.id())
}

/// Restore the working tree to match the given commit ID.
pub fn restore_to_commit(repo_path: &Path, commit_id: &str) -> Result<(), GitToolingError> {
    ensure_git_repository(repo_path)?;

    let repo_root = resolve_repository_root(repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), repo_path);

    let mut restore_args = vec![
        OsString::from("restore"),
        OsString::from("--source"),
        OsString::from(commit_id),
        OsString::from("--worktree"),
        OsString::from("--staged"),
        OsString::from("--"),
    ];
    if let Some(prefix) = repo_prefix.as_deref() {
        restore_args.push(prefix.as_os_str().to_os_string());
    } else {
        restore_args.push(OsString::from("."));
    }

    run_git_for_status(repo_root.as_path(), restore_args, None)?;
    Ok(())
}

/// Returns the default author and committer identity for ghost commits.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::run_git_for_stdout;
    use pretty_assertions::assert_eq;
    use std::process::Command;

    /// Runs a git command in the test repository and asserts success.
    fn run_git_in(repo_path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git command failed: {args:?}");
    }

    /// Runs a git command and returns its trimmed stdout output.
    fn run_git_stdout(repo_path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("git command");
        assert!(output.status.success(), "git command failed: {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Initializes a repository with consistent settings for cross-platform tests.
    fn init_test_repo(repo: &Path) {
        run_git_in(repo, &["init", "--initial-branch=main"]);
        run_git_in(repo, &["config", "core.autocrlf", "false"]);
    }

    #[test]
    /// Verifies a ghost commit can be created and restored end to end.
    fn create_and_restore_roundtrip() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);
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
        assert!(repo.join("ephemeral.txt").exists());

        Ok(())
    }

    #[test]
    /// Ensures ghost commits succeed in repositories without an existing HEAD.
    fn create_snapshot_without_existing_head() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

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
    /// Confirms custom messages are used when creating ghost commits.
    fn create_ghost_commit_uses_custom_message() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join("tracked.txt"), "contents\n")?;
        run_git_in(repo, &["add", "tracked.txt"]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );

        let message = "custom message";
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo).message(message))?;
        let commit_message = run_git_stdout(repo, &["log", "-1", "--format=%s", ghost.id()]);
        assert_eq!(commit_message, message);

        Ok(())
    }

    #[test]
    /// Rejects force-included paths that escape the repository.
    fn create_ghost_commit_rejects_force_include_parent_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path();
        init_test_repo(repo);
        let options = CreateGhostCommitOptions::new(repo)
            .force_include(vec![PathBuf::from("../outside.txt")]);
        let err = create_ghost_commit(&options).unwrap_err();
        assert!(matches!(err, GitToolingError::PathEscapesRepository { .. }));
    }

    #[test]
    /// Restoring a ghost commit from a non-git directory fails.
    fn restore_requires_git_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = restore_to_commit(temp.path(), "deadbeef").unwrap_err();
        assert!(matches!(err, GitToolingError::NotAGitRepository { .. }));
    }

    #[test]
    /// Restoring from a subdirectory affects only that subdirectory.
    fn restore_from_subdirectory_restores_files_relatively() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::create_dir_all(repo.join("workspace"))?;
        let workspace = repo.join("workspace");
        std::fs::write(repo.join("root.txt"), "root contents\n")?;
        std::fs::write(workspace.join("nested.txt"), "nested contents\n")?;
        run_git_in(repo, &["add", "."]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );

        std::fs::write(repo.join("root.txt"), "root modified\n")?;
        std::fs::write(workspace.join("nested.txt"), "nested modified\n")?;

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(&workspace))?;

        std::fs::write(repo.join("root.txt"), "root after\n")?;
        std::fs::write(workspace.join("nested.txt"), "nested after\n")?;

        restore_ghost_commit(&workspace, &ghost)?;

        let root_after = std::fs::read_to_string(repo.join("root.txt"))?;
        assert_eq!(root_after, "root after\n");
        let nested_after = std::fs::read_to_string(workspace.join("nested.txt"))?;
        assert_eq!(nested_after, "nested modified\n");
        assert!(!workspace.join("codex-rs").exists());

        Ok(())
    }

    #[test]
    /// Restoring from a subdirectory preserves ignored files in parent folders.
    fn restore_from_subdirectory_preserves_parent_vscode() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        let workspace = repo.join("codex-rs");
        std::fs::create_dir_all(&workspace)?;
        std::fs::write(repo.join(".gitignore"), ".vscode/\n")?;
        std::fs::write(workspace.join("tracked.txt"), "snapshot version\n")?;
        run_git_in(repo, &["add", "."]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );

        std::fs::write(workspace.join("tracked.txt"), "snapshot delta\n")?;
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(&workspace))?;

        std::fs::write(workspace.join("tracked.txt"), "post-snapshot\n")?;
        let vscode = repo.join(".vscode");
        std::fs::create_dir_all(&vscode)?;
        std::fs::write(vscode.join("settings.json"), "{\n  \"after\": true\n}\n")?;

        restore_ghost_commit(&workspace, &ghost)?;

        let tracked_after = std::fs::read_to_string(workspace.join("tracked.txt"))?;
        assert_eq!(tracked_after, "snapshot delta\n");
        assert!(vscode.join("settings.json").exists());
        let settings_after = std::fs::read_to_string(vscode.join("settings.json"))?;
        assert_eq!(settings_after, "{\n  \"after\": true\n}\n");

        Ok(())
    }

    #[test]
    /// Restoring from the repository root keeps ignored files intact.
    fn restore_preserves_ignored_files() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join(".gitignore"), ".vscode/\n")?;
        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
        let vscode = repo.join(".vscode");
        std::fs::create_dir_all(&vscode)?;
        std::fs::write(vscode.join("settings.json"), "{\n  \"before\": true\n}\n")?;
        run_git_in(repo, &["add", ".gitignore", "tracked.txt"]);
        run_git_in(
            repo,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );

        std::fs::write(repo.join("tracked.txt"), "snapshot delta\n")?;
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        std::fs::write(repo.join("tracked.txt"), "post-snapshot\n")?;
        std::fs::write(vscode.join("settings.json"), "{\n  \"after\": true\n}\n")?;
        std::fs::write(repo.join("temp.txt"), "new file\n")?;

        restore_ghost_commit(repo, &ghost)?;

        let tracked_after = std::fs::read_to_string(repo.join("tracked.txt"))?;
        assert_eq!(tracked_after, "snapshot delta\n");
        assert!(vscode.join("settings.json").exists());
        let settings_after = std::fs::read_to_string(vscode.join("settings.json"))?;
        assert_eq!(settings_after, "{\n  \"after\": true\n}\n");
        assert!(repo.join("temp.txt").exists());

        Ok(())
    }
}
