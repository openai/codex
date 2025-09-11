use std::io;
use std::path::Path;
use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Default)]
pub(crate) struct UndoPatchResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

async fn run_git_apply(diff: &str, cwd: &Path, args: &[&str]) -> io::Result<UndoPatchResult> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(diff.as_bytes()).await?;
    }

    let output = child.wait_with_output().await?;
    Ok(UndoPatchResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

pub(crate) async fn undo_patch(diff: &str, cwd: &Path) -> io::Result<UndoPatchResult> {
    const UNDO_ARGS: [&[&str]; 2] = [
        &["apply", "--unsafe-paths", "-R"],
        &["apply", "--unsafe-paths", "--3way", "-R"],
    ];

    let mut last_result = UndoPatchResult::default();
    for args in UNDO_ARGS {
        let result = run_git_apply(diff, cwd, args).await?;
        if result.success {
            return Ok(result);
        }
        last_result = result;
    }

    Ok(last_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn run_git(cwd: &Path, args: &[&str]) -> io::Result<()> {
        let status = Command::new("git").args(args).current_dir(cwd).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "git {args:?} failed with status {status}"
            )))
        }
    }

    fn git_output(cwd: &Path, args: &[&str]) -> io::Result<String> {
        let output = Command::new("git").args(args).current_dir(cwd).output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(io::Error::other(format!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn undo_patch_reverts_changes() -> io::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        run_git(root, &["init"])?;
        run_git(root, &["config", "user.email", "codex@example.com"])?;
        run_git(root, &["config", "user.name", "Codex"])?;

        let file = root.join("foo.txt");
        fs::write(&file, "hello\n")?;
        run_git(root, &["add", "foo.txt"])?;
        run_git(root, &["commit", "-m", "initial"])?;

        fs::write(&file, "hello\nworld\n")?;
        let diff = git_output(root, &["diff", "HEAD", "--", "foo.txt"])?;
        assert!(diff.contains("diff --git"));

        let result = undo_patch(&diff, root).await?;
        assert!(
            result.success,
            "Expected undo to succeed: {}",
            result.stderr
        );
        assert_eq!(fs::read_to_string(&file)?, "hello\n");

        let second_result = undo_patch(&diff, root).await?;
        assert!(!second_result.success, "second undo should fail");

        Ok(())
    }
}
