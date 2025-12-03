use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Component;
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
use crate::operations::run_git_for_stdout_all;

/// Default commit message used for ghost commits when none is provided.
const DEFAULT_COMMIT_MESSAGE: &str = "codex snapshot";
/// Default threshold that triggers a warning about large untracked directories.
const LARGE_UNTRACKED_WARNING_THRESHOLD: usize = 200;
/// Directories that should always be ignored when capturing ghost snapshots,
/// even if they are not listed in .gitignore.
///
/// These are typically large dependency or build trees that are not useful
/// for undo and can cause snapshots to grow without bound.
const DEFAULT_IGNORED_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".venv",
    "venv",
    "env",
    ".env",
    "dist",
    "build",
    ".pytest_cache",
    ".mypy_cache",
    ".cache",
    ".tox",
];

/// Options to control ghost commit creation.
pub struct CreateGhostCommitOptions<'a> {
    pub repo_path: &'a Path,
    pub message: Option<&'a str>,
    pub force_include: Vec<PathBuf>,
}

/// Summary produced alongside a ghost snapshot.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GhostSnapshotReport {
    pub large_untracked_dirs: Vec<LargeUntrackedDir>,
}

/// Directory containing a large amount of untracked content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LargeUntrackedDir {
    pub path: PathBuf,
    pub file_count: usize,
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

