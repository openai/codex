use std::ffi::OsString;
use std::path::Path;

use crate::GitToolingError;
use crate::operations::ensure_git_repository;
use crate::operations::resolve_head;
use crate::operations::resolve_repository_root;
use crate::operations::run_git_for_stdout;

/// Returns the merge-base commit between `HEAD` and the provided branch, if both exist.
///
/// The function mirrors `git merge-base HEAD <branch>` but returns `Ok(None)` when
/// the repository has no `HEAD` yet or when the branch cannot be resolved.
pub fn merge_base_with_head(
    repo_path: &Path,
    branch: &str,
) -> Result<Option<String>, GitToolingError> {
    ensure_git_repository(repo_path)?;
    let repo_root = resolve_repository_root(repo_path)?;
    let head = match resolve_head(repo_root.as_path())? {
        Some(head) => head,
        None => return Ok(None),
    };

    let branch_ref = match run_git_for_stdout(
        repo_root.as_path(),
        vec![
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(branch),
        ],
        None,
    ) {
        Ok(rev) => rev,
        Err(GitToolingError::GitCommand { .. }) => return Ok(None),
        Err(other) => return Err(other),
    };

    let merge_base = run_git_for_stdout(
        repo_root.as_path(),
        vec![
            OsString::from("merge-base"),
            OsString::from(head),
            OsString::from(branch_ref),
        ],
        None,
    )?;

    Ok(Some(merge_base))
}

#[cfg(test)]
mod tests {
    use super::merge_base_with_head;
    use crate::GitToolingError;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::process::Command;
    use tempfile::tempdir;

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

    fn init_test_repo(repo_path: &Path) {
        run_git_in(repo_path, &["init", "--initial-branch=main"]);
        run_git_in(repo_path, &["config", "core.autocrlf", "false"]);
    }

    fn commit(repo_path: &Path, message: &str) {
        run_git_in(
            repo_path,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                message,
            ],
        );
    }

    #[test]
    fn merge_base_returns_shared_commit() -> Result<(), GitToolingError> {
        let temp = tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join("base.txt"), "base\n")?;
        run_git_in(repo, &["add", "base.txt"]);
        commit(repo, "base commit");

        run_git_in(repo, &["checkout", "-b", "feature"]);
        std::fs::write(repo.join("feature.txt"), "feature change\n")?;
        run_git_in(repo, &["add", "feature.txt"]);
        commit(repo, "feature commit");

        run_git_in(repo, &["checkout", "main"]);
        std::fs::write(repo.join("main.txt"), "main change\n")?;
        run_git_in(repo, &["add", "main.txt"]);
        commit(repo, "main commit");

        run_git_in(repo, &["checkout", "feature"]);

        let expected = run_git_stdout(repo, &["merge-base", "HEAD", "main"]);
        let merge_base = merge_base_with_head(repo, "main")?;
        assert_eq!(merge_base, Some(expected));

        Ok(())
    }

    #[test]
    fn merge_base_returns_none_when_branch_missing() -> Result<(), GitToolingError> {
        let temp = tempdir()?;
        let repo = temp.path();
        init_test_repo(repo);

        std::fs::write(repo.join("tracked.txt"), "tracked\n")?;
        run_git_in(repo, &["add", "tracked.txt"]);
        commit(repo, "initial");

        let merge_base = merge_base_with_head(repo, "missing-branch")?;
        assert_eq!(merge_base, None);

        Ok(())
    }
}
