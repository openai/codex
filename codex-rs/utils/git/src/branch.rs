use std::ffi::OsString;
use std::path::Path;

use crate::GitToolingError;
use crate::operations::ensure_git_repository;
use crate::operations::resolve_head;
use crate::operations::resolve_repository_root;
use crate::operations::run_git_for_stdout;

/// Returns the merge-base commit between `HEAD` and the latest version between local
/// and remote of the provided branch, if both exist.
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

    let Some(branch_ref) = resolve_branch_ref(repo_root.as_path(), branch)? else {
        return Ok(None);
    };

    let preferred_ref =
        if let Some(upstream) = resolve_upstream_if_remote_ahead(repo_root.as_path(), branch)? {
            resolve_branch_ref(repo_root.as_path(), &upstream)?.unwrap_or(branch_ref)
        } else {
            branch_ref
        };

    let merge_base = run_git_for_stdout(
        repo_root.as_path(),
        vec![
            OsString::from("merge-base"),
            OsString::from(head),
            OsString::from(preferred_ref),
        ],
        None,
    )?;

    Ok(Some(merge_base))
}

fn resolve_branch_ref(repo_root: &Path, branch: &str) -> Result<Option<String>, GitToolingError> {
    let rev = run_git_for_stdout(
        repo_root,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(branch),
        ],
        None,
    );

    match rev {
        Ok(rev) => Ok(Some(rev)),
        Err(GitToolingError::GitCommand { .. }) => Ok(None),
        Err(other) => Err(other),
    }
}

fn resolve_upstream_if_remote_ahead(
    repo_root: &Path,
    branch: &str,
) -> Result<Option<String>, GitToolingError> {
    let upstream = match run_git_for_stdout(
        repo_root,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--abbrev-ref"),
            OsString::from("--symbolic-full-name"),
            OsString::from(format!("{branch}@{{upstream}}")),
        ],
        None,
    ) {
        Ok(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed.to_string()
        }
        Err(GitToolingError::GitCommand { .. }) => return Ok(None),
        Err(other) => return Err(other),
    };

    let counts = match run_git_for_stdout(
        repo_root,
        vec![
            OsString::from("rev-list"),
            OsString::from("--left-right"),
            OsString::from("--count"),
            OsString::from(format!("{branch}...{upstream}")),
        ],
        None,
    ) {
        Ok(counts) => counts,
        Err(GitToolingError::GitCommand { .. }) => return Ok(None),
        Err(other) => return Err(other),
    };

    let mut parts = counts.split_whitespace();
    let _left: i64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let right: i64 = parts.next().unwrap_or("0").parse().unwrap_or(0);

    if right > 0 {
        Ok(Some(upstream))
    } else {
        Ok(None)
    }
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
    fn merge_base_prefers_upstream_when_remote_ahead() -> Result<(), GitToolingError> {
        let temp = tempdir()?;
        let repo = temp.path().join("repo");
        let remote = temp.path().join("remote.git");
        std::fs::create_dir_all(&repo)?;
        std::fs::create_dir_all(&remote)?;

        run_git_in(&remote, &["init", "--bare"]);
        run_git_in(&repo, &["init", "--initial-branch=main"]);
        run_git_in(&repo, &["config", "core.autocrlf", "false"]);

        std::fs::write(repo.join("base.txt"), "base\n")?;
        run_git_in(&repo, &["add", "base.txt"]);
        commit(&repo, "base commit");

        run_git_in(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run_git_in(&repo, &["push", "-u", "origin", "main"]);

        run_git_in(&repo, &["checkout", "-b", "feature"]);
        std::fs::write(repo.join("feature.txt"), "feature change\n")?;
        run_git_in(&repo, &["add", "feature.txt"]);
        commit(&repo, "feature commit");

        run_git_in(&repo, &["checkout", "--orphan", "rewrite"]);
        run_git_in(&repo, &["rm", "-rf", "."]);
        std::fs::write(repo.join("new-main.txt"), "rewritten main\n")?;
        run_git_in(&repo, &["add", "new-main.txt"]);
        commit(&repo, "rewrite main");
        run_git_in(&repo, &["branch", "-M", "rewrite", "main"]);
        run_git_in(&repo, &["branch", "--set-upstream-to=origin/main", "main"]);

        run_git_in(&repo, &["checkout", "feature"]);
        run_git_in(&repo, &["fetch", "origin"]);

        let expected = run_git_stdout(&repo, &["merge-base", "HEAD", "origin/main"]);
        let merge_base = merge_base_with_head(&repo, "main")?;
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