fn detect_large_untracked_dirs(files: &[PathBuf], dirs: &[PathBuf]) -> Vec<LargeUntrackedDir> {
    let mut counts: BTreeMap<PathBuf, usize> = BTreeMap::new();

    let mut sorted_dirs: Vec<&PathBuf> = dirs.iter().collect();
    sorted_dirs.sort_by(|a, b| {
        let a_components = a.components().count();
        let b_components = b.components().count();
        b_components.cmp(&a_components).then_with(|| a.cmp(b))
    });

    for file in files {
        let mut key: Option<PathBuf> = None;
        for dir in &sorted_dirs {
            if file.starts_with(dir.as_path()) {
                key = Some((*dir).clone());
                break;
            }
        }
        let key = key.unwrap_or_else(|| {
            file.parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let entry = counts.entry(key).or_insert(0);
        *entry += 1;
    }

    let mut result: Vec<LargeUntrackedDir> = counts
        .into_iter()
        .filter(|(_, count)| *count >= LARGE_UNTRACKED_WARNING_THRESHOLD)
        .map(|(path, file_count)| LargeUntrackedDir { path, file_count })
        .collect();
    result.sort_by(|a, b| {
        b.file_count
            .cmp(&a.file_count)
            .then_with(|| a.path.cmp(&b.path))
    });
    result
}

fn to_session_relative_path(path: &Path, repo_prefix: Option<&Path>) -> PathBuf {
    match repo_prefix {
        Some(prefix) => path
            .strip_prefix(prefix)
            .map(PathBuf::from)
            .unwrap_or_else(|_| path.to_path_buf()),
        None => path.to_path_buf(),
    }
}

/// Create a ghost commit capturing the current state of the repository's working tree.
pub fn create_ghost_commit(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostCommit, GitToolingError> {
    create_ghost_commit_with_report(options).map(|(commit, _)| commit)
}

/// Compute a report describing the working tree for a ghost snapshot without creating a commit.
pub fn capture_ghost_snapshot_report(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostSnapshotReport, GitToolingError> {
    ensure_git_repository(options.repo_path)?;

    let repo_root = resolve_repository_root(options.repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), options.repo_path);
    let existing_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;

    let warning_files = existing_untracked
        .files
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let warning_dirs = existing_untracked
        .dirs
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();

    Ok(GhostSnapshotReport {
        large_untracked_dirs: detect_large_untracked_dirs(&warning_files, &warning_dirs),
    })
}

/// Create a ghost commit capturing the current state of the repository's working tree along with a report.
pub fn create_ghost_commit_with_report(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<(GhostCommit, GhostSnapshotReport), GitToolingError> {
    ensure_git_repository(options.repo_path)?;

    let repo_root = resolve_repository_root(options.repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), options.repo_path);
    let parent = resolve_head(repo_root.as_path())?;
    let existing_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;

    let warning_files = existing_untracked
        .files
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let warning_dirs = existing_untracked
        .dirs
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let large_untracked_dirs = detect_large_untracked_dirs(&warning_files, &warning_dirs);

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

    // Pre-populate the temporary index with HEAD so unchanged tracked files
    // are included in the snapshot tree.
    if let Some(parent_sha) = parent.as_deref() {
        run_git_for_status(
            repo_root.as_path(),
            vec![OsString::from("read-tree"), OsString::from(parent_sha)],
            Some(base_env.as_slice()),
        )?;
    }

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

    let ghost_commit = GhostCommit::new(
        commit_id,
        parent,
        existing_untracked.files,
        existing_untracked.dirs,
    );

    Ok((
        ghost_commit,
        GhostSnapshotReport {
            large_untracked_dirs,
        },
    ))
}

/// Restore the working tree to match the provided ghost commit.
pub fn restore_ghost_commit(repo_path: &Path, commit: &GhostCommit) -> Result<(), GitToolingError> {
    ensure_git_repository(repo_path)?;

    let repo_root = resolve_repository_root(repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), repo_path);
    let current_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;
    restore_to_commit_inner(repo_root.as_path(), repo_prefix.as_deref(), commit.id())?;
    remove_new_untracked(
        repo_root.as_path(),
        commit.preexisting_untracked_files(),
        commit.preexisting_untracked_dirs(),
        current_untracked,
    )
}

/// Restore the working tree to match the given commit ID.
pub fn restore_to_commit(repo_path: &Path, commit_id: &str) -> Result<(), GitToolingError> {
    ensure_git_repository(repo_path)?;

    let repo_root = resolve_repository_root(repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), repo_path);
    restore_to_commit_inner(repo_root.as_path(), repo_prefix.as_deref(), commit_id)
}

/// Restores the working tree and index to the given commit using `git restore`.
/// The repository root and optional repository-relative prefix limit the restore scope.
fn restore_to_commit_inner(
    repo_root: &Path,
    repo_prefix: Option<&Path>,
    commit_id: &str,
) -> Result<(), GitToolingError> {
    let mut restore_args = vec![
        OsString::from("restore"),
        OsString::from("--source"),
        OsString::from(commit_id),
        OsString::from("--worktree"),
        OsString::from("--staged"),
        OsString::from("--"),
    ];
    if let Some(prefix) = repo_prefix {
        restore_args.push(prefix.as_os_str().to_os_string());
    } else {
        restore_args.push(OsString::from("."));
    }

    run_git_for_status(repo_root, restore_args, None)?;
    Ok(())
}

#[derive(Default)]
struct UntrackedSnapshot {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

/// Captures the untracked and ignored entries under `repo_root`, optionally limited by `repo_prefix`.
/// Returns the result as an `UntrackedSnapshot`.
fn capture_existing_untracked(
    repo_root: &Path,
    repo_prefix: Option<&Path>,
) -> Result<UntrackedSnapshot, GitToolingError> {
    // Ask git for the zero-delimited porcelain status so we can enumerate
    // every untracked path (including ones filtered by prefix).
    let mut args = vec![
        OsString::from("status"),
        OsString::from("--porcelain=2"),
        OsString::from("-z"),
        OsString::from("--untracked-files=all"),
    ];
    if let Some(prefix) = repo_prefix {
        args.push(OsString::from("--"));
        args.push(prefix.as_os_str().to_os_string());
    }

    let output = run_git_for_stdout_all(repo_root, args, None)?;
    if output.is_empty() {
        return Ok(UntrackedSnapshot::default());
    }

    let mut snapshot = UntrackedSnapshot::default();
    // Each entry is of the form "<code> <path>" where code is '?' (untracked)
    // or '!' (ignored); everything else is irrelevant to this snapshot.
    for entry in output.split('\0') {
        if entry.is_empty() {
            continue;
        }
        let mut parts = entry.splitn(2, ' ');
        let code = parts.next();
        let path_part = parts.next();
        let (Some(code), Some(path_part)) = (code, path_part) else {
            continue;
        };
        if code != "?" && code != "!" {
            continue;
        }
        if path_part.is_empty() {
            continue;
        }

        let normalized = normalize_relative_path(Path::new(path_part))?;
        if should_ignore_for_snapshot(&normalized) {
            continue;
        }
        let absolute = repo_root.join(&normalized);
        let is_dir = absolute.is_dir();
        if is_dir {
            snapshot.dirs.push(normalized);
        } else {
            snapshot.files.push(normalized);
        }
    }

    Ok(snapshot)
}

fn should_ignore_for_snapshot(path: &Path) -> bool {
    path.components().any(|component| {
        if let Component::Normal(name) = component
            && let Some(name_str) = name.to_str()
        {
            return DEFAULT_IGNORED_DIR_NAMES
                .iter()
                .any(|ignored| ignored == &name_str);
        }
        false
    })
}

/// Removes untracked files and directories that were not present when the snapshot was captured.
fn remove_new_untracked(
    repo_root: &Path,
    preserved_files: &[PathBuf],
    preserved_dirs: &[PathBuf],
    current: UntrackedSnapshot,
) -> Result<(), GitToolingError> {
    if current.files.is_empty() && current.dirs.is_empty() {
        return Ok(());
    }

    let preserved_file_set: HashSet<PathBuf> = preserved_files.iter().cloned().collect();
    let preserved_dirs_vec: Vec<PathBuf> = preserved_dirs.to_vec();

    for path in current.files {
        if should_preserve(&path, &preserved_file_set, &preserved_dirs_vec) {
            continue;
        }
        remove_path(&repo_root.join(&path))?;
    }

    for dir in current.dirs {
        if should_preserve(&dir, &preserved_file_set, &preserved_dirs_vec) {
            continue;
        }
        remove_path(&repo_root.join(&dir))?;
    }

    Ok(())
}

/// Determines whether an untracked path should be kept because it existed in the snapshot.
fn should_preserve(
    path: &Path,
    preserved_files: &HashSet<PathBuf>,
    preserved_dirs: &[PathBuf],
) -> bool {
    if preserved_files.contains(path) {
        return true;
    }

    preserved_dirs
        .iter()
        .any(|dir| path.starts_with(dir.as_path()))
}

/// Deletes the file or directory at the provided path, ignoring if it is already absent.
fn remove_path(path: &Path) -> Result<(), GitToolingError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(err.into());
        }
    }
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
    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;
    use std::process::Command;
    use walkdir::WalkDir;

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

        let preexisting_untracked = repo.join("notes.txt");
        std::fs::write(&preexisting_untracked, "notes before\n")?;

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
        std::fs::write(&preexisting_untracked, "notes after\n")?;

        restore_ghost_commit(repo, &ghost)?;

        let tracked_after = std::fs::read_to_string(repo.join("tracked.txt"))?;
        assert_eq!(tracked_after, tracked_contents);
        let ignored_after = std::fs::read_to_string(repo.join("ignored.txt"))?;
        assert_eq!(ignored_after, ignored_contents);
        let new_file_after = std::fs::read_to_string(repo.join("new-file.txt"))?;
        assert_eq!(new_file_after, new_file_contents);
        assert_eq!(repo.join("delete-me.txt").exists(), false);
        assert!(!repo.join("ephemeral.txt").exists());
        let notes_after = std::fs::read_to_string(&preexisting_untracked)?;
        assert_eq!(notes_after, "notes before\n");

        Ok(())
    }

    #[test]
    fn create_snapshot_reports_large_untracked_dirs() -> Result<(), GitToolingError> {
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

        let models = repo.join("models");
        std::fs::create_dir(&models)?;
        for idx in 0..(LARGE_UNTRACKED_WARNING_THRESHOLD + 1) {
            let file = models.join(format!("weights-{idx}.bin"));
            std::fs::write(file, "data\n")?;
        }

        let (ghost, report) =
            create_ghost_commit_with_report(&CreateGhostCommitOptions::new(repo))?;
        assert!(ghost.parent().is_some());
        assert_eq!(
            report.large_untracked_dirs,
            vec![LargeUntrackedDir {
                path: PathBuf::from("models"),
                file_count: LARGE_UNTRACKED_WARNING_THRESHOLD + 1,
            }]
        );

        Ok(())
    }

    #[test]
    fn snapshot_ignores_default_ignored_directories() -> Result<(), GitToolingError> {
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

        let node_modules = repo.join("node_modules");
        std::fs::create_dir_all(node_modules.join("@scope/package/src"))?;
        for idx in 0..50 {
            let file = node_modules.join(format!("file-{idx}.js"));
            std::fs::write(file, "console.log('ignored');\n")?;
        }
        std::fs::write(
            node_modules.join("@scope/package/src/index.js"),
            "console.log('nested ignored');\n",
        )?;

        let venv = repo.join(".venv");
        std::fs::create_dir_all(venv.join("lib/python/site-packages"))?;
        std::fs::write(
            venv.join("lib/python/site-packages/pkg.py"),
            "print('ignored')\n",
        )?;

        let (ghost, report) =
            create_ghost_commit_with_report(&CreateGhostCommitOptions::new(repo))?;
        assert!(ghost.parent().is_some());

        for file in ghost.preexisting_untracked_files() {
            let components = file.components().collect::<Vec<_>>();
            let mut has_default_ignored_component = false;
            for component in components {
                if let Component::Normal(name) = component
                    && let Some(name_str) = name.to_str()
                    && DEFAULT_IGNORED_DIR_NAMES
                        .iter()
                        .any(|ignored| ignored == &name_str)
                {
                    has_default_ignored_component = true;
                    break;
                }
            }
            assert!(
                !has_default_ignored_component,
                "unexpected default-ignored file captured: {file:?}"
            );
        }

        for dir in ghost.preexisting_untracked_dirs() {
            let components = dir.components().collect::<Vec<_>>();
            let mut has_default_ignored_component = false;
            for component in components {
                if let Component::Normal(name) = component
                    && let Some(name_str) = name.to_str()
                    && DEFAULT_IGNORED_DIR_NAMES
                        .iter()
                        .any(|ignored| ignored == &name_str)
                {
                    has_default_ignored_component = true;
                    break;
                }
            }
            assert!(
                !has_default_ignored_component,
                "unexpected default-ignored dir captured: {dir:?}"
            );
        }

        for entry in &report.large_untracked_dirs {
            let components = entry.path.components().collect::<Vec<_>>();
            let mut has_default_ignored_component = false;
            for component in components {
                if let Component::Normal(name) = component
                    && let Some(name_str) = name.to_str()
                    && DEFAULT_IGNORED_DIR_NAMES
                        .iter()
                        .any(|ignored| ignored == &name_str)
                {
                    has_default_ignored_component = true;
                    break;
                }
            }
            assert!(
                !has_default_ignored_component,
                "unexpected default-ignored dir in large_untracked_dirs: {:?}",
                entry.path
            );
        }

        Ok(())
    }

    #[test]
    fn restore_preserves_default_ignored_directories() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
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

        let node_modules = repo.join("node_modules");
        std::fs::create_dir_all(node_modules.join("pkg"))?;
        std::fs::write(
            node_modules.join("pkg/index.js"),
            "console.log('before');\n",
        )?;

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        std::fs::write(repo.join("tracked.txt"), "snapshot delta\n")?;
        std::fs::write(node_modules.join("pkg/index.js"), "console.log('after');\n")?;
        std::fs::write(node_modules.join("pkg/extra.js"), "console.log('extra');\n")?;
        std::fs::write(repo.join("temp.txt"), "new file\n")?;

        restore_ghost_commit(repo, &ghost)?;

        let tracked_after = std::fs::read_to_string(repo.join("tracked.txt"))?;
        assert_eq!(tracked_after, "snapshot version\n");

        let node_modules_exists = node_modules.exists();
        assert!(node_modules_exists);

        let files_under_node_modules: Vec<_> = WalkDir::new(&node_modules)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .collect();
        assert!(!files_under_node_modules.is_empty());

        assert!(!repo.join("temp.txt").exists());

        Ok(())
    }

    #[test]
    fn create_snapshot_reports_nested_large_untracked_dirs_under_tracked_parent()
    -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        // Create a tracked src directory.
        let src = repo.join("src");
        std::fs::create_dir(&src)?;
        std::fs::write(src.join("main.rs"), "fn main() {}\n")?;
        run_git_in(repo, &["add", "src/main.rs"]);
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

        // Create a large untracked tree nested under the tracked src directory.
        let generated = src.join("generated").join("cache");
        std::fs::create_dir_all(&generated)?;
        for idx in 0..(LARGE_UNTRACKED_WARNING_THRESHOLD + 1) {
            let file = generated.join(format!("file-{idx}.bin"));
            std::fs::write(file, "data\n")?;
        }

        let (_, report) = create_ghost_commit_with_report(&CreateGhostCommitOptions::new(repo))?;
        assert_eq!(report.large_untracked_dirs.len(), 1);
        let entry = &report.large_untracked_dirs[0];
        assert_ne!(entry.path, PathBuf::from("src"));
        assert!(
            entry.path.starts_with(Path::new("src/generated")),
            "unexpected path for large untracked directory: {}",
            entry.path.display()
        );
        assert_eq!(entry.file_count, LARGE_UNTRACKED_WARNING_THRESHOLD + 1);

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
        assert_matches!(err, GitToolingError::PathEscapesRepository { .. });
    }

    #[test]
    /// Restoring a ghost commit from a non-git directory fails.
    fn restore_requires_git_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = restore_to_commit(temp.path(), "deadbeef").unwrap_err();
        assert_matches!(err, GitToolingError::NotAGitRepository { .. });
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
        assert!(!repo.join("temp.txt").exists());

        Ok(())
    }

    #[test]
    /// Restoring leaves ignored directories created after the snapshot untouched.
    fn restore_preserves_new_ignored_directory() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join(".gitignore"), ".vscode/\n")?;
        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
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

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        let vscode = repo.join(".vscode");
        std::fs::create_dir_all(&vscode)?;
        std::fs::write(vscode.join("settings.json"), "{\n  \"after\": true\n}\n")?;

        restore_ghost_commit(repo, &ghost)?;

        assert!(vscode.exists());
        let settings_after = std::fs::read_to_string(vscode.join("settings.json"))?;
        assert_eq!(settings_after, "{\n  \"after\": true\n}\n");

        Ok(())
    }

    #[test]
    /// Restoring leaves ignored files created after the snapshot untouched.
    fn restore_preserves_new_ignored_file() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join(".gitignore"), "ignored.txt\n")?;
        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
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

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        let ignored = repo.join("ignored.txt");
        std::fs::write(&ignored, "created later\n")?;

        restore_ghost_commit(repo, &ghost)?;

        assert!(ignored.exists());
        let contents = std::fs::read_to_string(&ignored)?;
        assert_eq!(contents, "created later\n");

        Ok(())
    }

    #[test]
    /// Restoring keeps deleted ignored files deleted when they were absent before the snapshot.
    fn restore_respects_removed_ignored_file() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join(".gitignore"), "ignored.txt\n")?;
        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
        let ignored = repo.join("ignored.txt");
        std::fs::write(&ignored, "initial state\n")?;
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

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        std::fs::remove_file(&ignored)?;

        restore_ghost_commit(repo, &ghost)?;

        assert!(!ignored.exists());

        Ok(())
    }

    #[test]
    /// Restoring leaves files matched by glob ignores intact.
    fn restore_preserves_ignored_glob_matches() -> Result<(), GitToolingError> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join(".gitignore"), "dummy-dir/*.txt\n")?;
        std::fs::write(repo.join("tracked.txt"), "snapshot version\n")?;
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

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

        let dummy_dir = repo.join("dummy-dir");
        std::fs::create_dir_all(&dummy_dir)?;
        let file1 = dummy_dir.join("file1.txt");
        let file2 = dummy_dir.join("file2.txt");
        std::fs::write(&file1, "first\n")?;
        std::fs::write(&file2, "second\n")?;

        restore_ghost_commit(repo, &ghost)?;

        assert!(file1.exists());
        assert!(file2.exists());
        assert_eq!(std::fs::read_to_string(file1)?, "first\n");
        assert_eq!(std::fs::read_to_string(file2)?, "second\n");

        Ok(())
    }
}
