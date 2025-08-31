use std::path::Path;
use std::time::Duration;

use rand::Rng;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

/// Helper: walk up from `base_dir` and return `true` if any marker in `markers`
/// is found at a parent directory. Semantics:
///   - ".sl" : returns true only if the path is a directory.
///   - ".hg" : returns true only if the path is a directory.
///   - ".svn" : returns true only if the path is a directory.
///   - any other marker (including `.git`): `.exists()`.
fn is_inside_any_marker(base_dir: &Path, markers: &[&str]) -> bool {
    fn marker_matches(dir: &Path, marker: &str) -> bool {
        let p = dir.join(marker);
        match marker {
            ".sl" => p.is_dir(),
            ".hg" => p.is_dir(),
            ".svn" => p.is_dir(),
            _ => p.exists(),
        }
    }

    let mut dir = base_dir.to_path_buf();

    loop {
        for &m in markers {
            if marker_matches(&dir, m) {
                return true;
            }
        }

        // Pop one component (go up one directory).  `pop` returns false when
        // we have reached the filesystem root.
        if !dir.pop() {
            break;
        }
    }

    false
}

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` **file or
/// directory** (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Codex to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn is_inside_git_repo(base_dir: &Path) -> bool {
    is_inside_any_marker(base_dir, &[".git"])
}

/// Return `true` if the project folder specified by the `Config` is inside a
/// repository (Currently checks for Git, Sapling, Mercurial, and Subversion).
pub fn is_inside_repo(base_dir: &Path) -> bool {
    is_inside_any_marker(base_dir, &[".git", ".sl", ".hg", ".svn"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn mkdir<P: AsRef<Path>>(p: P) {
        fs::create_dir_all(p).unwrap();
    }

    fn touch<P: AsRef<Path>>(p: P) {
        fs::write(p, b"").unwrap();
    }

    fn nested_under(root: &Path) -> PathBuf {
        let nested = root.join("a").join("b").join("c");
        mkdir(&nested);
        nested
    }

    #[test]
    fn detect_git_directory_from_root() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".git"));

        assert!(is_inside_git_repo(&repo), "should detect .git directory");
        assert!(
            is_inside_repo(&repo),
            "is_inside_repo should accept .git directory"
        );
    }

    #[test]
    fn detect_git_file() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        // .git as a *file* (common for worktrees/submodules with a gitdir file)
        touch(repo.join(".git"));

        assert!(is_inside_git_repo(&repo), "should detect .git file");
        assert!(
            is_inside_repo(&repo),
            "is_inside_repo should accept .git file"
        );
    }

    #[test]
    fn detect_git_directory_in_subdir() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".git"));

        let base = nested_under(&repo);
        assert!(is_inside_git_repo(&base), "should detect .git directory");
        assert!(
            is_inside_repo(&base),
            "is_inside_repo should accept .git directory"
        );
    }

    #[test]
    fn detect_git_file_in_subdir() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        // .git as a *file* (common for worktrees/submodules with a gitdir file)
        touch(repo.join(".git"));

        let base = nested_under(&repo);
        assert!(is_inside_git_repo(&base), "should detect .git file");
        assert!(
            is_inside_repo(&base),
            "is_inside_repo should accept .git file"
        );
    }

    #[test]
    fn detect_sapling_directory() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".sl"));

        assert!(
            is_inside_repo(&repo),
            "is_inside_repo should accept .sl directory"
        );
    }

    #[test]
    fn detect_sapling_directory_in_subdir() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".sl"));

        let base = nested_under(&repo);
        assert!(
            is_inside_repo(&base),
            "is_inside_repo should accept .sl directory"
        );
    }

    #[test]
    fn sapling_file_not_accepted() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        // .sl as a *file* does NOT count as a repo marker
        touch(repo.join(".sl"));

        assert!(
            !is_inside_repo(&repo),
            "is_inside_repo must reject .sl file (directory only)"
        );
        assert!(
            !is_inside_git_repo(&repo),
            "is_inside_git_repo should remain false without .git"
        );
    }

    #[test]
    fn detect_mercurial_directory() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".hg"));

        assert!(
            is_inside_repo(&repo),
            "is_inside_repo should accept .hg directory"
        );
    }

    #[test]
    fn detect_subversion_directory() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        mkdir(repo.join(".svn"));

        assert!(
            is_inside_repo(&repo),
            "is_inside_repo should accept .svn directory"
        );
    }

    #[test]
    fn no_markers_returns_false() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        let base = nested_under(&repo);
        assert!(!is_inside_git_repo(&base));
        assert!(!is_inside_repo(&base));
    }

    #[test]
    fn both_markers_present() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        mkdir(&repo);

        // .sl as directory and .git as file simultaneously
        mkdir(repo.join(".sl"));
        touch(repo.join(".git"));

        let base = nested_under(&repo);
        assert!(is_inside_git_repo(&base));
        assert!(is_inside_repo(&base));
    }
}
