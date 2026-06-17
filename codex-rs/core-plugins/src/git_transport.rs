use std::process::Command;
use tempfile::TempDir;

/// Runs transport commands from a clean, minimal repository so Git cannot
/// discover repository-local configuration from the directory where Codex was
/// launched. User-level Git configuration remains available.
pub(crate) struct NeutralGitCwd {
    directory: TempDir,
}

impl NeutralGitCwd {
    pub(crate) fn new() -> std::io::Result<Self> {
        let directory = tempfile::tempdir()?;
        initialize_empty_repository(directory.path())?;
        Ok(Self { directory })
    }

    pub(crate) fn configure(&self, command: &mut Command) {
        command
            .current_dir(self.directory.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env("GIT_CEILING_DIRECTORIES", self.directory.path());
    }
}

fn initialize_empty_repository(root: &std::path::Path) -> std::io::Result<()> {
    let git_dir = root.join(".git");
    std::fs::create_dir(&git_dir)?;
    std::fs::create_dir(git_dir.join("objects"))?;
    std::fs::create_dir_all(git_dir.join("refs/heads"))?;
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n")?;
    std::fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
    )
}

#[cfg(test)]
mod tests {
    use super::NeutralGitCwd;
    use super::initialize_empty_repository;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    use std::fs;
    use std::process::Command;

    #[test]
    fn configures_an_isolated_repository_discovery_boundary() {
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");
        let mut command = Command::new("git");
        neutral_cwd.configure(&mut command);

        assert_eq!(
            command.get_current_dir(),
            Some(neutral_cwd.directory.path())
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new("GIT_CEILING_DIRECTORIES"))
                .and_then(|(_, value)| value),
            Some(neutral_cwd.directory.path().as_os_str())
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new("GIT_DIR"))
                .map(|(_, value)| value),
            Some(None)
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new("GIT_WORK_TREE"))
                .map(|(_, value)| value),
            Some(None)
        );
    }

    #[cfg(unix)]
    #[test]
    fn nested_neutral_directory_does_not_load_parent_repository_config() {
        let root = tempfile::tempdir().expect("create test root");
        let source = root.path().join("source");
        fs::create_dir(&source).expect("create source repository");
        run_git(&source, &["init"]);
        run_git(&source, &["config", "user.email", "codex-test@example.com"]);
        run_git(&source, &["config", "user.name", "Codex Test"]);
        fs::write(source.join("README.md"), "safe source\n").expect("write source file");
        run_git(&source, &["add", "README.md"]);
        run_git(&source, &["commit", "-m", "initial"]);

        let hostile_repo = root.path().join("hostile");
        fs::create_dir(&hostile_repo).expect("create hostile repository");
        run_git(&hostile_repo, &["init"]);
        run_git(
            &hostile_repo,
            &[
                "config",
                "url.file:///definitely-not-a-real-codex-test-repo.insteadOf",
                source.to_string_lossy().as_ref(),
            ],
        );

        let directory = tempfile::Builder::new()
            .prefix("neutral-")
            .tempdir_in(&hostile_repo)
            .expect("create nested neutral directory");
        initialize_empty_repository(directory.path()).expect("initialize neutral repository");
        let neutral_cwd = NeutralGitCwd { directory };
        let mut command = Command::new("git");
        neutral_cwd.configure(&mut command);
        let output = command
            .args(["ls-remote", source.to_string_lossy().as_ref(), "HEAD"])
            .output()
            .expect("run git ls-remote");

        assert!(
            output.status.success(),
            "ls-remote should ignore the parent repository's URL rewrite: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }

    #[cfg(unix)]
    fn run_git(cwd: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
